use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::work_order_entity};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use sea_orm::{DatabaseConnection, EntityTrait, ActiveModelTrait, Set};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ScheduleWorkOrderCommand {
    pub work_order_id: i32,
    #[validate]
    pub start_date: chrono::NaiveDateTime, // Scheduled start date and time
}

#[async_trait::async_trait]
impl Command for ScheduleWorkOrderCommand {
    type Result = work_order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_work_order = self.schedule_work_order(&db).await.map_err(|e| {
            error!("Transaction failed for scheduling Work Order ID {}: {}", self.work_order_id, e);
            ServiceError::DatabaseError(format!("Failed to schedule Work Order: {}", e))
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl ScheduleWorkOrderCommand {
    async fn schedule_work_order(&self, db: &DatabaseConnection) -> Result<work_order_entity::Model, ServiceError> {
        let target = work_order_entity::Entity::find_by_id(self.work_order_id)
            .one(db)
            .await
            .map_err(|e| {
                error!("Failed to find Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to find Work Order: {}", e))
            })?
            .ok_or_else(|| {
                error!("Work Order ID {} not found", self.work_order_id);
                ServiceError::NotFound(format!("Work Order ID {} not found", self.work_order_id))
            })?;

        let mut active_model = target.into_active_model();
        active_model.start_date = Set(Some(self.start_date));

        active_model.update(db)
            .await
            .map_err(|e| {
                error!("Failed to schedule Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to schedule Work Order: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &work_order_entity::Model) -> Result<(), ServiceError> {
        info!("Work Order ID: {} scheduled for start at: {}", self.work_order_id, self.start_date);
        event_sender.send(Event::WorkOrderScheduled(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderScheduled event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
