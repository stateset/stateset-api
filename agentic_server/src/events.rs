use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    CheckoutStarted { session_id: Uuid },
    CheckoutCompleted { session_id: Uuid, order_id: Uuid },
    OrderCreated { order_id: Uuid },
}

#[derive(Clone)]
pub struct EventSender {
    tx: tokio::sync::mpsc::Sender<Event>,
}

impl EventSender {
    pub fn new(tx: tokio::sync::mpsc::Sender<Event>) -> Self {
        Self { tx }
    }

    pub async fn send(&self, event: Event) {
        if let Err(e) = self.tx.send(event).await {
            eprintln!("Failed to send event: {}", e);
        }
    }
}
