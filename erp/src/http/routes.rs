use crate::environment::Environment;
use crate::http::{Request, Response};
use erp_internal_types::MethodRegistry;
use erp_types::method::MethodFn;
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// What a route calls: reads its parameters out of the request, and calls the method by its own
/// name so the call goes through the override chain.
pub type HttpFn = fn(&mut Environment, &Request) -> Result<Response>;

/// A struct whose methods answer URLs.
///
/// Controllers carry no state — every call gets the environment and the request — so the struct
/// is only an identity: several structs declaring the same `controller_name` extend one
/// controller, the way several structs declaring the same model id extend one model.
pub trait Controller {
    fn controller_name() -> &'static str;
    fn instance() -> Self;
}

/// Registers the methods and routes an `#[erp_routes]` block declares.
pub trait HasRoutes {
    fn register_routes(registry: &mut ControllerRegistry);
}

/// One segment of a route: fixed, a parameter, or — last — the rest of the path, `<*name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Literal(String),
    Param(String),
    Rest(String),
}

struct Route {
    pattern: String,
    segments: Vec<Segment>,
    methods: Vec<String>,
    controller: String,
    method: String,
    call: HttpFn,
    plugin: String,
}

impl Route {
    fn literals(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| matches!(segment, Segment::Literal(_)))
            .count()
    }

    /// The parameters, when this route matches the path.
    ///
    /// A rest segment takes one segment or more, joined back with `/`.
    fn matches(&self, path: &[&str]) -> Option<Vec<(String, String)>> {
        let rest = matches!(self.segments.last(), Some(Segment::Rest(_)));
        let fits = if rest {
            path.len() >= self.segments.len()
        } else {
            path.len() == self.segments.len()
        };
        if !fits {
            return None;
        }
        let decode = crate::http::request::percent_decode;
        let mut params = Vec::new();
        for (index, segment) in self.segments.iter().enumerate() {
            let part = path[index];
            match segment {
                Segment::Literal(literal) if literal == part => {}
                Segment::Literal(_) => return None,
                Segment::Param(name) => params.push((name.clone(), decode(part))),
                Segment::Rest(name) => {
                    let joined: Vec<String> =
                        path[index..].iter().map(|part| decode(part)).collect();
                    params.push((name.clone(), joined.join("/")));
                }
            }
        }
        Some(params)
    }

    /// Whether two routes answer the same URLs: same shape, whatever their parameters are named.
    fn same_shape(&self, other: &Route) -> bool {
        self.segments.len() == other.segments.len()
            && self
                .segments
                .iter()
                .zip(&other.segments)
                .all(|pair| match pair {
                    (Segment::Literal(a), Segment::Literal(b)) => a == b,
                    (Segment::Param(_), Segment::Param(_)) => true,
                    (Segment::Rest(_), Segment::Rest(_)) => true,
                    _ => false,
                })
    }
}

fn split(path: &str) -> Vec<&str> {
    path.split('/').filter(|part| !part.is_empty()).collect()
}

fn parse_pattern(pattern: &str) -> Vec<Segment> {
    split(pattern)
        .into_iter()
        .map(
            |part| match part.strip_prefix('<').and_then(|p| p.strip_suffix('>')) {
                Some(name) => match name.strip_prefix('*') {
                    Some(rest) => Segment::Rest(rest.to_string()),
                    None => Segment::Param(name.to_string()),
                },
                None => Segment::Literal(part.to_string()),
            },
        )
        .collect()
}

/// What a request resolves to.
pub enum Resolution {
    Found {
        call: HttpFn,
        params: Vec<(String, String)>,
    },
    MethodNotAllowed(Vec<String>),
    NotFound,
}

/// Every controller the loaded plugins declared: their override chains, and the URLs they answer.
///
/// Held by the model manager for the same reason the RPC registry is: it holds function pointers
/// from plugin libraries, and the manager is cleared before any of them is unloaded.
#[derive(Default)]
pub struct ControllerRegistry {
    methods: HashMap<String, MethodRegistry>,
    routes: Vec<Route>,
    pub(crate) current_plugin_loading: Option<String>,
}

