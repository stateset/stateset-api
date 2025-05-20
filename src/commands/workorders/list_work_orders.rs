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
pub struct ListWorkOrdersCommand;

#[async_trait]
impl Command for ListWorkOrdersCommand {
    type Result = Vec<work_order_entity::Model>;

    #[instrument(skip(_self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        let db = db_pool.as_ref();
        let orders = work_order_entity::Entity::find()
            .all(db)
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        info!("Listed {} work orders", orders.len());
        event_sender
            .send(Event::with_data("work_orders_listed".to_string()))
            .await
            .map_err(ServiceError::EventError)?;

        Ok(orders)
    }
}

