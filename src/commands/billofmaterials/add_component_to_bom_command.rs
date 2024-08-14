use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{BOMComponent, NewBOMComponent}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddComponentToBOMCommand {
    pub bom_id: i32,
    pub component_id: i32, // ID of the component to add
    #[validate(range(min = 1))]
    pub quantity: i32, // Quantity of the component
}

#[async_trait::async_trait]
impl Command for AddComponentToBOMCommand {
    type Result = BOMComponent;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let component = conn.transaction(|| {
            self.add_component(&conn)
        }).map_err(|e| {
            error!("Transaction failed for adding component {} to BOM ID {}: {}", self.component_id, self.bom_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &component).await?;

        Ok(component)
    }
}

impl AddComponentToBOMCommand {
    fn add_component(&self, conn: &PgConnection) -> Result<BOMComponent, ServiceError> {
        let new_component = NewBOMComponent {
            bom_id: self.bom_id,
            component_id: self.component_id,
            quantity: self.quantity,
            created_at: Utc::now(),
        };

        diesel::insert_into(bom_components::table)
            .values(&new_component)
            .get_result::<BOMComponent>(conn)
            .map_err(|e| {
                error!("Failed to add component {} to BOM ID {}: {}", self.component_id, self.bom_id, e);
                ServiceError::DatabaseError(format!("Failed to add component: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, component: &BOMComponent) -> Result<(), ServiceError> {
        info!("Component ID: {} added to BOM ID: {}", self.component_id, self.bom_id);
        event_sender.send(Event::ComponentAddedToBOM(self.bom_id, component.id))
            .await
            .map_err(|e| {
                error!("Failed to send ComponentAddedToBOM event for BOM ID {}: {}", self.bom_id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
