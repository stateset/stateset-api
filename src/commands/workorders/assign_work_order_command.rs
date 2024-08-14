use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::WorkOrder};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AssignWorkOrderCommand {
    pub work_order_id: i32,
    pub assignee_id: i32, // ID of the worker or team to whom the work order is assigned
}

#[async_trait::async_trait]
impl Command for AssignWorkOrderCommand {
    type Result = WorkOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_work_order = conn.transaction(|| {
            self.assign_work_order(&conn)
        }).map_err(|e| {
            error!("Transaction failed for assigning Work Order ID {}: {}", self.work_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl AssignWorkOrderCommand {
    fn assign_work_order(&self, conn: &PgConnection) -> Result<WorkOrder, ServiceError> {
        diesel::update(work_orders::table.find(self.work_order_id))
            .set(work_orders::assignee_id.eq(self.assignee_id))
            .get_result::<WorkOrder>(conn)
            .map_err(|e| {
                error!("Failed to assign Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to assign Work Order: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &WorkOrder) -> Result<(), ServiceError> {
        info!("Work Order ID: {} assigned to Assignee ID: {}", self.work_order_id, self.assignee_id);
        event_sender.send(Event::WorkOrderAssigned(work_order.id, self.assignee_id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderAssigned event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
