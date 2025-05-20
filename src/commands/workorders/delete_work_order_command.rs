use std::sync::Arc;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use tracing::{info, instrument};

use crate::{
    db::DbPool,
    errors::ServiceError,
    models::work_order_entity,
    events::{Event, EventSender},
    commands::Command,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteWorkOrderCommand {
    pub work_order_id: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteWorkOrderResult {
    pub deleted: bool,
}

#[async_trait]
impl Command for DeleteWorkOrderCommand {
    type Result = DeleteWorkOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();
        work_order_entity::Entity::delete_by_id(self.work_order_id)
            .exec(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        info!("Deleted work order {}", self.work_order_id);
        event_sender
            .send(Event::with_data(format!("work_order_deleted:{}", self.work_order_id)))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(DeleteWorkOrderResult { deleted: true })
    }
}

