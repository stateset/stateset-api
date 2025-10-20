use uuid::Uuid;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{work_order_entity, WorkOrderStatus},
};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct CancelWorkOrderCommand {
    pub work_order_id: Uuid,
    pub reason: String, // Reason for canceling the work order
}

#[async_trait::async_trait]
impl Command for CancelWorkOrderCommand {
    type Result = work_order_entity::Model;
    
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let updated_work_order = db
            .transaction::<_, ServiceError, _>(|txn| {
                Box::pin(async move {
                    let result = self.cancel_work_order(&txn).await;
                    result
                })
            })
            .await?;
        self.log_and_trigger_event(event_sender, &updated_work_order)
            .await?;
        Ok(updated_work_order)
    }
}

impl CancelWorkOrderCommand {
    async fn cancel_work_order(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<work_order_entity::Model, ServiceError> {
        let mut work_order = work_order_entity::Entity::find_by_id(self.work_order_id)
            .one(txn)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("Work Order with ID {} not found", self.work_order_id)))?;
        let mut active_model = work_order.into_active_model();
        active_model.status = Set(work_order_entity::WorkOrderStatus::Cancelled);
        let saved = active_model
            .update(txn)
            .await?;
        Ok(saved)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        work_order: &work_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Work Order ID: {} canceled. Reason: {}",
            self.work_order_id, self.reason
        );
        event_sender
            .send(Event::WorkOrderCancelled(work_order.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderCancelled event for Work Order ID {}: {}",
                    work_order.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}