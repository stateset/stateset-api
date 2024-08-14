use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::PurchaseOrder};
use diesel::prelude::*;
use tracing::{info, error, instrument};

#[derive(Debug, Serialize, Deserialize)]
pub struct TrackPurchaseOrderCommand {
    pub purchase_order_id: i32,
}

#[async_trait::async_trait]
impl Command for TrackPurchaseOrderCommand {
    type Result = PurchaseOrder;

    #[instrument(skip(self, db_pool))]
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|e| {
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError("Failed to get database connection".into())
        })?;

        let purchase_order = self.track_purchase_order(&conn)?;

        info!("Tracking Purchase Order ID: {}", self.purchase_order_id);

        Ok(purchase_order)
    }
}

impl TrackPurchaseOrderCommand {
    fn track_purchase_order(&self, conn: &PgConnection) -> Result<PurchaseOrder, ServiceError> {
        purchase_orders::table.find(self.purchase_order_id)
            .first::<PurchaseOrder>(conn)
            .map_err(|e| {
                error!("Failed to track Purchase Order ID {}: {}", self.purchase_order_id, e);
                ServiceError::DatabaseError(format!("Failed to track Purchase Order: {}", e))
            })
    }
}
