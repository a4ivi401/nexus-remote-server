use futures::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

use nexus_remote_server::config::Config;
use nexus_remote_server::server;
use nexus_remote_server::session::SessionManager;
use nexus_remote_server::ws::messages::{ClientMessage, ServerMessage};
use nexus_remote_server::ws::SharedState;

async fn spawn_test_server() -> SocketAddr {
    let config = Config {
        host: "127.0.0.1".into(),
        port: 0,
        turn_secret: "test-secret".into(),
        turn_host: "127.0.0.1".into(),
        turn_port: 3478,
        turn_ttl: 3600,
    };

    let sessions = SessionManager::new();
    let state = Arc::new(SharedState {
        sessions,
        config,
    });
    let app = server::create_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    addr
}

async fn reqwest_get(addr: SocketAddr, path: &str) -> String {
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    tokio::io::AsyncWriteExt::write_all(&mut stream, req.as_bytes())
        .await
        .unwrap();

    let mut response = String::new();
    tokio::io::AsyncReadExt::read_to_string(&mut stream, &mut response)
        .await
        .unwrap();
    response
}

#[tokio::test]
async fn test_health_endpoint() {
    let addr = spawn_test_server().await;
    let resp = reqwest_get(addr, "/health").await;
    assert!(resp.contains("\"status\":\"ok\""));
    assert!(resp.contains("\"active_sessions\":0"));
    assert!(resp.contains("\"active_connections\":0"));
}

#[tokio::test]
async fn test_full_signaling_flow() {
    let addr = spawn_test_server().await;
    let ws_url = format!("ws://{addr}/ws");

    // Client A connects
    let (mut ws_a, _) = connect_async(&ws_url).await.unwrap();
    let reg_msg = serde_json::to_string(&ClientMessage::Register).unwrap();
    ws_a.send(Message::Text(reg_msg.into())).await.unwrap();

    let msg_a_1 = ws_a.next().await.unwrap().unwrap();
    let srv_msg_a: ServerMessage = serde_json::from_str(msg_a_1.to_text().unwrap()).unwrap();
    let session_a = match srv_msg_a {
        ServerMessage::Registered { session_id, turn } => {
            assert_eq!(session_id.len(), 6);
            assert!(!turn.urls.is_empty());
            assert!(!turn.username.is_empty());
            assert!(!turn.credential.is_empty());
            session_id
        }
        _ => panic!("Expected Registered message"),
    };

    // Client B connects
    let (mut ws_b, _) = connect_async(&ws_url).await.unwrap();
    let reg_msg_b = serde_json::to_string(&ClientMessage::Register).unwrap();
    ws_b.send(Message::Text(reg_msg_b.into())).await.unwrap();

    let msg_b_1 = ws_b.next().await.unwrap().unwrap();
    let srv_msg_b: ServerMessage = serde_json::from_str(msg_b_1.to_text().unwrap()).unwrap();
    let session_b = match srv_msg_b {
        ServerMessage::Registered { session_id, turn } => {
            assert_eq!(session_id.len(), 6);
            assert_ne!(session_id, session_a);
            assert!(!turn.urls.is_empty());
            session_id
        }
        _ => panic!("Expected Registered message"),
    };

    // Client A requests connection to Client B
    let conn_req = serde_json::to_string(&ClientMessage::ConnectRequest {
        target_id: session_b.clone(),
    })
    .unwrap();
    ws_a.send(Message::Text(conn_req.into())).await.unwrap();

    // Client B receives connection request
    let msg_b_2 = ws_b.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(msg_b_2.to_text().unwrap()).unwrap();
    match srv_msg {
        ServerMessage::ConnectionRequest { requester_id } => {
            assert_eq!(requester_id, session_a);
        }
        _ => panic!("Expected ConnectionRequest message"),
    }

    // Client B accepts connection
    let conn_resp = serde_json::to_string(&ClientMessage::ConnectResponse {
        requester_id: session_a.clone(),
        accepted: true,
    })
    .unwrap();
    ws_b.send(Message::Text(conn_resp.into())).await.unwrap();

    // Client B receives ConnectionAccepted
    let msg_b_3 = ws_b.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(msg_b_3.to_text().unwrap()).unwrap();
    match srv_msg {
        ServerMessage::ConnectionAccepted { peer_id } => {
            assert_eq!(peer_id, session_a);
        }
        _ => panic!("Expected ConnectionAccepted on B"),
    }

    // Client A receives ConnectionAccepted
    let msg_a_2 = ws_a.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(msg_a_2.to_text().unwrap()).unwrap();
    match srv_msg {
        ServerMessage::ConnectionAccepted { peer_id } => {
            assert_eq!(peer_id, session_b);
        }
        _ => panic!("Expected ConnectionAccepted on A"),
    }

    // Client A sends signal (SDP offer) to Client B
    let signal_offer = serde_json::to_string(&ClientMessage::Signal {
        target_id: session_b.clone(),
        data: "v=0\r\no=offer 1 1 IN IP4 127.0.0.1".into(),
    })
    .unwrap();
    ws_a.send(Message::Text(signal_offer.into())).await.unwrap();

    // Client B receives signal from Client A
    let msg_b_4 = ws_b.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(msg_b_4.to_text().unwrap()).unwrap();
    match srv_msg {
        ServerMessage::Signal { from_id, data } => {
            assert_eq!(from_id, session_a);
            assert!(data.contains("o=offer"));
        }
        _ => panic!("Expected Signal on B"),
    }

    // Client B sends signal (SDP answer) to Client A
    let signal_answer = serde_json::to_string(&ClientMessage::Signal {
        target_id: session_a.clone(),
        data: "v=0\r\no=answer 1 1 IN IP4 127.0.0.1".into(),
    })
    .unwrap();
    ws_b.send(Message::Text(signal_answer.into())).await.unwrap();

    // Client A receives signal from Client B
    let msg_a_3 = ws_a.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(msg_a_3.to_text().unwrap()).unwrap();
    match srv_msg {
        ServerMessage::Signal { from_id, data } => {
            assert_eq!(from_id, session_b);
            assert!(data.contains("o=answer"));
        }
        _ => panic!("Expected Signal on A"),
    }

    // Disconnect
    let disc_msg = serde_json::to_string(&ClientMessage::Disconnect {
        target_id: session_b.clone(),
    })
    .unwrap();
    ws_a.send(Message::Text(disc_msg.into())).await.unwrap();

    let msg_b_5 = ws_b.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(msg_b_5.to_text().unwrap()).unwrap();
    match srv_msg {
        ServerMessage::PeerDisconnected { peer_id } => {
            assert_eq!(peer_id, session_a);
        }
        _ => panic!("Expected PeerDisconnected on B"),
    }
}

