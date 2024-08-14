use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{BOM, BOMComponent, NewBOM}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct DuplicateBOMCommand {
    pub bom_id: i32,
    #[validate(length(min = 1))]
    pub new_name: String, // Name for the duplicated BOM
    pub new_description: Option<String>, // Optional description for the duplicated BOM
}

#[async_trait::async_trait]
impl Command for DuplicateBOMCommand {
    type Result = BOM;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let duplicated_bom = conn.transaction(|| {
            let new_bom = self.create_new_bom(&conn)?;
            self.copy_components(&conn, new_bom.id)?;
            Ok(new_bom)
        }).map_err(|e| {
            error!("Transaction failed for duplicating BOM ID {}: {}", self.bom_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &duplicated_bom).await?;

        Ok(duplicated_bom)
    }
}

impl DuplicateBOMCommand {
    fn create_new_bom(&self, conn: &PgConnection) -> Result<BOM, ServiceError> {
        let original_bom: BOM = boms::table.find(self.bom_id).first(conn).map_err(|e| {
            error!("Failed to find original BOM ID {}: {}", self.bom_id, e);
            ServiceError::DatabaseError(format!("Failed to find original BOM: {}", e))
        })?;

        let new_bom = NewBOM {
            product_id: original_bom.product_id,
            name: self.new_name.clone(),
            description: self.new_description.clone(),
            created_at: Utc::now(),
        };

        diesel::insert_into(boms::table)
            .values(&new_bom)
            .get_result::<BOM>(conn)
            .map_err(|e| {
                error!("Failed to create duplicated BOM: {}", e);
                ServiceError::DatabaseError(format!("Failed to create duplicated BOM: {}", e))
            })
    }

    fn copy_components(&self, conn: &PgConnection, new_bom_id: i32) -> Result<(), ServiceError> {
        let original_components = bom_components::table.filter(bom_components::bom_id.eq(self.bom_id))
            .load::<BOMComponent>(conn).map_err(|e| {
                error!("Failed to load components from original BOM ID {}: {}", self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to load components: {}", e))
            })?;

        for component in original_components {
            let new_component = NewBOMComponent {
                bom_id: new_bom_id,
                component_id: component.component_id,
                quantity: component.quantity,
                created_at: Utc::now(),
            };

            diesel::insert_into(bom_components::table)
                .values(&new_component)
                .execute(conn)
                .map_err(|e| {
                    error!("Failed to copy component ID {} to new BOM ID {}: {}", component.component_id, new_bom_id, e);
                    ServiceError::DatabaseError(format!("Failed to copy component: {}", e))
                })?;
        }

        Ok(())
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, bom: &BOM) -> Result<(), ServiceError> {
        info!("BOM ID: {} duplicated as BOM ID: {}.", self.bom_id, bom.id);
        event_sender.send(Event::BOMDuplicated(self.bom_id, bom.id))
            .await
            .map_err(|e| {
                error!("Failed to send BOMDuplicated event for new BOM ID {}: {}", bom.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
