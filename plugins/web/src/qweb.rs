//! Rendering templates on the server, as HTML: the part of QWeb a page needs.
//!
//! `t-call` renders another template, its body available there as `0`, the `t-set` of that body
//! as values. `t-set` names a value, from `t-value` or from its rendered body. `t-out` writes a
//! value, escaped unless it is markup. `t-call-assets` loads a bundle. Anything else starting with
//! `t-` is refused, rather than silently written out.

use erp::xml::{Element, Node};
use std::collections::{HashMap, HashSet};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// A value a template writes out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Written escaped.
    Text(String),
    /// Written as it is: rendered markup.
    Markup(String),
}

pub type Values = HashMap<String, Value>;

/// Elements HTML writes without a closing tag, and which hold nothing.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// How deep `t-call` may go: past it, templates are calling each other in a loop.
const MAXIMUM_DEPTH: usize = 32;

/// Renders templates by key, from their resolved markup.
pub struct Renderer<'a> {
    templates: &'a HashMap<String, Vec<Node>>,
    browser_templates: &'a HashSet<String>,
    import_map: &'a str,
    import_map_written: bool,
    depth: usize,
}

impl<'a> Renderer<'a> {
    /// `browser_templates` are the keys the browser renders, only to say so when one is called.
    pub fn new(
        templates: &'a HashMap<String, Vec<Node>>,
        browser_templates: &'a HashSet<String>,
        import_map: &'a str,
    ) -> Self {
        Renderer {
            templates,
            browser_templates,
            import_map,
            import_map_written: false,
            depth: 0,
        }
    }

    /// A template, rendered with these values.
    pub fn render(&mut self, key: &str, values: &mut Values) -> Result<String> {
        let nodes = self.templates.get(key).ok_or_else(|| {
            if self.browser_templates.contains(key) {
                format!("Template {key} is the browser's: it is rendered by its component")
            } else {
                format!("No template is named {key}")
            }
        })?;
        self.depth += 1;
        if self.depth > MAXIMUM_DEPTH {
            return Err(format!("Template {key} is called more than {MAXIMUM_DEPTH} deep").into());
        }
        let mut out = String::new();
        let rendered = self.nodes(nodes, values, &mut out);
        self.depth -= 1;
        rendered.map_err(|error| format!("Template {key}: {error}"))?;
        Ok(out)
    }

    fn nodes(&mut self, nodes: &[Node], values: &mut Values, out: &mut String) -> Result<()> {
        for node in nodes {
            match node {
                Node::Text(text) => out.push_str(&escape(text)),
                Node::Comment(_) => {}
                Node::Element(element) => self.element(element, values, out)?,
            }
        }
        Ok(())
    }

    fn element(&mut self, element: &Element, values: &mut Values, out: &mut String) -> Result<()> {
        if let Some((name, _)) = element.attributes.iter().find(|(name, _)| {
            name.starts_with("t-")
                && !matches!(
                    name.as_str(),
                    "t-set" | "t-value" | "t-call" | "t-out" | "t-call-assets"
                )
        }) {
            return Err(format!("<{}> uses {name}, which is not supported", element.name).into());
        }
        if let Some(name) = element.attribute("t-set") {
            let value = match element.attribute("t-value") {
                Some(expression) => evaluate(expression, values)?,
                None => {
                    let mut body = String::new();
                    self.nodes(&element.children, &mut values.clone(), &mut body)?;
                    Some(Value::Markup(body))
                }
            };
            match value {
                Some(value) => values.insert(name.to_string(), value),
                None => values.remove(name),
            };
            return Ok(());
        }
        if let Some(key) = element.attribute("t-call") {
            let mut called = values.clone();
            let mut body = String::new();
            self.nodes(&element.children, &mut called, &mut body)?;
            called.insert("0".to_string(), Value::Markup(body));
            out.push_str(&self.render(key, &mut called)?);
            return Ok(());
        }
        if let Some(bundle) = element.attribute("t-call-assets") {
            self.assets(bundle, out);
            return Ok(());
        }
        let content = match element.attribute("t-out") {
            Some(expression) => Some(evaluate(expression, values)?),
            None => None,
        };
        let is_wrapper = element.name == "t";
        if !is_wrapper {
            out.push('<');
            out.push_str(&element.name);
            for (name, value) in &element.attributes {
                if !name.starts_with("t-") {
                    out.push_str(&format!(" {name}=\"{}\"", escape_attribute(value)));
                }
            }
            out.push('>');
            if VOID_ELEMENTS.contains(&element.name.as_str()) {
                return Ok(());
            }
        }
        match content {
            Some(Some(Value::Text(text))) => out.push_str(&escape(&text)),
            Some(Some(Value::Markup(markup))) => out.push_str(&markup),
            Some(None) => {}
            None => self.nodes(&element.children, &mut values.clone(), out)?,
        }
        if !is_wrapper {
            out.push_str(&format!("</{}>", element.name));
        }
        Ok(())
    }

    /// What a page loads for a bundle: its styles and its scripts, after the import map they need
    /// — written once, before the first module.
    fn assets(&mut self, bundle: &str, out: &mut String) {
        if !self.import_map_written {
            out.push_str(&format!(
                "<script type=\"importmap\">{}</script>",
                self.import_map.replace("</", "<\\/")
            ));
            self.import_map_written = true;
        }
        let bundle = escape_attribute(bundle);
        out.push_str(&format!(
            "<link rel=\"stylesheet\" href=\"/web/assets/{bundle}.css\">\
             <script type=\"module\" src=\"/web/assets/{bundle}.js\"></script>"
        ));
    }
}

/// The value an expression names: a quoted string, a number, or a value by its name.
///
/// `None` for a name holding nothing, so that writing it writes nothing, as in QWeb.
fn evaluate(expression: &str, values: &Values) -> Result<Option<Value>> {
    let expression = expression.trim();
    let quoted = ['\'', '"'].iter().find_map(|quote| {
        expression
            .strip_prefix(*quote)
            .and_then(|rest| rest.strip_suffix(*quote))
            .filter(|inner| !inner.contains(*quote))
    });
    if let Some(literal) = quoted {
        return Ok(Some(Value::Text(literal.to_string())));
    }
    if !expression.is_empty() && expression.chars().all(|c| c.is_ascii_digit()) {
        if let Some(value) = values.get(expression) {
            return Ok(Some(value.clone()));
        }
        return Ok(Some(Value::Text(expression.to_string())));
    }
    let is_name = expression
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && expression
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if is_name {
        return Ok(values.get(expression).cloned());
    }
    Err(format!("{expression:?} is not an expression the server renders").into())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attribute(text: &str) -> String {
    escape(text).replace('"', "&quot;")
}
