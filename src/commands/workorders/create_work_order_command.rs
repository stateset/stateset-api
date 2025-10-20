use uuid::Uuid;
use crate::commands::Command;
use crate::events::{Event, EventSender};
use crate::{
    db::DbPool,
    errors::ServiceError,
    models::{work_order::{WorkOrderPriority}, work_order_entity, NewWorkOrder},
};
use async_trait::async_trait;
use chrono::Utc;
use chrono::DateTime;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkOrderCommand {
    pub bom_id: i32,
    pub title: String,
    pub description: String,
    pub priority: WorkOrderPriority,
    pub due_date: Option<DateTime<Utc>>,
    pub assigned_to: Option<Uuid>,
    pub created_by: Uuid,
    pub related_order_id: Option<Uuid>,
    pub related_return_id: Option<Uuid>,
    pub related_warranty_id: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub equipment_id: Option<Uuid>,
    pub materials: Vec<(Uuid, i32)>,
    pub estimated_hours: Option<f32>,
}

#[async_trait::async_trait]
impl Command for CreateWorkOrderCommand {
    type Result = work_order_entity::Model;
    
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let work_order = match db
            .transaction::<_, ServiceError, _>(|txn| {
                Box::pin(async move {
                    let model = self.create_work_order(&txn).await?;
                    Ok::<_, ServiceError>(model)
                })
            })
            .await
        {
            Ok(model) => model,
            Err(e) => return Err(ServiceError::db_error(e.to_string())),
        };
        self.log_and_trigger_event(event_sender, &work_order)
            .await?;
        Ok(work_order)
    }
}

impl CreateWorkOrderCommand {
    async fn create_work_order(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<work_order_entity::Model, ServiceError> {
        let new_work_order = work_order_entity::ActiveModel {
            title: Set(self.title.clone()),
            description: Set(self.description.clone()),
            priority: Set(self.priority),
            due_date: Set(self.due_date),
            created_at: Set(Utc::now()),
            ..Default::default()
        };
        new_work_order.insert(txn).await.map_err(ServiceError::db_error)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        work_order: &work_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            "Work Order created with ID: {}. BOM ID: {}",
            work_order.id, self.bom_id
        );
        event_sender
            .send(Event::WorkOrderCreated(work_order.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderCreated event for Work Order ID {}: {}",
                    work_order.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}