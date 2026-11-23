use std::net::SocketAddr;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub turn_secret: String,
    pub turn_host: String,
    pub turn_port: u16,
    pub turn_ttl: u64,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            turn_secret: std::env::var("TURN_SECRET")
                .unwrap_or_else(|_| "nexus-dev-secret".into()),
            turn_host: std::env::var("TURN_HOST").unwrap_or_else(|_| "localhost".into()),
            turn_port: std::env::var("TURN_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3478),
            turn_ttl: std::env::var("TURN_TTL")
                .ok()
                .and_then(|t| t.parse().ok())
                .unwrap_or(86400),
        }
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host.parse().expect("invalid HOST"), self.port)
    }
}
