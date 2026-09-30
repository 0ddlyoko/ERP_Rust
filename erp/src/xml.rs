//! XML that can be changed: templates, and the extensions other plugins apply to them.
//!
//! A template's markup is parsed into a small tree, located into with a subset of XPath, changed
//! by `<xpath expr="..." position="...">` specifications, and written back. The subset covers what
//! extending markup needs: steps by name or `*`, children (`/`) or descendants (`//`), and
//! predicates on attributes (`[@name]`, `[@name='value']`) or position (`[2]`).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
    Comment(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Node>,
}

impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(attribute, _)| attribute == name)
            .map(|(_, value)| value.as_str())
    }

    /// Set an attribute, keeping its place when it was already there.
    pub fn set_attribute(&mut self, name: &str, value: &str) {
        match self
            .attributes
            .iter_mut()
            .find(|(attribute, _)| attribute == name)
        {
            Some((_, current)) => *current = value.to_string(),
            None => self.attributes.push((name.to_string(), value.to_string())),
        }
    }

    pub fn remove_attribute(&mut self, name: &str) {
        self.attributes.retain(|(attribute, _)| attribute != name);
    }
}

/// Something wrong with markup or with an extension of it, saying where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlError(pub String);

impl fmt::Display for XmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for XmlError {}

type Result<T> = std::result::Result<T, XmlError>;

/// Parse markup that may hold several top-level nodes, as a template's body does.
pub fn parse_fragment(markup: &str) -> Result<Vec<Node>> {
    let wrapped = format!("<fragment>{markup}</fragment>");
    let document = roxmltree::Document::parse(&wrapped)
        .map_err(|error| XmlError(format!("The markup is not well formed: {error}")))?;
    Ok(document
        .root_element()
        .children()
        .filter_map(convert)
        .collect())
}

/// Parse a whole document, and return its root element.
pub fn parse_document(text: &str) -> Result<Element> {
    let document = roxmltree::Document::parse(text)
        .map_err(|error| XmlError(format!("The document is not well formed: {error}")))?;
    match convert(document.root_element()) {
        Some(Node::Element(root)) => Ok(root),
        _ => Err(XmlError("The document has no root element".to_string())),
    }
}

fn convert(node: roxmltree::Node) -> Option<Node> {
    if node.is_element() {
        Some(Node::Element(Element {
            name: node.tag_name().name().to_string(),
            attributes: node
                .attributes()
                .map(|attribute| (attribute.name().to_string(), attribute.value().to_string()))
                .collect(),
            children: node.children().filter_map(convert).collect(),
        }))
    } else if node.is_text() {
        node.text().map(|text| Node::Text(text.to_string()))
    } else if node.is_comment() {
        node.text().map(|text| Node::Comment(text.to_string()))
    } else {
        None
    }
}

/// Write nodes back as markup, escaping what has to be.
pub fn to_markup(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        write_node(node, &mut out);
    }
    out
}

fn write_node(node: &Node, out: &mut String) {
    match node {
        Node::Text(text) => out.push_str(&escape(text, false)),
        Node::Comment(text) => {
            out.push_str("<!--");
            out.push_str(text);
            out.push_str("-->");
        }
        Node::Element(element) => {
            out.push('<');
            out.push_str(&element.name);
            for (name, value) in &element.attributes {
                out.push(' ');
                out.push_str(name);
                out.push_str("=\"");
                out.push_str(&escape(value, true));
                out.push('"');
            }
            if element.children.is_empty() {
                out.push_str("/>");
                return;
            }
            out.push('>');
            for child in &element.children {
                write_node(child, out);
            }
            out.push_str("</");
            out.push_str(&element.name);
            out.push('>');
        }
    }
}

fn escape(text: &str, in_attribute: bool) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' if in_attribute => escaped.push_str("&quot;"),
            other => escaped.push(other),
        }
    }
    escaped
}

// ---- locating ----

