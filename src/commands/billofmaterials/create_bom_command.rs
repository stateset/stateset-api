use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::bom, models::prelude::*};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use sea_orm::{entity::*, query::*, DbConn};
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateBOMCommand {
    pub product_id: i32,
    #[validate(length(min = 1))]
    pub name: String, // Name of the BOM
    pub description: Option<String>, // Optional description of the BOM
}

#[async_trait::async_trait]
impl Command for CreateBOMCommand {
    type Result = bom::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let bom = self.create_bom(&db).await.map_err(|e| {
            error!("Transaction failed for creating BOM for product ID {}: {}", self.product_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &bom).await?;

        Ok(bom)
    }
}

impl CreateBOMCommand {
    async fn create_bom(&self, db: &DbConn) -> Result<bom::Model, ServiceError> {
        let new_bom = bom::ActiveModel {
            product_id: Set(self.product_id),
            name: Set(self.name.clone()),
            description: Set(self.description.clone()),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default() // Other fields can be filled here as needed
        };

        bom::Entity::insert(new_bom)
            .exec_with_returning(db)
            .await
            .map_err(|e| {
                error!("Failed to create BOM for product ID {}: {}", self.product_id, e);
                ServiceError::DatabaseError(format!("Failed to create BOM: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, bom: &bom::Model) -> Result<(), ServiceError> {
        info!("BOM created for product ID: {}. BOM ID: {}", self.product_id, bom.id);
        event_sender.send(Event::BOMCreated(bom.id))
            .await
            .map_err(|e| {
                error!("Failed to send BOMCreated event for BOM ID {}: {}", bom.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
