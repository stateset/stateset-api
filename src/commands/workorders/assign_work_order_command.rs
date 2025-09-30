use uuid::Uuid;
use sea_orm::DatabaseTransaction;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{db::DbPool, errors::ServiceError, models::work_order_entity};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct AssignWorkOrderCommand {
    pub work_order_id: Uuid,
    pub assignee_id: i32, // ID of the worker or team to whom the work order is assigned
}

#[async_trait::async_trait]
impl Command for AssignWorkOrderCommand {
    type Result = work_order_entity::Model;
    
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let updated_work_order = db
            .transaction(|txn| Box::pin(async move { self.assign_work_order(txn).await }))
            .await
            .map_err(|e| {
                error!(
                    "Transaction failed for assigning Work Order ID {}: {}",
                    self.work_order_id, e
                );
                match e {
                    sea_orm::TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
                    sea_orm::TransactionError::Transaction(service_err) => service_err,
                }
            })?;
        self.log_and_trigger_event(event_sender, &updated_work_order)
            .await?;
        Ok(updated_work_order)
    }
}

impl AssignWorkOrderCommand {
    async fn assign_work_order(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<work_order_entity::Model, ServiceError> {
        let mut work_order: work_order_entity::ActiveModel =
            work_order_entity::Entity::find_by_id(self.work_order_id)
                .one(txn)
                .await
                .map_err(|e| ServiceError::DatabaseError(e))?
                .ok_or_else(|| {
                    ServiceError::NotFound(format!(
                        "Work Order ID {} not found",
                        self.work_order_id
                    ))
                })?
                .into();
        work_order.assignee_id = Set(Some(self.assignee_id));
        work_order.update(txn).await.map_err(|e| {
            error!(
                "Failed to assign Work Order ID {}: {}",
                self.work_order_id, e
            );
            ServiceError::DatabaseError(e)
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        work_order: &work_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Work Order ID: {} assigned to Assignee ID: {}",
            self.work_order_id, self.assignee_id
        );
        event_sender
            .send(Event::WorkOrderUpdated(work_order.id))
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))
    }
}
