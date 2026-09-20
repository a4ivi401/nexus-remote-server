mod types;

pub use types::Session;

use axum::extract::ws::Message;
use dashmap::DashMap;
use rand::Rng;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct SessionManager {
    sessions: DashMap<String, Session>,
    connections: DashMap<String, String>,
}

impl SessionManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: DashMap::new(),
            connections: DashMap::new(),
        })
    }

    pub fn register(&self, tx: mpsc::Sender<Message>) -> String {
        let id = self.generate_id();
        self.sessions.insert(id.clone(), Session::new(id.clone(), tx));
        id
    }

    pub fn unregister(&self, session_id: &str) {
        self.sessions.remove(session_id);
        let peer = self.connections.remove(session_id).map(|(_, v)| v);
        if let Some(peer_id) = peer {
            self.connections.remove(&peer_id);
        }
    }

    pub async fn send_to(&self, target_id: &str, msg: Message) -> bool {
        if let Some(session) = self.sessions.get(target_id) {
            session.tx.send(msg).await.is_ok()
        } else {
            false
        }
    }

    pub fn session_exists(&self, id: &str) -> bool {
        self.sessions.contains_key(id)
    }

    pub fn add_connection(&self, a: &str, b: &str) {
        self.connections.insert(a.to_string(), b.to_string());
        self.connections.insert(b.to_string(), a.to_string());
    }

    pub fn remove_connection(&self, id: &str) -> Option<String> {
        if let Some((_, peer)) = self.connections.remove(id) {
            self.connections.remove(&peer);
            Some(peer)
        } else {
            None
        }
    }

    pub fn get_peer(&self, id: &str) -> Option<String> {
        self.connections.get(id).map(|r| r.value().clone())
    }

    pub fn active_sessions(&self) -> usize {
        self.sessions.len()
    }

    pub fn active_connections(&self) -> usize {
        self.connections.len() / 2
    }

    fn generate_id(&self) -> String {
        let mut rng = rand::rng();
        loop {
            let id: u32 = rng.random_range(100_000..1_000_000);
            let id_str = id.to_string();
            if !self.sessions.contains_key(&id_str) {
                return id_str;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn register_and_unregister() {
        let mgr = SessionManager::new();
        let (tx, _rx) = mpsc::channel(1);
        let id = mgr.register(tx);
        assert_eq!(id.len(), 6);
        assert!(mgr.session_exists(&id));
        mgr.unregister(&id);
        assert!(!mgr.session_exists(&id));
    }

    #[tokio::test]
    async fn connection_tracking() {
        let mgr = SessionManager::new();
        mgr.add_connection("a", "b");
        assert_eq!(mgr.get_peer("a").as_deref(), Some("b"));
        assert_eq!(mgr.get_peer("b").as_deref(), Some("a"));
        assert_eq!(mgr.active_connections(), 1);

        let peer = mgr.remove_connection("a");
        assert_eq!(peer.as_deref(), Some("b"));
        assert!(mgr.get_peer("a").is_none());
        assert!(mgr.get_peer("b").is_none());
    }
}
