use serde::Deserialize;

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    // Not 8069: that is Odoo's, and running both on one machine is the normal case here.
    8080
}

/// What the server listens on, and how much it serves at once.
///
/// Fields are public so an embedder — or a test — can configure it in code instead of through a
/// file.
#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    /// Address to bind. `127.0.0.1` accepts only local callers; `0.0.0.0` accepts any.
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// Requests served at once. `0` means no bound.
    ///
    /// Separate from the connection pool: a request waiting for a turn has not taken a
    /// connection yet, which is what keeps a burst from holding the pool open while it queues.
    #[serde(default)]
    pub max_concurrent_requests: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            max_concurrent_requests: 0,
        }
    }
}

impl ServerConfig {
    /// The address to bind, as a socket address.
    pub fn address(&self) -> std::result::Result<std::net::SocketAddr, std::io::Error> {
        use std::net::ToSocketAddrs;
        (self.host.as_str(), self.port)
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("{}:{} resolves to no address", self.host, self.port),
                )
            })
    }
}
