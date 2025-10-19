use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
};
use crate::{db::DbPool, errors::ServiceError, models::bom, models::prelude::*};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{entity::*, query::*, DbConn};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateBOMCommand {
    pub product_id: Uuid,
    #[validate(length(min = 1))]
    pub name: String, // Name of the BOM
    pub description: Option<String>, // Optional description of the BOM
}

#[async_trait::async_trait]
impl Command for CreateBOMCommand {
    type Result = bom::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let bom = self.create_bom(&db).await.map_err(|e| {
            error!(
                "Transaction failed for creating BOM for product ID {}: {}",
                self.product_id, e
            );
            e
        })?;

        self.log_and_trigger_event(event_sender, &bom).await?;

        Ok(bom)
    }
}

impl CreateBOMCommand {
    async fn create_bom(&self, db: &DbConn) -> Result<bom::Model, ServiceError> {
        let new_bom = bom::ActiveModel {
            number: Set(self.name.clone()),
            groups: Set(Some(self.description.clone().unwrap_or_default())),
            updated_at: Set(Utc::now()),
            valid: Set(true),
            ..Default::default()
        };

        bom::Entity::insert(new_bom)
            .exec_with_returning(db)
            .await
            .map_err(|e| {
                error!(
                    "Failed to create BOM for product ID {}: {}",
                    self.product_id, e
                );
                ServiceError::db_error(e)
            })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        bom: &bom::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "BOM created for product ID: {}. BOM ID: {}",
            self.product_id, bom.id
        );
        event_sender
            .send(Event::BOMCreated(bom.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send BOMCreated event for BOM ID {}: {}",
                    bom.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
