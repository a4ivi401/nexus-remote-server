use serde::{Deserialize, Serialize};

use crate::turn::TurnCredentials;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "register")]
    Register,

    #[serde(rename = "connect_request")]
    ConnectRequest { target_id: String },

    #[serde(rename = "connect_response")]
    ConnectResponse {
        requester_id: String,
        accepted: bool,
    },

    #[serde(rename = "signal")]
    Signal { target_id: String, data: String },

    #[serde(rename = "disconnect")]
    Disconnect { target_id: String },

    #[serde(rename = "pong")]
    Pong,

    #[serde(rename = "ping")]
    Ping,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type")]
pub enum ServerMessage {
    #[serde(rename = "registered")]
    Registered {
        session_id: String,
        turn: TurnCredentials,
    },

    #[serde(rename = "connection_request")]
    ConnectionRequest { requester_id: String },

    #[serde(rename = "connection_accepted")]
    ConnectionAccepted { peer_id: String },

    #[serde(rename = "connection_rejected")]
    ConnectionRejected { peer_id: String, reason: String },

    #[serde(rename = "signal")]
    Signal { from_id: String, data: String },

    #[serde(rename = "peer_disconnected")]
    PeerDisconnected { peer_id: String },

    #[serde(rename = "error")]
    Error { code: String, message: String },

    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "pong")]
    Pong,
}

impl ServerMessage {
    pub fn error(code: &str, message: &str) -> Self {
        Self::Error {
            code: code.to_string(),
            message: message.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_register() {
        let msg: ClientMessage = serde_json::from_str(r#"{"type":"register"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::Register));
    }

    #[test]
    fn parse_connect_request() {
        let msg: ClientMessage =
            serde_json::from_str(r#"{"type":"connect_request","target_id":"123456"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::ConnectRequest { target_id } if target_id == "123456"));
    }

    #[test]
    fn parse_ping_pong() {
        let msg: ClientMessage = serde_json::from_str(r#"{"type":"ping"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::Ping));
        let msg: ClientMessage = serde_json::from_str(r#"{"type":"pong"}"#).unwrap();
        assert!(matches!(msg, ClientMessage::Pong));

        let srv_ping = serde_json::to_string(&ServerMessage::Ping).unwrap();
        assert_eq!(srv_ping, r#"{"type":"ping"}"#);
        let srv_pong = serde_json::to_string(&ServerMessage::Pong).unwrap();
        assert_eq!(srv_pong, r#"{"type":"pong"}"#);
    }

    #[test]
    fn parse_signal() {
        let msg: Result<ClientMessage, _> = serde_json::from_str(
            r#"{"type":"signal","target_id":"123456","data":"{\"sdp\":\"...\"}""#,
        );
        assert!(msg.is_err()); // missing closing brace
        let msg: ClientMessage = serde_json::from_str(
            r#"{"type":"signal","target_id":"123456","data":"{\"sdp\":\"...\"}"}"#,
        )
        .unwrap();
        assert!(matches!(msg, ClientMessage::Signal { .. }));
    }

    #[test]
    fn serialize_registered() {
        let msg = ServerMessage::Registered {
            session_id: "123456".into(),
            turn: TurnCredentials {
                urls: vec!["stun:localhost:3478".into()],
                username: "user".into(),
                credential: "pass".into(),
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""type":"registered"#));
        assert!(json.contains(r#""session_id":"123456"#));
    }
}