impl ControllerRegistry {
    /// Register a controller's methods and routes, on behalf of the plugin being loaded.
    pub fn register<C: Controller + HasRoutes>(&mut self) {
        C::register_routes(self);
    }

    fn plugin(&self) -> String {
        self.current_plugin_loading
            .clone()
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Add one implementation of a controller method, ahead of the ones registered before it.
    pub fn register_method<A: 'static, R: 'static>(
        &mut self,
        controller: &str,
        method: &str,
        link: MethodFn<A, R>,
    ) {
        let plugin = self.plugin();
        self.methods
            .entry(controller.to_string())
            .or_default()
            .register(controller, method, link, &plugin);
    }

    /// Say which URL a controller method answers.
    ///
    /// A route belongs to the method, not to one implementation of it: an override that declares
    /// a route replaces the one its method had, and one that does not keeps it. Two different
    /// methods answering the same URL with the same verb is refused, because which one answered
    /// would depend on load order.
    pub fn register_route(
        &mut self,
        controller: &str,
        method: &str,
        pattern: &str,
        methods: &[&str],
        call: HttpFn,
    ) {
        let route = Route {
            pattern: pattern.to_string(),
            segments: parse_pattern(pattern),
            methods: methods
                .iter()
                .map(|verb| verb.to_ascii_uppercase())
                .collect(),
            controller: controller.to_string(),
            method: method.to_string(),
            call,
            plugin: self.plugin(),
        };
        self.routes
            .retain(|existing| !(existing.controller == controller && existing.method == method));
        if let Some(clash) = self.routes.iter().find(|existing| {
            existing.same_shape(&route)
                && existing
                    .methods
                    .iter()
                    .any(|verb| route.methods.contains(verb))
        }) {
            panic!(
                "Two controller methods answer the same URL.\n  {}.{} ({}) answers {} {:?}\n  \
                 {}.{} ({}) answers {} {:?}\nWhich one answered would depend on load order.",
                clash.controller,
                clash.method,
                clash.plugin,
                clash.pattern,
                clash.methods,
                route.controller,
                route.method,
                route.plugin,
                route.pattern,
                route.methods,
            );
        }
        self.routes.push(route);
    }

    /// Implementations of a controller method, most-derived first.
    pub fn chain<A: 'static, R: 'static>(
        &self,
        controller: &str,
        method: &str,
    ) -> Option<&[MethodFn<A, R>]> {
        self.methods.get(controller)?.chain::<A, R>(method)
    }

    /// The route answering a request.
    ///
    /// The one with the most fixed segments wins, so `/web/login` is not taken for `/web/<page>`.
    pub fn resolve(&self, verb: &str, path: &str) -> Resolution {
        let parts = split(path);
        let mut matching: Vec<(&Route, Vec<(String, String)>)> = self
            .routes
            .iter()
            .filter_map(|route| route.matches(&parts).map(|params| (route, params)))
            .collect();
        if matching.is_empty() {
            return Resolution::NotFound;
        }
        matching.sort_by_key(|(route, _)| std::cmp::Reverse(route.literals()));
        let verb = verb.to_ascii_uppercase();
        let allowed: Vec<String> = matching
            .iter()
            .flat_map(|(route, _)| route.methods.clone())
            .collect();
        match matching
            .into_iter()
            .find(|(route, _)| route.methods.contains(&verb))
        {
            Some((route, params)) => Resolution::Found {
                call: route.call,
                params,
            },
            None => Resolution::MethodNotAllowed(allowed),
        }
    }

    /// Every route, as `(verbs, pattern, controller.method)`, for whoever lists what is served.
    pub fn routes(&self) -> Vec<(Vec<String>, String, String)> {
        self.routes
            .iter()
            .map(|route| {
                (
                    route.methods.clone(),
                    route.pattern.clone(),
                    format!("{}.{}", route.controller, route.method),
                )
            })
            .collect()
    }
}
