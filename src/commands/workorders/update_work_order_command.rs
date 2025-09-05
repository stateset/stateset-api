use uuid::Uuid;
use crate::{
    commands::Command,
    events::{Event, EventSender},
    db::DbPool,
    errors::ServiceError,
    models::work_order_entity,
};
use async_trait::async_trait;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use validator::Validate;
use chrono::NaiveDateTime;
use crate::models::work_order::WorkOrderPriority;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateWorkOrderCommand {
    pub work_order_id: Uuid,
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<WorkOrderPriority>,
    pub due_date: Option<NaiveDateTime>,
    pub assigned_to: Option<Uuid>,
    pub location_id: Option<Uuid>,
    pub equipment_id: Option<Uuid>,
    pub estimated_hours: Option<f32>,
}

#[async_trait::async_trait]
impl Command for UpdateWorkOrderCommand {
    type Result = work_order_entity::Model;
    
    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.clone();
        let updated_work_order = self.update_work_order(&db).await.map_err(|e| {
            error!(
                "Transaction failed for updating Work Order ID {}: {}",
                self.work_order_id, e
            );
            ServiceError::DatabaseError(format!("Failed to update Work Order: {}", e.to_string()))
        })?;
        self.log_and_trigger_event(event_sender, &updated_work_order)
            .await?;
        Ok(updated_work_order)
    }
}

impl UpdateWorkOrderCommand {
    async fn update_work_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<work_order_entity::Model, ServiceError> {
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
        if let Some(title) = &self.title {
            active_model.title = Set(title.clone());
        }
        if let Some(description) = &self.description {
            active_model.description = Set(description.clone());
        }
        if let Some(priority) = self.priority {
            active_model.priority = Set(priority);
        }
        if let Some(due_date) = self.due_date {
            active_model.due_date = Set(Some(due_date));
        }
        if let Some(assigned_to) = self.assigned_to {
            active_model.assigned_to = Set(Some(assigned_to));
        }
        if let Some(location_id) = self.location_id {
            active_model.location_id = Set(Some(location_id));
        }
        if let Some(equipment_id) = self.equipment_id {
            active_model.equipment_id = Set(Some(equipment_id));
        }
        if let Some(estimated_hours) = self.estimated_hours {
            active_model.estimated_hours = Set(Some(estimated_hours));
        }
        
        active_model.update(db).await.map_err(|e| {
            error!(
                "Failed to update Work Order ID {}: {}",
                self.work_order_id, e
            );
            ServiceError::DatabaseError(format!("Failed to update Work Order: {}", e.to_string()))
        })
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: Arc<EventSender>,
        work_order: &work_order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!("Work Order updated with ID: {}", self.work_order_id);
        event_sender
            .send(Event::WorkOrderUpdated(work_order.id))
            .await
            .map_err(|e| {
                error!(
                    "Failed to send WorkOrderUpdated event for Work Order ID {}: {}",
                    work_order.id, e
                );
                ServiceError::EventError(e.to_string())
            })
    }
}