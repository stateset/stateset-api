use uuid::Uuid;
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{entity::*, query::*, DbConn, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

use crate::commands::Command;
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::events::{Event, EventSender};
use crate::models::{billofmaterials as bom, bom_line_item};

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
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let duplicated_bom = db
            .transaction::<_, ServiceError, _>(|txn| {
                Box::pin(async move {
                    let new_bom = self.create_new_bom(txn).await?;
                    self.copy_components(txn, new_bom.id).await?;
                    Ok(new_bom)
                })
            })
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for duplicating BOM ID {}: {}",
                    self.bom_id, e
                );
                e
            })?;

        self.log_and_trigger_event(event_sender, &duplicated_bom)
            .await?;

        Ok(duplicated_bom)
    }
}

impl DuplicateBOMCommand {
    async fn create_new_bom(&self, txn: &DbConn) -> Result<bom::Model, ServiceError> {
        let original_bom = bom::Entity::find_by_id(self.bom_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find original BOM with ID {}: {}", self.bom_id, e);
                ServiceError::db_error(e)
            })?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("BOM with ID {} not found", self.bom_id))
            })?;

        let new_bom = bom::ActiveModel {
            number: Set(format!("{}_copy", original_bom.number)),
            groups: Set(original_bom.groups),
            updated_at: Set(Utc::now()),
            valid: Set(original_bom.valid),
            ..Default::default()
        };

        new_bom.insert(txn).await.map_err(|e| {
            error!("Failed to create duplicated BOM: {}", e);
            ServiceError::db_error(e)
        })
    }

    async fn copy_components(&self, txn: &DbConn, new_bom_id: i32) -> Result<(), ServiceError> {
        let original_components = bom_line_item::Entity::find()
            .filter(bom_line_item::Column::BillOfMaterialsId.eq(self.bom_id))
            .all(txn)
            .await
            .map_err(|e| {
                error!(
                    "Failed to load components from original BOM ID {}: {}",
                    self.bom_id, e
                );
                ServiceError::db_error(e)
            })?;

        for component in original_components {
            let new_component = bom_line_item::ActiveModel {
                bill_of_materials_number: Set(new_bom_id.to_string()),
                line_type: Set(component.line_type),
                part_number: Set(component.part_number),
                part_name: Set(component.part_name),
                purchase_supply_type: Set(component.purchase_supply_type),
                quantity: Set(component.quantity),
                status: Set(component.status),
                bill_of_materials_id: Set(new_bom_id),
                created_at: Set(Utc::now()),
                updated_at: Set(Utc::now()),
                ..Default::default()
            };
            new_component.insert(txn).await.map_err(|e| {
                error!(
                    "Failed to copy component to new BOM ID {}: {}",
                    new_bom_id, e
                );
                ServiceError::db_error(e)
            })?;
        }

        Ok(())
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        bom: &bom::Model,
    ) -> Result<(), ServiceError> {
        info!("BOM ID: {} duplicated as BOM ID: {}.", self.bom_id, bom.id);
        event_sender
            .send(Event::BOMDuplicated(self.bom_id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send BOMDuplicated event for new BOM ID {}: {}",
                    bom.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
