use uuid::Uuid;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{db::DbPool, errors::ServiceError, models::work_order_entity};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct UnassignWorkOrderCommand {
    pub work_order_id: Uuid,
}

#[async_trait::async_trait]
impl Command for UnassignWorkOrderCommand {
    type Result = work_order_entity::Model;
    
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let updated_work_order = db
            .transaction(|txn| Box::pin(async move { self.unassign_work_order(txn).await }))
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for unassigning Work Order ID {}: {}",
                    self.work_order_id, e
                );
                ServiceError::DatabaseError(format!("Transaction failed: {}", e))
            })?;
        self.log_and_trigger_event(event_sender, &updated_work_order)
            .await?;
        Ok(updated_work_order)
    }
}

impl UnassignWorkOrderCommand {
    async fn unassign_work_order(
        &self,
        txn: &DatabaseConnection,
    ) -> Result<work_order_entity::Model, ServiceError> {
        let mut work_order: work_order_entity::ActiveModel =
            work_order_entity::Entity::find_by_id(self.work_order_id)
                .one(txn)
                .await
                .map_err(|e| {
                    error!("Failed to find Work Order ID {}: {}", self.work_order_id, e);
                    ServiceError::DatabaseError(format!("Failed to find Work Order: {}", e))
                })?
                .ok_or_else(|| {
                    ServiceError::NotFound(format!(
                        "Work Order ID {} not found",
                        self.work_order_id
                    ))
                })?
                .into();
        work_order.assignee_id = Set(None);
        work_order.update(txn).await.map_err(|e| {
            error!(
                "Failed to unassign Work Order ID {}: {}",
                self.work_order_id, e
            );
            ServiceError::DatabaseError(format!("Failed to unassign Work Order: {}", e))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        work_order: &work_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Work Order ID: {} has been unassigned.", self.work_order_id);
        event_sender
            .send(Event::WorkOrderUnassigned(work_order.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderUnassigned event for Work Order ID {}: {}",
                    work_order.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}