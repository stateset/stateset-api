use async_trait::async_trait;;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{bom_component, NewBOMComponent, BOMComponent}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use chrono::Utc;
use sea_orm::{entity::*, query::*, DbConn, Set};

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
        let db = db_pool.clone();

        let component = db
            .transaction::<_, ServiceError, _>(|txn| Box::pin(self.add_component(txn)))
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for adding component {} to BOM ID {}: {}",
                    self.component_id, self.bom_id, e
                );
                e
            })?;

        self.log_and_trigger_event(event_sender, &component).await?;

        Ok(component)
    }
}

impl AddComponentToBOMCommand {
    async fn add_component(&self, db: &DbConn) -> Result<BOMComponent, ServiceError> {
        let new_component = bom_component::ActiveModel {
            bom_id: Set(self.bom_id),
            component_id: Set(self.component_id),
            quantity: Set(self.quantity),
            created_at: Set(Utc::now().naive_utc()),
            ..Default::default()
        };

        new_component
            .insert(db)
            .await
            .map_err(|e| {
                error!(
                    "Failed to add component {} to BOM ID {}: {}",
                    self.component_id, self.bom_id, e
                );
                ServiceError::DatabaseError(format!("Failed to add component: {}", e))
            })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        component: &BOMComponent,
    ) -> Result<(), ServiceError> {
        info!("Component ID: {} added to BOM ID: {}", self.component_id, self.bom_id);
        event_sender
            .send(Event::ComponentAddedToBOM(self.bom_id, component.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send ComponentAddedToBOM event for BOM ID {}: {}",
                    self.bom_id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}
