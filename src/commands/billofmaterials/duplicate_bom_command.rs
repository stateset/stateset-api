use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, error, instrument};
use sea_orm::{entity::*, query::*, DbConn, TransactionTrait};
use chrono::Utc;
use validator::Validate;

use crate::errors::ServiceError;
use crate::events::{Event, EventSender};
use crate::db::DbPool;
use crate::models::{bom, bom_component, NewBOM, NewBOMComponent};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DuplicateBOMCommand {
    pub bom_id: i32,
    #[validate(length(min = 1))]
    pub new_name: String, // Name for the duplicated BOM
    pub new_description: Option<String>, // Optional description for the duplicated BOM
}

#[async_trait::async_trait]
impl Command for DuplicateBOMCommand {
    type Result = bom::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let duplicated_bom = db.transaction::<_, ServiceError, _>(|txn| {
            Box::pin(async move {
                let new_bom = self.create_new_bom(txn).await?;
                self.copy_components(txn, new_bom.id).await?;
                Ok(new_bom)
            })
        }).await.map_err(|e| {
            error!("Transaction failed for duplicating BOM ID {}: {}", self.bom_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &duplicated_bom).await?;

        Ok(duplicated_bom)
    }
}

impl DuplicateBOMCommand {
    async fn create_new_bom(&self, txn: &DbConn) -> Result<bom::Model, ServiceError> {
        let original_bom: Option<bom::Model> = bom::Entity::find_by_id(self.bom_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find original BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to find original BOM: {}", e))
            })?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("BOM with ID {} not found", self.bom_id))
            })?;

        let new_bom = bom::ActiveModel {
            product_id: ActiveValue::Set(original_bom.product_id),
            name: ActiveValue::Set(self.new_name.clone()),
            description: ActiveValue::Set(self.new_description.clone()),
            created_at: ActiveValue::Set(Utc::now()),
            ..Default::default()
        };

        new_bom.insert(txn)
            .await
            .map_err(|e| {
                error!("Failed to create duplicated BOM: {}", e);
                ServiceError::DatabaseError(format!("Failed to create duplicated BOM: {}", e))
            })
    }

    async fn copy_components(&self, txn: &DbConn, new_bom_id: i32) -> Result<(), ServiceError> {
        let original_components = bom_component::Entity::find()
            .filter(bom_component::Column::BomId.eq(self.bom_id))
            .all(txn)
            .await
            .map_err(|e| {
                error!("Failed to load components from original BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to load components: {}", e))
            })?;

        let new_components: Vec<NewBOMComponent> = original_components.iter().map(|component| {
            NewBOMComponent {
                bom_id: new_bom_id,
                component_id: component.component_id,
                quantity: component.quantity,
                created_at: Utc::now(),
            }
        }).collect();

        bom_component::Entity::insert_many(new_components)
            .exec(txn)
            .await
            .map_err(|e| {
                error!("Failed to copy components to new BOM ID {}: {}", new_bom_id, e);
                ServiceError::DatabaseError(format!("Failed to copy components: {}", e))
            })?;

        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, bom: &bom::Model) -> Result<(), ServiceError> {
        info!("BOM ID: {} duplicated as BOM ID: {}.", self.bom_id, bom.id);
        event_sender.send(Event::BOMDuplicated(self.bom_id, bom.id))
            .await
            .map_err(|e| {
                error!("Failed to send BOMDuplicated event for new BOM ID {}: {}", bom.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
