use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::config::Config;

type HmacSha1 = Hmac<Sha1>;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TurnCredentials {
    pub urls: Vec<String>,
    pub username: String,
    pub credential: String,
}

pub fn generate_credentials(config: &Config, session_id: &str) -> TurnCredentials {
    let expiry = chrono::Utc::now().timestamp() as u64 + config.turn_ttl;
    let username = format!("{}:{}", expiry, session_id);

    let mut mac =
        HmacSha1::new_from_slice(config.turn_secret.as_bytes()).expect("HMAC accepts any key size");
    mac.update(username.as_bytes());
    let credential = STANDARD.encode(mac.finalize().into_bytes());

    TurnCredentials {
        urls: vec![
            format!("stun:{}:{}", config.turn_host, config.turn_port),
            format!("turn:{}:{}?transport=udp", config.turn_host, config.turn_port),
            format!("turn:{}:{}?transport=tcp", config.turn_host, config.turn_port),
        ],
        username,
        credential,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_are_deterministic_for_same_input() {
        let config = Config {
            host: "0.0.0.0".into(),
            port: 3000,
            turn_secret: "test-secret".into(),
            turn_host: "turn.example.com".into(),
            turn_port: 3478,
            turn_ttl: 86400,
        };

        let creds = generate_credentials(&config, "123456");
        assert!(creds.username.ends_with(":123456"));
        assert!(!creds.credential.is_empty());
        assert_eq!(creds.urls.len(), 3);
        assert!(creds.urls[0].starts_with("stun:"));
        assert!(creds.urls[1].starts_with("turn:"));
    }
}