#[tokio::test]
async fn test_connection_rejection() {
    let addr = spawn_test_server().await;
    let ws_url = format!("ws://{addr}/ws");

    let (mut ws_a, _) = connect_async(&ws_url).await.unwrap();
    ws_a.send(Message::Text(
        serde_json::to_string(&ClientMessage::Register).unwrap().into(),
    ))
    .await
    .unwrap();
    let msg_a_1 = ws_a.next().await.unwrap().unwrap();
    let session_a = match serde_json::from_str::<ServerMessage>(msg_a_1.to_text().unwrap()).unwrap()
    {
        ServerMessage::Registered { session_id, .. } => session_id,
        _ => panic!("Expected Registered"),
    };

    let (mut ws_b, _) = connect_async(&ws_url).await.unwrap();
    ws_b.send(Message::Text(
        serde_json::to_string(&ClientMessage::Register).unwrap().into(),
    ))
    .await
    .unwrap();
    let msg_b_1 = ws_b.next().await.unwrap().unwrap();
    let session_b = match serde_json::from_str::<ServerMessage>(msg_b_1.to_text().unwrap()).unwrap()
    {
        ServerMessage::Registered { session_id, .. } => session_id,
        _ => panic!("Expected Registered"),
    };

    // A requests B
    ws_a.send(Message::Text(
        serde_json::to_string(&ClientMessage::ConnectRequest {
            target_id: session_b.clone(),
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();

    // B receives request and rejects
    let _ = ws_b.next().await.unwrap().unwrap();
    ws_b.send(Message::Text(
        serde_json::to_string(&ClientMessage::ConnectResponse {
            requester_id: session_a.clone(),
            accepted: false,
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();

    // A receives ConnectionRejected
    let msg_a_2 = ws_a.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(msg_a_2.to_text().unwrap()).unwrap();
    match srv_msg {
        ServerMessage::ConnectionRejected { peer_id, reason } => {
            assert_eq!(peer_id, session_b);
            assert_eq!(reason, "rejected");
        }
        _ => panic!("Expected ConnectionRejected"),
    }
}

#[tokio::test]
async fn test_edge_cases() {
    let addr = spawn_test_server().await;
    let ws_url = format!("ws://{addr}/ws");

    let (mut ws, _) = connect_async(&ws_url).await.unwrap();

    // 1. Send Ping -> get Pong
    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::Ping).unwrap().into(),
    ))
    .await
    .unwrap();
    let pong_resp = ws.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(pong_resp.to_text().unwrap()).unwrap();
    assert_eq!(srv_msg, ServerMessage::Pong);

    // 2. Connect before register -> not_registered error
    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::ConnectRequest {
            target_id: "123456".into(),
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    let err_resp = ws.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(err_resp.to_text().unwrap()).unwrap();
    assert!(matches!(srv_msg, ServerMessage::Error { code, .. } if code == "not_registered"));

    // 3. Register
    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::Register).unwrap().into(),
    ))
    .await
    .unwrap();
    let reg_resp = ws.next().await.unwrap().unwrap();
    let my_id = match serde_json::from_str::<ServerMessage>(reg_resp.to_text().unwrap()).unwrap() {
        ServerMessage::Registered { session_id, .. } => session_id,
        _ => panic!("Expected Registered"),
    };

    // 4. Register again -> already_registered error
    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::Register).unwrap().into(),
    ))
    .await
    .unwrap();
    let double_reg = ws.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(double_reg.to_text().unwrap()).unwrap();
    assert!(matches!(srv_msg, ServerMessage::Error { code, .. } if code == "already_registered"));

    // 5. Connect to self -> invalid_target error
    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::ConnectRequest {
            target_id: my_id.clone(),
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    let self_conn = ws.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(self_conn.to_text().unwrap()).unwrap();
    assert!(matches!(srv_msg, ServerMessage::Error { code, .. } if code == "invalid_target"));

    // 6. Connect to non-existent target -> ConnectionRejected with not_found
    ws.send(Message::Text(
        serde_json::to_string(&ClientMessage::ConnectRequest {
            target_id: "999999".into(),
        })
        .unwrap()
        .into(),
    ))
    .await
    .unwrap();
    let not_found_resp = ws.next().await.unwrap().unwrap();
    let srv_msg: ServerMessage = serde_json::from_str(not_found_resp.to_text().unwrap()).unwrap();
    assert!(matches!(srv_msg, ServerMessage::ConnectionRejected { reason, .. } if reason == "not_found"));
}
