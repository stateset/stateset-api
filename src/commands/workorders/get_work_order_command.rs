use async_trait::async_trait;
use sea_orm::EntityTrait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use uuid::Uuid;
use validator::Validate;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::work_order_entity,
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GetWorkOrderCommand {
    pub work_order_id: Uuid,
}

#[async_trait]
impl Command for GetWorkOrderCommand {
    type Result = work_order_entity::Model;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();
        let work_order = work_order_entity::Entity::find_by_id(self.work_order_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Work order {} not found", self.work_order_id))
            })?;

        info!("Fetched work order {}", self.work_order_id);

        event_sender
            .send(Event::WorkOrderUpdated(work_order.id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(work_order)
    }
}
