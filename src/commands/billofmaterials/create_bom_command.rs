use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{BOM, NewBOM}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
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
    type Result = BOM;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let bom = conn.transaction(|| {
            self.create_bom(&conn)
        }).map_err(|e| {
            error!("Transaction failed for creating BOM for product ID {}: {}", self.product_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &bom).await?;

        Ok(bom)
    }
}

impl CreateBOMCommand {
    fn create_bom(&self, conn: &PgConnection) -> Result<BOM, ServiceError> {
        let new_bom = NewBOM {
            product_id: self.product_id,
            name: self.name.clone(),
            description: self.description.clone(),
            created_at: Utc::now(),
        };

        diesel::insert_into(boms::table)
            .values(&new_bom)
            .get_result::<BOM>(conn)
            .map_err(|e| {
                error!("Failed to create BOM for product ID {}: {}", self.product_id, e);
                ServiceError::DatabaseError(format!("Failed to create BOM: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, bom: &BOM) -> Result<(), ServiceError> {
        info!("BOM created for product ID: {}. BOM ID: {}", self.product_id, bom.id);
        event_sender.send(Event::BOMCreated(bom.id))
            .await
            .map_err(|e| {
                error!("Failed to send BOMCreated event for BOM ID {}: {}", bom.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
