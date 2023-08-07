use std::fmt::{Display, Formatter};

pub struct HttpStatus {
    pub code: u32
}

macro_rules! make_http_status {
    ($($code: expr, $code_str: expr, $fnc_name: ident, $name: expr),+) => {
        $(
            #[allow(non_upper_case_globals)]
            pub const $fnc_name: HttpStatus = HttpStatus { code: $code };
        )+

        pub fn reason(&self) -> Option<&'static str> {
            match self.code {
                $($code => Some($name),)+
                _ => None
            }
        }
    }
}

impl HttpStatus {
    make_http_status! {
        100, "100", Continue, "Continue",
        101, "101", SwitchingProtocols, "Switching Protocols",
        102, "102", Processing, "Processing",
        200, "200", Ok, "OK",
        201, "201", Created, "Created",
        202, "202", Accepted, "Accepted",
        203, "203", NonAuthoritativeInformation, "Non-Authoritative Information",
        204, "204", NoContent, "No Content",
        205, "205", ResetContent, "Reset Content",
        206, "206", PartialContent, "Partial Content",
        207, "207", MultiStatus, "Multi-Status",
        208, "208", AlreadyReported, "Already Reported",
        226, "226", ImUsed, "IM Used",
        300, "300", MultipleChoices, "Multiple Choices",
        301, "301", MovedPermanently, "Moved Permanently",
        302, "302", Found, "Found",
        303, "303", SeeOther, "See Other",
        304, "304", NotModified, "Not Modified",
        305, "305", UseProxy, "Use Proxy",
        307, "307", TemporaryRedirect, "Temporary Redirect",
        308, "308", PermanentRedirect, "Permanent Redirect",
        400, "400", BadRequest, "Bad Request",
        401, "401", Unauthorized, "Unauthorized",
        402, "402", PaymentRequired, "Payment Required",
        403, "403", Forbidden, "Forbidden",
        404, "404", NotFound, "Not Found",
        405, "405", MethodNotAllowed, "Method Not Allowed",
        406, "406", NotAcceptable, "Not Acceptable",
        407, "407", ProxyAuthenticationRequired, "Proxy Authentication Required",
        408, "408", RequestTimeout, "Request Timeout",
        409, "409", Conflict, "Conflict",
        410, "410", Gone, "Gone",
        411, "411", LengthRequired, "Length Required",
        412, "412", PreconditionFailed, "Precondition Failed",
        413, "413", PayloadTooLarge, "Payload Too Large",
        414, "414", UriTooLong, "URI Too Long",
        415, "415", UnsupportedMediaType, "Unsupported Media Type",
        416, "416", RangeNotSatisfiable, "Range Not Satisfiable",
        417, "417", ExpectationFailed, "Expectation Failed",
        418, "418", ImATeapot, "I'm a teapot",
        421, "421", MisdirectedRequest, "Misdirected Request",
        422, "422", UnprocessableEntity, "Unprocessable Entity",
        423, "423", Locked, "Locked",
        424, "424", FailedDependency, "Failed Dependency",
        426, "426", UpgradeRequired, "Upgrade Required",
        428, "428", PreconditionRequired, "Precondition Required",
        429, "429", TooManyRequests, "Too Many Requests",
        431, "431", RequestHeaderFieldsTooLarge, "Request Header Fields Too Large",
        451, "451", UnavailableForLegalReasons, "Unavailable For Legal Reasons",
        500, "500", InternalServerError, "Internal Server Error",
        501, "501", NotImplemented, "Not Implemented",
        502, "502", BadGateway, "Bad Gateway",
        503, "503", ServiceUnavailable, "Service Unavailable",
        504, "504", GatewayTimeout, "Gateway Timeout",
        505, "505", HttpVersionNotSupported, "HTTP Version Not Supported",
        506, "506", VariantAlsoNegotiates, "Variant Also Negotiates",
        507, "507", InsufficientStorage, "Insufficient Storage",
        508, "508", LoopDetected, "Loop Detected",
        510, "510", NotExtended, "Not Extended",
        511, "511", NetworkAuthenticationRequired, "Network Authentication Required"
    }

    pub const fn new(code: u32) -> HttpStatus {
        HttpStatus { code: code }
    }
}

impl Display for HttpStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Http Code {} ({:?})", self.code, self.reason())
    }
}
