use axum::extract::ws::Message;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct Session {
    #[allow(dead_code)]
    pub id: String,
    pub tx: mpsc::Sender<Message>,
}

impl Session {
    pub fn new(id: String, tx: mpsc::Sender<Message>) -> Self {
        Self { id, tx }
    }
}
