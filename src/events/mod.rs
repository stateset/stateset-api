use serde::{Serialize, Deserialize};
use async_trait::async_trait;;
use tokio::sync::broadcast;
use std::sync::Arc;
use tracing::{info, error};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Event {
    OrderCreated(i32),
    OrderUpdated(i32),
    OrderCancelled(i32),
    ReturnCreated(i32),
    ReturnProcessed(i32),
    WarrantyClaimed(i32),
    ShipmentCreated(i32),
    ShipmentUpdated(i32),
    InventoryAdjusted(i32, i32),
    WorkOrderCreated(i32),
    WorkOrderCompleted(i32),
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: Event) -> Result<(), String>;
}

pub async fn process_events(mut rx: broadcast::Receiver<Event>, handlers: Vec<Arc<dyn EventHandler>>) {
    while let Ok(event) = rx.recv().await {
        let mut tasks = vec![];

        for handler in &handlers {
            let event_clone = event.clone();
            let handler_clone = Arc::clone(handler);

            tasks.push(tokio::spawn(async move {
                if let Err(e) = handler_clone.handle_event(event_clone).await {
                    error!("Error handling event {:?}: {}", event_clone, e);
                }
            }));
        }

        // Wait for all handlers to process the event
        futures::future::join_all(tasks).await;

        info!("Event {:?} processed by all handlers", event);
    }
}