#[derive(Debug, Clone, PartialEq, Eq)]
enum Predicate {
    Has(String),
    Equals(String, String),
    Position(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Step {
    descendant: bool,
    name: Option<String>,
    predicates: Vec<Predicate>,
}

fn parse_path(expr: &str) -> Result<Vec<Step>> {
    let invalid = |why: &str| XmlError(format!("Cannot read the path {expr:?}: {why}"));
    let chars: Vec<char> = expr.trim().chars().collect();
    let mut steps = Vec::new();
    let mut index = 0;
    let mut first = true;
    while index < chars.len() {
        let descendant = if chars[index..].starts_with(&['/', '/']) {
            index += 2;
            true
        } else if chars[index] == '/' {
            index += 1;
            false
        } else if first {
            // A relative path starts anywhere, as `//` would.
            true
        } else {
            return Err(invalid("a step must follow / or //"));
        };
        first = false;

        let start = index;
        while index < chars.len() && !matches!(chars[index], '/' | '[') {
            index += 1;
        }
        let name: String = chars[start..index].iter().collect();
        if name.is_empty() {
            return Err(invalid("a step has no name"));
        }
        let name = (name != "*").then_some(name);

        let mut predicates = Vec::new();
        while index < chars.len() && chars[index] == '[' {
            let close = chars[index..]
                .iter()
                .position(|&c| c == ']')
                .map(|offset| index + offset)
                .ok_or_else(|| invalid("a predicate is not closed"))?;
            let inside: String = chars[index + 1..close].iter().collect();
            predicates.push(parse_predicate(inside.trim()).ok_or_else(|| {
                invalid(&format!(
                    "[{inside}] is not @name, @name='value' or a position"
                ))
            })?);
            index = close + 1;
        }
        steps.push(Step {
            descendant,
            name,
            predicates,
        });
    }
    if steps.is_empty() {
        return Err(invalid("it is empty"));
    }
    Ok(steps)
}

fn parse_predicate(inside: &str) -> Option<Predicate> {
    if let Ok(position) = inside.parse::<usize>() {
        return (position > 0).then_some(Predicate::Position(position));
    }
    let attribute = inside.strip_prefix('@')?;
    match attribute.split_once('=') {
        None => Some(Predicate::Has(attribute.trim().to_string())),
        Some((name, value)) => {
            let value = value.trim();
            let unquoted = value
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
                .or_else(|| value.strip_prefix('"').and_then(|v| v.strip_suffix('"')))?;
            Some(Predicate::Equals(
                name.trim().to_string(),
                unquoted.to_string(),
            ))
        }
    }
}

/// Where a node is: the indices to follow from the top-level nodes down to it.
type Path = Vec<usize>;

/// The first element the path designates, in document order.
fn locate(nodes: &[Node], steps: &[Step]) -> Option<Path> {
    let mut candidates: Vec<Path> = vec![Vec::new()];
    for step in steps {
        let mut next = Vec::new();
        for parent in &candidates {
            let children = children_at(nodes, parent);
            let mut matched: Vec<Path> = Vec::new();
            collect_matches(children, parent, step, &mut matched);
            for predicate in &step.predicates {
                matched = match predicate {
                    Predicate::Position(position) => {
                        matched.get(position - 1).cloned().into_iter().collect()
                    }
                    Predicate::Has(name) => matched
                        .into_iter()
                        .filter(|path| element_at(nodes, path).attribute(name).is_some())
                        .collect(),
                    Predicate::Equals(name, value) => matched
                        .into_iter()
                        .filter(|path| {
                            element_at(nodes, path).attribute(name) == Some(value.as_str())
                        })
                        .collect(),
                };
            }
            next.extend(matched);
        }
        candidates = next;
        if candidates.is_empty() {
            return None;
        }
    }
    candidates.into_iter().next()
}

fn collect_matches(children: &[Node], parent: &Path, step: &Step, matched: &mut Vec<Path>) {
    for (index, child) in children.iter().enumerate() {
        let Node::Element(element) = child else {
            continue;
        };
        let mut path = parent.clone();
        path.push(index);
        if step.name.as_ref().is_none_or(|name| *name == element.name) {
            matched.push(path.clone());
        }
        if step.descendant {
            collect_matches(&element.children, &path, step, matched);
        }
    }
}

fn children_at<'a>(nodes: &'a [Node], path: &Path) -> &'a [Node] {
    if path.is_empty() {
        return nodes;
    }
    &element_at(nodes, path).children
}

