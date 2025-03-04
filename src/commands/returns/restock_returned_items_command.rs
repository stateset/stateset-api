use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        return_entity,
        returned_item_entity::{self, Entity as ReturnedItem},
        inventory_entity::{self, Entity as Inventory},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct RestockReturnedItemsCommand {
    pub return_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestockReturnedItemsResult {
    pub return_id: Uuid,
    pub items_restocked: usize,
}

#[async_trait::async_trait]
impl Command for RestockReturnedItemsCommand {
    type Result = RestockReturnedItemsResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();

        let returned_items = self.get_returned_items(db).await?;

        let items_restocked = db
            .transaction::<_, usize, ServiceError>(|txn| {
                Box::pin(async move {
                    let count = self.restock_items(txn, &returned_items).await?;
                    self.log_and_trigger_event(&event_sender, count).await?;
                    Ok(count)
                })
            })
            .await?;

        Ok(RestockReturnedItemsResult {
            return_id: self.return_id,
            items_restocked,
        })
    }
}

impl RestockReturnedItemsCommand {
    async fn get_returned_items(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<returned_item_entity::Model>, ServiceError> {
        ReturnedItem::find()
            .filter(returned_item_entity::Column::ReturnId.eq(self.return_id))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch returned items: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })
    }

    async fn restock_items(
        &self,
        db: &DatabaseConnection,
        items: &[returned_item_entity::Model],
    ) -> Result<usize, ServiceError> {
        let mut restocked_count = 0;

        for item in items {
            let inventory = Inventory::find_by_id(item.product_id)
                .one(db)
                .await
                .map_err(|e| {
                    let msg = format!("Failed to fetch inventory for product ID {}: {}", item.product_id, e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?
                .ok_or_else(|| {
                    let msg = format!("Inventory not found for product ID: {}", item.product_id);
                    error!("{}", msg);
                    ServiceError::NotFound(msg)
                })?;

            let mut inventory: inventory_entity::ActiveModel = inventory.into();
            inventory.quantity = Set(inventory.quantity.unwrap() + item.quantity);

            inventory.update(db).await.map_err(|e| {
                let msg = format!("Failed to restock item ID {}: {}", item.product_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;

            restocked_count += 1;
        }

        Ok(restocked_count)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        items_restocked: usize,
    ) -> Result<(), ServiceError> {
        info!("Returned items restocked for return ID: {}. Items restocked: {}", self.return_id, items_restocked);
        event_sender
            .send(Event::InventoryAdjusted { 
                product_id: self.return_id, // This should be the actual product ID in a real implementation
                adjustment: items_restocked as i32 
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for restocked items: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}