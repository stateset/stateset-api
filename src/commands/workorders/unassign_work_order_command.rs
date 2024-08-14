use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::WorkOrder};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UnassignWorkOrderCommand {
    pub work_order_id: i32,
}

#[async_trait::async_trait]
impl Command for UnassignWorkOrderCommand {
    type Result = WorkOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_work_order = conn.transaction(|| {
            self.unassign_work_order(&conn)
        }).map_err(|e| {
            error!("Transaction failed for unassigning Work Order ID {}: {}", self.work_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl UnassignWorkOrderCommand {
    fn unassign_work_order(&self, conn: &PgConnection) -> Result<WorkOrder, ServiceError> {
        diesel::update(work_orders::table.find(self.work_order_id))
            .set(work_orders::assignee_id.eq::<Option<i32>>(None)) // Unassign the work order
            .get_result::<WorkOrder>(conn)
            .map_err(|e| {
                error!("Failed to unassign Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to unassign Work Order: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &WorkOrder) -> Result<(), ServiceError> {
        info!("Work Order ID: {} has been unassigned.", self.work_order_id);
        event_sender.send(Event::WorkOrderUnassigned(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderUnassigned event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
