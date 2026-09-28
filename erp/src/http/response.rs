/// What a controller answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, content_type: &str, body: impl Into<Vec<u8>>) -> Self {
        Response {
            status,
            headers: vec![("Content-Type".to_string(), content_type.to_string())],
            body: body.into(),
        }
    }

    pub fn html(body: impl Into<String>) -> Self {
        Response::new(200, "text/html; charset=utf-8", body.into())
    }

    pub fn text(body: impl Into<String>) -> Self {
        Response::new(200, "text/plain; charset=utf-8", body.into())
    }

    pub fn json(value: &serde_json::Value) -> Self {
        Response::new(200, "application/json", value.to_string())
    }

    /// Bytes of a known kind: an image, a script, a stylesheet.
    pub fn file(body: impl Into<Vec<u8>>, content_type: &str) -> Self {
        Response::new(200, content_type, body)
    }

    /// Send the browser elsewhere, with a GET: 303, so a form posted here is not posted again.
    pub fn redirect(location: &str) -> Self {
        Response::new(303, "text/plain; charset=utf-8", Vec::new())
            .with_header("Location", location)
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }

    /// Set a header, replacing one of the same name.
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers
            .retain(|(header, _)| !header.eq_ignore_ascii_case(name));
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    pub fn status(&self) -> u16 {
        self.status
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// The body as text, for a test or a log.
    pub fn text_body(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// Nothing answered: what `sup.call` yields when no implementation is left below.
impl Default for Response {
    fn default() -> Self {
        Response::text("Nothing is served here").with_status(404)
    }
}
