use super::csrf::Binding;
use std::sync::Arc;

/// An HTTP request, as a controller sees it.
///
/// Transport-free, like the JSON-RPC layer: whatever carried the bytes builds one of these, so a
/// controller is tested without a socket. Cheap to clone — the body is shared — because it
/// travels with the arguments of every link of an override chain.
#[derive(Debug, Clone, Default)]
pub struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: Arc<[u8]>,
    path_params: Vec<(String, String)>,
    csrf: Option<Arc<Binding>>,
}

impl Request {
    /// A request for `target`, a path with an optional query string: `/web?debug=1`.
    pub fn new(method: &str, target: &str) -> Self {
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        Request {
            method: method.to_ascii_uppercase(),
            path: path.to_string(),
            query: decode_pairs(query),
            ..Request::default()
        }
    }

    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn with_body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Arc::from(body.into());
        self
    }

    pub(crate) fn with_path_params(mut self, params: Vec<(String, String)>) -> Self {
        self.path_params = params;
        self
    }

    pub(crate) fn with_csrf(mut self, binding: Arc<Binding>) -> Self {
        self.csrf = Some(binding);
        self
    }

    /// A token to put in a form this request answers with, as `csrf_token`, or to send in the
    /// `X-CSRF-Token` header.
    ///
    /// # Panics
    /// Panics for a request that did not go through [`super::handle`], which binds it.
    pub fn csrf_token(&self) -> String {
        self.csrf
            .as_ref()
            .expect("a request is bound to its browser by http::handle")
            .token()
    }

    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    /// A header, whatever the case it was sent in.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// A cookie the browser sent, by name.
    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.header("cookie")?
            .split(';')
            .filter_map(|pair| pair.trim().split_once('='))
            .find(|(cookie, _)| *cookie == name)
            .map(|(_, value)| value)
    }

    pub fn query(&self, name: &str) -> Option<&str> {
        lookup(&self.query, name)
    }

    /// A parameter by name: from the path first, then the query string, then a submitted form.
    ///
    /// The path wins because it is what chose the route; a query parameter of the same name
    /// could otherwise contradict the URL that was matched.
    pub fn param(&self, name: &str) -> Option<String> {
        if let Some(value) = lookup(&self.path_params, name).or_else(|| self.query(name)) {
            return Some(value.to_string());
        }
        let is_form = self
            .header("content-type")
            .is_some_and(|kind| kind.starts_with("application/x-www-form-urlencoded"));
        if !is_form {
            return None;
        }
        let form = decode_pairs(std::str::from_utf8(&self.body).ok()?);
        lookup(&form, name).map(str::to_string)
    }
}

fn lookup<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// Read `a=1&b=two` into pairs, undoing the URL encoding.
pub(crate) fn decode_pairs(raw: &str) -> Vec<(String, String)> {
    raw.split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

/// Undo URL encoding: `+` is a space, `%XX` a byte. A malformed escape is kept as written.
pub(crate) fn percent_decode(raw: &str) -> String {
    let hex = |byte: u8| (byte as char).to_digit(16).map(|digit| digit as u8);
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => decoded.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                match (hex(bytes[index + 1]), hex(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        decoded.push(high * 16 + low);
                        index += 2;
                    }
                    _ => decoded.push(b'%'),
                }
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}
