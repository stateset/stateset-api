use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Return, ReturnStatus, ReturnedItem}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use async_trait::async_trait;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RestockReturnedItemsCommand {
    pub return_id: i32,
}

#[async_trait::async_trait]
impl Command for RestockReturnedItemsCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let returned_items = self.get_returned_items(&conn)?;

        conn.transaction(|| {
            self.restock_items(&conn, &returned_items)?;
            self.log_and_trigger_event(event_sender).await
        }).map_err(|e| {
            error!("Transaction failed for restocking items for return ID {}: {}", self.return_id, e);
            e
        })?;

        Ok(())
    }
}

impl RestockReturnedItemsCommand {
    fn get_returned_items(&self, conn: &PgConnection) -> Result<Vec<ReturnedItem>, ServiceError> {
        // Fetch the returned items for the given return ID
        // Replace with actual query to get items
        Ok(vec![]) // Placeholder: Replace with actual query to fetch returned items
    }

    fn restock_items(&self, conn: &PgConnection, items: &[ReturnedItem]) -> Result<(), ServiceError> {
        for item in items {
            diesel::update(inventory::table.find(item.product_id))
                .set(inventory::quantity.eq(inventory::quantity + item.quantity))
                .execute(conn)
                .map_err(|e| {
                    error!("Failed to restock item ID {}: {}", item.product_id, e);
                    ServiceError::DatabaseError(format!("Failed to restock item: {}", e))
                })?;
        }
        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>) -> Result<(), ServiceError> {
        info!("Returned items restocked for return ID: {}", self.return_id);
        event_sender.send(Event::InventoryAdjusted(self.return_id, 0))
            .await
            .map_err(|e| {
                error!("Failed to send InventoryAdjusted event for return ID {}: {}", self.return_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
