use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{work_order_entity, NewWorkOrder}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use sea_orm::{DatabaseConnection, EntityTrait, Set, TransactionTrait, ActiveModelTrait};
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkOrderCommand {
    pub bom_id: i32,
    pub description: String, // Description of the work order
    pub quantity: i32, // Quantity to be produced or assembled
    pub due_date: Option<chrono::NaiveDateTime>, // Optional due date for the work order
}

#[async_trait::async_trait]
impl Command for CreateWorkOrderCommand {
    type Result = work_order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let work_order = db.transaction(|txn| {
            Box::pin(async move {
                self.create_work_order(txn).await
            })
        }).await.map_err(|e| {
            error!("Transaction failed for creating Work Order: {}", e);
            ServiceError::DatabaseError(format!("Transaction failed: {}", e))
        })?;

        self.log_and_trigger_event(event_sender, &work_order).await?;

        Ok(work_order)
    }
}

impl CreateWorkOrderCommand {
    async fn create_work_order(&self, txn: &DatabaseConnection) -> Result<work_order_entity::Model, ServiceError> {
        let new_work_order = work_order_entity::ActiveModel {
            bom_id: Set(self.bom_id),
            description: Set(self.description.clone()),
            quantity: Set(self.quantity),
            due_date: Set(self.due_date),
            created_at: Set(Utc::now()),
            ..Default::default()
        };

        new_work_order.insert(txn).await.map_err(|e| {
            error!("Failed to create Work Order: {}", e);
            ServiceError::DatabaseError(format!("Failed to create Work Order: {}", e))
        })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &work_order_entity::Model) -> Result<(), ServiceError> {
        info!("Work Order created with ID: {}. BOM ID: {}", work_order.id, self.bom_id);
        event_sender.send(Event::WorkOrderCreated(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderCreated event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
