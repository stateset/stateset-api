use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use tokio::sync::broadcast;
use std::sync::Arc;
use tracing::{info, error, warn};
use futures::future::{join_all, BoxFuture};

pub type EventSender = broadcast::Sender<Event>;

// Define the various events that can occur in the system.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Event {
    BOMDeleted(i32),
    BOMUpdated(i32),
    BOMCreated(i32),
    OrderCreated(Uuid),
    OrderUpdated(Uuid),
    OrderCancelled(Uuid),
    OrderDeleted(Uuid),
    OrderCompleted(Uuid),
    OrderRefunded(Uuid),
    OrdersMerged(Uuid),
    OrderTagged(Uuid),
    OrderSplit(Uuid),
    OrderExchanged(Uuid),
    OrderReleasedFromHold(Uuid),
    OrderOnHold(Uuid),
    OrderShipped(Uuid),
    OrderItemAdded(Uuid),
    ReturnCreated(Uuid),
    ReturnProcessed(Uuid),
    ReturnInitiated(Uuid),
    ReturnCancelled(Uuid),
    ReturnClosed(Uuid),
    ReturnDeleted(Uuid),
    ReturnCompleted(Uuid),
    ReturnRefunded(Uuid),
    ReturnApproved(Uuid),
    ReturnRejected(Uuid),
    WarrantyClaimed(Uuid),
    ShipmentCreated(Uuid),
    ShipmentCancelled(Uuid),
    ShipmentUpdated(Uuid),
    ShipmentTracked(Uuid),
    InventoryAdjusted { product_id: Uuid, adjustment: i32 },
    WorkOrderCreated(Uuid),
    WorkOrderStarted(Uuid),
    WorkOrderUnassigned(Uuid),
    WorkOrderAssigned(Uuid),
    WorkOrderCancelled(Uuid),
    WorkOrderUpdated(Uuid),
    WorkOrderCompleted(Uuid),
    WorkOrderIssued(Uuid),
    WorkOrderPicked(Uuid),
    WorkOrderUpdatedYielded(Uuid),
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

pub type EventSender = broadcast::Sender<Event>;