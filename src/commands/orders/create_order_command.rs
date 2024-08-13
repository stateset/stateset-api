use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use crate::{errors::ServiceError, db::DbPool, models::{Order, OrderItem, OrderStatus}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_CREATIONS: IntCounter = 
        IntCounter::new("order_creations_total", "Total number of orders created")
            .expect("metric can be created");

    static ref ORDER_CREATION_FAILURES: IntCounter = 
        IntCounter::new("order_creation_failures_total", "Total number of failed order creations")
            .expect("metric can be created");
}

#[async_trait]
pub trait Command: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError>;
}

// CreateOrderCommand: Handles the creation of a new order
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateOrderCommand {
    #[validate(range(min = 1))]
    pub customer_id: i32,

    #[validate(length(min = 1))]
    pub items: Vec<OrderItem>,
}

#[async_trait]
impl Command for CreateOrderCommand {
    type Result = Order;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        // Validate command data
        self.validate().map_err(|e| {
            ORDER_CREATION_FAILURES.inc();
            error!("Validation error: {}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let conn = db_pool.get().map_err(|e| {
            ORDER_CREATION_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let new_order = Order {
            customer_id: self.customer_id,
            items: self.items.clone(),
            status: OrderStatus::Pending,
            // Set other fields like total amount, created_at, etc.
        };

        let saved_order = match diesel::insert_into(orders::table)
            .values(&new_order)
            .get_result::<Order>(&conn) {
            Ok(order) => order,
            Err(e) => {
                ORDER_CREATION_FAILURES.inc();
                error!("Failed to save order: {}", e);
                return Err(ServiceError::DatabaseError);
            }
        };

        // Trigger an event
        if let Err(e) = event_sender.send(Event::OrderCreated(saved_order.id)).await {
            ORDER_CREATION_FAILURES.inc();
            error!("Failed to send OrderCreated event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_CREATIONS.inc();

        info!("Order created successfully: {:?}", saved_order);

        Ok(saved_order)
    }
}
