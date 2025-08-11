use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{bom, prelude::*},
};
use async_trait::async_trait;
use sea_orm::{entity::*, query::*, DbConn, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateBOMCommand {
    pub bom_id: i32,
    #[validate(length(min = 1))]
    pub name: Option<String>, // Optional new name for the BOM
    pub description: Option<String>, // Optional new description for the BOM
}

#[async_trait::async_trait]
impl Command for UpdateBOMCommand {
    type Result = bom::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_bom = db
            .transaction::<_, ServiceError, _>(|txn| {
                Box::pin(async move { self.update_bom(txn).await })
            })
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for updating BOM ID {}: {}",
                    self.bom_id, e
                );
                e
            })?;

        self.log_and_trigger_event(event_sender, &updated_bom)
            .await?;

        Ok(updated_bom)
    }
}

impl UpdateBOMCommand {
    async fn update_bom(&self, txn: &DbConn) -> Result<bom::Model, ServiceError> {
        let mut update_data = bom::ActiveModel {
            id: ActiveValue::Unchanged(self.bom_id),
            ..Default::default()
        };

        if let Some(ref name) = self.name {
            update_data.name = ActiveValue::Set(name.clone());
        }

        if let Some(ref description) = self.description {
            update_data.description = ActiveValue::Set(Some(description.clone()));
        }

        update_data.update(txn).await.map_err(|e| {
            error!("Failed to update BOM ID {}: {}", self.bom_id, e);
            ServiceError::DatabaseError(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        bom: &bom::Model,
    ) -> Result<(), ServiceError> {
        info!("BOM updated for BOM ID: {}", self.bom_id);
        event_sender
            .send(Event::BOMCreated(bom.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send BOMUpdated event for BOM ID {}: {}",
                    bom.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
