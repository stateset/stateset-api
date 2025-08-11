use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        inventory_item_entity::{self, Entity as Inventory},
        return_item_entity::{self, Entity as ReturnedItem},
    },
    proto::return_order::ReturnItem,
};
use chrono::Utc;
use sea_orm::{*, Set, TransactionError, TransactionTrait, EntityTrait, ColumnTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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

        let items_restocked = db
            .transaction::<_, usize, ServiceError>(|txn| {
                Box::pin(async move {
                    let items = self.fetch_returned_items(txn).await?;
                    let count = items.len();
                    self.restock_items(txn, items).await?;
                    Ok(count)
                })
            })
            .await
            .map_err(|e| match e {
                TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                TransactionError::Transaction(service_err) => service_err,
            })?;

        info!(
            return_id = %self.return_id,
            items_restocked = %items_restocked,
            "Items restocked for return"
        );

        event_sender
            .send(Event::InventoryUpdated {
                item_id: self.return_id,
                quantity: items_restocked as i32,
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send inventory adjusted event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;

        Ok(RestockReturnedItemsResult {
            return_id: self.return_id,
            items_restocked,
        })
    }
}

impl RestockReturnedItemsCommand {
    async fn fetch_returned_items(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<return_item_entity::Model>, ServiceError> {
        ReturnedItem::find()
            .filter(return_item_entity::Column::ReturnId.eq(self.return_id))
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch returned items: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(e)
            })
    }

    async fn restock_items(
        &self,
        txn: &DatabaseTransaction,
        items: Vec<return_item_entity::Model>,
    ) -> Result<(), ServiceError> {
        for item in items {
            let inventory = Inventory::find_by_id(item.product_id)
                .one(txn)
                .await
                .map_err(|e| {
                    let msg = format!("Failed to fetch inventory: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(e)
                })?
                .ok_or_else(|| {
                    let msg = format!("Inventory for product {} not found", item.product_id);
                    error!("{}", msg);
                    ServiceError::NotFound(msg)
                })?;

            let mut inventory: inventory_item_entity::ActiveModel = inventory.into();
            let current_quantity = inventory.quantity.clone().unwrap_or(Set(0));
            inventory.quantity = Set(current_quantity + item.quantity);

            inventory.update(txn).await.map_err(|e| {
                let msg = format!("Failed to update inventory: {}", e);
                error!("{}", msg);
                ServiceError::DatabaseError(e)
            })?;
        }
        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        items_restocked: usize,
    ) -> Result<(), ServiceError> {
        info!(
            "Returned items restocked for return ID: {}. Items restocked: {}",
            self.return_id, items_restocked
        );
        event_sender
            .send(Event::InventoryAdjusted {
                product_id: self.return_id, // This should be the actual product ID in a real implementation
                adjustment: items_restocked as i32,
            })
            .await
            .map_err(|e| {
                let msg = format!("Failed to send event for restocked items: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
