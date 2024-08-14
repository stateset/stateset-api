use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{WorkOrder, WorkOrderStatus}};
use diesel::prelude::*;
use tracing::{info, error, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct ListWorkOrdersCommand {
    pub status: Option<WorkOrderStatus>, // Optional filter by status
    pub assignee_id: Option<i32>, // Optional filter by assignee
}

#[async_trait::async_trait]
impl Command for ListWorkOrdersCommand {
    type Result = Vec<WorkOrder>;

    #[instrument(skip(self, db_pool))]
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let work_orders = self.list_work_orders(&conn)?;

        info!("Listed work orders. Count: {}", work_orders.len());

        Ok(work_orders)
    }
}

impl ListWorkOrdersCommand {
    fn list_work_orders(&self, conn: &PgConnection) -> Result<Vec<WorkOrder>, ServiceError> {
        let mut query = work_orders::table.into_boxed();

        if let Some(status) = self.status {
            query = query.filter(work_orders::status.eq(status));
        }

        if let Some(assignee_id) = self.assignee_id {
            query = query.filter(work_orders::assignee_id.eq(assignee_id));
        }

        query.load::<WorkOrder>(conn).map_err(|e| {
            error!("Failed to list work orders: {}", e);
            ServiceError::DatabaseError(format!("Failed to list work orders: {}", e))
        })
    }
}
