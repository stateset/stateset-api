use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use tokio::sync::broadcast;
use std::sync::Arc;
use tracing::{info, error, warn};
use futures::future::{join_all, BoxFuture};

// Define the various events that can occur in the system.
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
    InventoryAdjusted { product_id: i32, adjustment: i32 },
    WorkOrderCreated(i32),
    WorkOrderCompleted(i32),
}

// Define a trait for handling events. Handlers implementing this trait will process events asynchronously.
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: Event) -> Result<(), String>;
}

// Function to process incoming events and distribute them to registered event handlers.
pub async fn process_events(
    mut rx: broadcast::Receiver<Event>,
    handlers: Vec<Arc<dyn EventHandler>>,
) {
    while let Ok(event) = rx.recv().await {
        info!("Received event: {:?}", event);
        let mut tasks: Vec<BoxFuture<'_, ()>> = Vec::with_capacity(handlers.len());

        for handler in &handlers {
            let event_clone = event.clone();
            let handler_clone = Arc::clone(handler);

            // Push each handler's task to be executed concurrently.
            tasks.push(Box::pin(async move {
                match handler_clone.handle_event(event_clone).await {
                    Ok(_) => info!("Event handled successfully by {:?}", std::any::type_name::<Arc<dyn EventHandler>>()),
                    Err(e) => error!("Error handling event {:?}: {}", event_clone, e),
                }
            }));
        }

        // Wait for all handlers to process the event concurrently.
        join_all(tasks).await;

        info!("Event {:?} processed by all handlers", event);
    }

    warn!("Event processing loop has ended.");
}