fn element_at<'a>(nodes: &'a [Node], path: &Path) -> &'a Element {
    let mut current = nodes;
    let mut element = None;
    for &index in path {
        let Node::Element(found) = &current[index] else {
            unreachable!("a path only goes through elements");
        };
        element = Some(found);
        current = &found.children;
    }
    element.expect("a path to an element is not empty")
}

fn siblings_mut<'a>(nodes: &'a mut Vec<Node>, parent: &[usize]) -> &'a mut Vec<Node> {
    let mut current = nodes;
    for &index in parent {
        let Node::Element(element) = &mut current[index] else {
            unreachable!("a path only goes through elements");
        };
        current = &mut element.children;
    }
    current
}

// ---- extending ----

/// Apply an extension to markup: each `<xpath expr="..." position="...">` of `spec`, in order.
///
/// A `position` of `inside` (the default) appends to the element, `before` and `after` insert
/// beside it, `replace` puts the content in its place, and `attributes` sets the attributes its
/// `<attribute name="...">` children give — removing one given empty. A path designating nothing
/// is an error: an extension that silently does nothing is one whose target moved.
pub fn apply_extension(nodes: &mut Vec<Node>, spec: &[Node]) -> Result<()> {
    for node in spec {
        let Node::Element(element) = node else {
            continue;
        };
        match element.name.as_str() {
            "data" => apply_extension(nodes, &element.children)?,
            "xpath" => apply_xpath(nodes, element)?,
            other => {
                return Err(XmlError(format!(
                    "An extension is made of <xpath> elements, not <{other}>"
                )));
            }
        }
    }
    Ok(())
}

fn apply_xpath(nodes: &mut Vec<Node>, xpath: &Element) -> Result<()> {
    let expr = xpath
        .attribute("expr")
        .ok_or_else(|| XmlError("An <xpath> needs an expr".to_string()))?;
    let steps = parse_path(expr)?;
    let target = locate(nodes, &steps)
        .ok_or_else(|| XmlError(format!("Nothing matches the path {expr:?}")))?;
    let content = xpath.children.clone();
    let (last, parent) = target.split_last().expect("a match is an element");
    let position = xpath.attribute("position").unwrap_or("inside");
    match position {
        "inside" => {
            let Node::Element(element) = &mut siblings_mut(nodes, parent)[*last] else {
                unreachable!("a match is an element");
            };
            element.children.extend(content);
        }
        "after" => {
            let siblings = siblings_mut(nodes, parent);
            for (offset, node) in content.into_iter().enumerate() {
                siblings.insert(last + 1 + offset, node);
            }
        }
        "before" => {
            let siblings = siblings_mut(nodes, parent);
            for (offset, node) in content.into_iter().enumerate() {
                siblings.insert(last + offset, node);
            }
        }
        "replace" => {
            let siblings = siblings_mut(nodes, parent);
            siblings.splice(*last..=*last, content);
        }
        "attributes" => {
            let Node::Element(element) = &mut siblings_mut(nodes, parent)[*last] else {
                unreachable!("a match is an element");
            };
            for node in &content {
                let Node::Element(attribute) = node else {
                    continue;
                };
                if attribute.name != "attribute" {
                    return Err(XmlError(format!(
                        "position=\"attributes\" takes <attribute> elements, not <{}>",
                        attribute.name
                    )));
                }
                let name = attribute
                    .attribute("name")
                    .ok_or_else(|| XmlError("An <attribute> needs a name".to_string()))?;
                let value: String = attribute
                    .children
                    .iter()
                    .filter_map(|child| match child {
                        Node::Text(text) => Some(text.as_str()),
                        _ => None,
                    })
                    .collect();
                if value.trim().is_empty() {
                    element.remove_attribute(name);
                } else {
                    element.set_attribute(name, value.trim());
                }
            }
        }
        other => {
            return Err(XmlError(format!(
                "Unknown position {other:?}: inside, before, after, replace or attributes"
            )));
        }
    }
    Ok(())
}
