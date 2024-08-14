use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::WorkOrder};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ScheduleWorkOrderCommand {
    pub work_order_id: i32,
    #[validate]
    pub start_date: NaiveDateTime, // Scheduled start date and time
}

#[async_trait::async_trait]
impl Command for ScheduleWorkOrderCommand {
    type Result = WorkOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_work_order = conn.transaction(|| {
            self.schedule_work_order(&conn)
        }).map_err(|e| {
            error!("Transaction failed for scheduling Work Order ID {}: {}", self.work_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl ScheduleWorkOrderCommand {
    fn schedule_work_order(&self, conn: &PgConnection) -> Result<WorkOrder, ServiceError> {
        diesel::update(work_orders::table.find(self.work_order_id))
            .set(work_orders::start_date.eq(self.start_date))
            .get_result::<WorkOrder>(conn)
            .map_err(|e| {
                error!("Failed to schedule Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to schedule Work Order: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &WorkOrder) -> Result<(), ServiceError> {
        info!("Work Order ID: {} scheduled for start at: {}", self.work_order_id, self.start_date);
        event_sender.send(Event::WorkOrderScheduled(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderScheduled event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
