use uuid::Uuid;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument};
use sea_orm::EntityTrait;

use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::work_order_entity,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteWorkOrderCommand {
    pub work_order_id: Uuid,
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
            .map_err(|e| ServiceError::DatabaseError(e))?;
        
        info!("Deleted work order {}", self.work_order_id);
        
        event_sender
            .send(Event::WorkOrderUpdated(self.work_order_id))
            .await
            .map_err(ServiceError::EventError)?;
        
        Ok(DeleteWorkOrderResult { deleted: true })
    }
}