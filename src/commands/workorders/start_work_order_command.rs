use uuid::Uuid;
use crate::events::{Event, EventSender};
use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    models::{work_order_entity, WorkOrderStatus},
};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, DatabaseTransaction, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct StartWorkOrderCommand {
    pub work_order_id: Uuid,
}

#[async_trait::async_trait]
impl Command for StartWorkOrderCommand {
    type Result = work_order_entity::Model;
    
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let updated_work_order = match db
            .transaction::<_, ServiceError, _>(|txn| {
                Box::pin(async move {
                    let model = self.start_work_order(&txn).await?;
                    Ok::<_, ServiceError>(model)
                })
            })
            .await
        {
            Ok(model) => model,
            Err(e) => return Err(ServiceError::DatabaseError(e.to_string())),
        };
        self.log_and_trigger_event(event_sender, &updated_work_order)
            .await?;
        Ok(updated_work_order)
    }
}

impl StartWorkOrderCommand {
    async fn start_work_order(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<work_order_entity::Model, ServiceError> {
        let mut work_order = work_order_entity::Entity::find_by_id(self.work_order_id)
            .one(txn)
            .await
            .map_err(ServiceError::DatabaseError)?
            .ok_or_else(|| ServiceError::NotFound(format!("Work Order ID {} not found", self.work_order_id)))?;
        let mut active = work_order.into_active_model();
        active.status = Set(WorkOrderStatus::InProgress);
        let saved = active.update(txn).await.map_err(ServiceError::DatabaseError)?;
        Ok(saved)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        work_order: &work_order_entity::Model,
    ) -> Result<(), ServiceError> {
        if let Some(start_date) = work_order.start_date {
            info!(
                "Work Order ID: {} started at: {}",
                self.work_order_id, start_date
            );
        }
        
        event_sender
            .send(Event::WorkOrderStarted(work_order.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderStarted event for Work Order ID {}: {}",
                    work_order.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}