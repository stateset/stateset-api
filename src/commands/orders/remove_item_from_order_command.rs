use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{order_item_entity, order_item_entity::Entity as OrderItem}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_ITEMS_REMOVED: IntCounter = 
        IntCounter::new("order_items_removed_total", "Total number of items removed from orders")
            .expect("metric can be created");

    static ref ORDER_ITEM_REMOVE_FAILURES: IntCounter = 
        IntCounter::new("order_item_remove_failures_total", "Total number of failed item removals from orders")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveItemFromOrderCommand {
    pub order_id: i32,
    pub item_id: i32,
}

#[async_trait]
impl Command for RemoveItemFromOrderCommand {
    type Result = ();

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            ORDER_ITEM_REMOVE_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Attempt to delete the item from the order_items table
        let delete_result = OrderItem::delete_many()
            .filter(
                Condition::all()
                    .add(order_item_entity::Column::Id.eq(self.item_id))
                    .add(order_item_entity::Column::OrderId.eq(self.order_id))
            )
            .exec(&db)
            .await
            .map_err(|e| {
                ORDER_ITEM_REMOVE_FAILURES.inc();
                error!("Failed to remove item {} from order {}: {}", self.item_id, self.order_id, e);
                ServiceError::DatabaseError
            })?;

        if delete_result.rows_affected == 0 {
            ORDER_ITEM_REMOVE_FAILURES.inc();
            error!(
                "Item {} not found in order {}. No rows were deleted.",
                self.item_id, self.order_id
            );
            return Err(ServiceError::NotFound(format!("Item {} not found in order {}", self.item_id, self.order_id)));
        }

        // Trigger an event indicating that an item was removed from the order
        if let Err(e) = event_sender.send(Event::OrderItemRemoved(self.order_id, self.item_id)).await {
            ORDER_ITEM_REMOVE_FAILURES.inc();
            error!("Failed to send OrderItemRemoved event for order {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_ITEMS_REMOVED.inc();

        info!(
            order_id = %self.order_id,
            item_id = %self.item_id,
            "Item removed from order successfully"
        );

        Ok(())
    }
}