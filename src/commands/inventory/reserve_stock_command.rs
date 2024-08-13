use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{OrderItem}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use diesel::prelude::*;
use prometheus::IntCounter;

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReserveStockCommand {
    pub order_id: i32,
    #[validate(length(min = 1))]
    pub items: Vec<OrderItem>, // Items to reserve
}

#[async_trait]
impl Command for ReserveStockCommand {
    type Result = ();

    async fn execute(&self, db_pool: Arc<DbPool>, _event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let conn = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        // Begin transaction to ensure atomicity
        conn.transaction(|| {
            // Reserve stock for each item
            for item in &self.items {
                diesel::update(inventory::table.find(item.product_id))
                    .set(inventory::reserved_quantity.eq(inventory::reserved_quantity + item.quantity))
                    .execute(&conn)
                    .map_err(|e| ServiceError::DatabaseError)?;
            }

            info!("Stock reserved for order ID: {}", self.order_id);

            Ok(())
        })
    }
}