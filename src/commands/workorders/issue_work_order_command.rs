use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{work_order_entity, WorkOrderStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use chrono::Utc;
use sea_orm::{DatabaseConnection, EntityTrait, Set, TransactionTrait, ActiveModelTrait};

#[derive(Debug, Serialize, Deserialize)]
pub struct IssueWorkOrderCommand {
    pub work_order_id: i32,
}

#[async_trait::async_trait]
impl Command for IssueWorkOrderCommand {
    type Result = work_order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();

        let updated_work_order = db.transaction(|txn| {
            Box::pin(async move {
                self.issue_work_order(txn).await
            })
        }).await.map_err(|e| {
            error!("Transaction failed for issuing Work Order ID {}: {}", self.work_order_id, e);
            ServiceError::DatabaseError(format!("Transaction failed: {}", e))
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl IssueWorkOrderCommand {
    async fn issue_work_order(&self, txn: &DatabaseConnection) -> Result<work_order_entity::Model, ServiceError> {
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

        work_order.status = Set(WorkOrderStatus::Issued);
        work_order.issued_at = Set(Some(Utc::now()));

        work_order.update(txn).await.map_err(|e| {
            error!("Failed to issue Work Order ID {}: {}", self.work_order_id, e);
            ServiceError::DatabaseError(format!("Failed to issue Work Order: {}", e))
        })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &work_order_entity::Model) -> Result<(), ServiceError> {
        info!("Work Order ID: {} marked as issued.", self.work_order_id);
        event_sender.send(Event::WorkOrderIssued(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderIssued event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
