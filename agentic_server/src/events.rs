use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    CheckoutStarted { session_id: Uuid },
    CheckoutCompleted { session_id: Uuid, order_id: Uuid },
    OrderCreated { order_id: Uuid },
    PurchaseOrderDrafted { product_id: String, quantity: u32, reason: String },
    QualityAlert { product_id: String, return_id: String, reason: String },
    CartRecoveryMessageSent { session_id: String, email: String, message: String },
}

#[derive(Clone)]
pub struct EventSender {
    tx: tokio::sync::mpsc::Sender<Event>,
}

impl EventSender {
    pub fn new(tx: tokio::sync::mpsc::Sender<Event>) -> Self {
        Self { tx }
    }

    pub async fn send(&self, event: Event) -> Result<(), anyhow::Error> {
        self.tx.send(event).await?;
        Ok(())
    }
}
