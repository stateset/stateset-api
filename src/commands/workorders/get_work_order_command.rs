use std::sync::Arc;
use serde::{Deserialize, Serialize};
use validator::Validate;
use async_trait::async_trait;
use tracing::{info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    models::work_order_entity,
    events::{Event, EventSender},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GetWorkOrderCommand {
    pub work_order_id: i32,
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
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?
            .ok_or_else(|| ServiceError::NotFoundError(format!("Work order {} not found", self.work_order_id)))?;

        info!("Fetched work order {}", self.work_order_id);
        event_sender
            .send(Event::WorkOrderUpdated(work_order.id))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(work_order)
    }
}

