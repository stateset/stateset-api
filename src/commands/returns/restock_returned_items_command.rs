use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::info;
use chrono::{DateTime, Utc};


#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RestockReturnedItemsCommand {
    pub return_id: i32,
}

#[async_trait]
impl Command for RestockReturnedItemsCommand {
    type Result = ();

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Get the returned items
        let returned_items = get_returned_items(self.return_id, &conn)?;

        // Begin transaction to ensure atomicity
        conn.transaction(|| {
            // Restock each item
            for item in returned_items {
                diesel::update(inventory::table.find(item.product_id))
                    .set(inventory::quantity.eq(inventory::quantity + item.quantity))
                    .execute(&conn)
                    .map_err(|e| ServiceError::DatabaseError)?;
            }

            // Log and trigger events
            info!("Returned items restocked for return ID: {}", self.return_id);
            event_sender.send(Event::InventoryAdjusted(self.return_id, 0)).await.map_err(|e| ServiceError::EventError(e.to_string()))?;

            Ok(())
        })
    }
}
