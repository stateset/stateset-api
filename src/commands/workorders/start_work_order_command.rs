use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{WorkOrder, WorkOrderStatus}};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct StartWorkOrderCommand {
    pub work_order_id: i32,
}

#[async_trait::async_trait]
impl Command for StartWorkOrderCommand {
    type Result = WorkOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_work_order = conn.transaction(|| {
            self.start_work_order(&conn)
        }).map_err(|e| {
            error!("Transaction failed for starting Work Order ID {}: {}", self.work_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl StartWorkOrderCommand {
    fn start_work_order(&self, conn: &PgConnection) -> Result<WorkOrder, ServiceError> {
        diesel::update(work_orders::table.find(self.work_order_id))
            .set((
                work_orders::status.eq(WorkOrderStatus::InProgress),
                work_orders::start_date.eq(Utc::now()), // Set the actual start time
            ))
            .get_result::<WorkOrder>(conn)
            .map_err(|e| {
                error!("Failed to start Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to start Work Order: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &WorkOrder) -> Result<(), ServiceError> {
        info!("Work Order ID: {} started at: {}", self.work_order_id, work_order.start_date.unwrap());
        event_sender.send(Event::WorkOrderStarted(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderStarted event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
