use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{work_order_entity, WorkOrderStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use sea_orm::{DatabaseConnection, EntityTrait, Set, TransactionTrait, ActiveModelTrait};
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
pub struct StartWorkOrderCommand {
    pub work_order_id: i32,
}

#[async_trait::async_trait]
impl Command for StartWorkOrderCommand {
    type Result = work_order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_work_order = db.transaction(|txn| {
            Box::pin(async move {
                self.start_work_order(txn).await
            })
        }).await.map_err(|e| {
            error!("Transaction failed for starting Work Order ID {}: {}", self.work_order_id, e);
            ServiceError::DatabaseError(format!("Transaction failed: {}", e))
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl StartWorkOrderCommand {
    async fn start_work_order(&self, txn: &DatabaseConnection) -> Result<work_order_entity::Model, ServiceError> {
        let mut work_order: work_order_entity::ActiveModel = work_order_entity::Entity::find_by_id(self.work_order_id)
            .one(txn)
            .await
            .map_err(|e| {
                error!("Failed to find Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to find Work Order: {}", e))
            })?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work Order ID {} not found", self.work_order_id))
            })?
            .into();

        work_order.status = Set(WorkOrderStatus::InProgress);
        work_order.start_date = Set(Some(Utc::now()));

        work_order.update(txn).await.map_err(|e| {
            error!("Failed to start Work Order ID {}: {}", self.work_order_id, e);
            ServiceError::DatabaseError(format!("Failed to start Work Order: {}", e))
        })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &work_order_entity::Model) -> Result<(), ServiceError> {
        if let Some(start_date) = work_order.start_date {
            info!("Work Order ID: {} started at: {}", self.work_order_id, start_date);
        }
        event_sender.send(Event::WorkOrderStarted(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderStarted event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
