use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::WorkOrder};
use crate::events::{Event, EventSender};
use tracing::{info, error, instrument};
use diesel::prelude::*;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateWorkOrderCommand {
    pub work_order_id: i32,
    pub description: Option<String>, // Optional new description for the work order
    pub quantity: Option<i32>, // Optional new quantity
    pub due_date: Option<chrono::NaiveDateTime>, // Optional new due date
}

#[async_trait::async_trait]
impl Command for UpdateWorkOrderCommand {
    type Result = WorkOrder;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let updated_work_order = conn.transaction(|| {
            self.update_work_order(&conn)
        }).map_err(|e| {
            error!("Transaction failed for updating Work Order ID {}: {}", self.work_order_id, e);
            e
        })?;

        self.log_and_trigger_event(event_sender, &updated_work_order).await?;

        Ok(updated_work_order)
    }
}

impl UpdateWorkOrderCommand {
    fn update_work_order(&self, conn: &PgConnection) -> Result<WorkOrder, ServiceError> {
        let target = work_orders::table.find(self.work_order_id);

        diesel::update(target)
            .set((
                self.description.as_ref().map(|desc| work_orders::description.eq(desc)),
                self.quantity.map(|qty| work_orders::quantity.eq(qty)),
                self.due_date.map(|date| work_orders::due_date.eq(date)),
            ))
            .get_result::<WorkOrder>(conn)
            .map_err(|e| {
                error!("Failed to update Work Order ID {}: {}", self.work_order_id, e);
                ServiceError::DatabaseError(format!("Failed to update Work Order: {}", e))
            })
    }

    async fn log_and_trigger_event(&self, event_sender: Arc<EventSender>, work_order: &WorkOrder) -> Result<(), ServiceError> {
        info!("Work Order updated with ID: {}", self.work_order_id);
        event_sender.send(Event::WorkOrderUpdated(work_order.id))
            .await
            .map_err(|e| {
                error!("Failed to send WorkOrderUpdated event for Work Order ID {}: {}", work_order.id, e);
                ServiceError::EventError(e.to_string())
            })
    }
}
