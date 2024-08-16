use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{return_entity, returned_item_entity, inventory_entity}};
use crate::models::return_entity::ReturnStatus;
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RestockReturnedItemsCommand {
    pub return_id: i32,
}

#[async_trait]
impl Command for RestockReturnedItemsCommand {
    type Result = ();

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let returned_items = self.get_returned_items(&db).await?;

        db.transaction::<_, (), ServiceError>(|txn| {
            Box::pin(async move {
                self.restock_items(txn, &returned_items).await?;
                self.log_and_trigger_event(event_sender).await?;
                Ok(())
            })
        }).await?;

        Ok(())
    }
}

impl RestockReturnedItemsCommand {
    async fn get_returned_items(&self, db: &DatabaseConnection) -> Result<Vec<returned_item_entity::Model>, ServiceError> {
        returned_item_entity::Entity::find()
            .filter(returned_item_entity::Column::ReturnId.eq(self.return_id))
            .all(db)
            .await
            .map_err(|e| {
                error!("Failed to fetch returned items: {}", e);
                ServiceError::DatabaseError(format!("Failed to fetch returned items: {}", e))
            })
    }

    async fn restock_items(&self, db: &DatabaseConnection, items: &[returned_item_entity::Model]) -> Result<(), ServiceError> {
        for item in items {
            let inventory = inventory_entity::Entity::find_by_id(item.product_id)
                .one(db)
                .await
                .map_err(|e| {
                    error!("Failed to fetch inventory for product ID {}: {}", item.product_id, e);
                    ServiceError::DatabaseError(format!("Failed to fetch inventory: {}", e))
                })?
                .ok_or_else(|| {
                    error!("Inventory not found for product ID: {}", item.product_id);
                    ServiceError::NotFound(format!("Inventory not found for product ID {}", item.product_id))
                })?;

            let mut inventory: inventory_entity::ActiveModel = inventory.into();
            inventory.quantity = Set(inventory.quantity.unwrap() + item.quantity);

            inventory.update(db).await.map_err(|e| {
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