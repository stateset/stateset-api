use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{
    order_entity, order_entity::Entity as Order,
    order_item_entity, order_item_entity::Entity as OrderItem,
    OrderStatus
}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use chrono::Utc;
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateOrderCommand {
    #[validate(range(min = 1))]
    pub customer_id: i32,

    #[validate(length(min = 1))]
    pub items: Vec<order_item_entity::Model>,
}

#[async_trait]
impl Command for CreateOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        // Validate command data
        self.validate().map_err(|e| {
            ORDER_CREATION_FAILURES.inc();
            error!("Validation error: {}", e);
            ServiceError::ValidationError(e.to_string())
        })?;

        let db = db_pool.get().map_err(|e| {
            ORDER_CREATION_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let result = db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                // Create new order
                let new_order = order_entity::ActiveModel {
                    customer_id: Set(self.customer_id),
                    status: Set(OrderStatus::Pending.to_string()),
                    created_at: Set(Utc::now()),
                    // Set other fields as needed
                    ..Default::default()
                };

                let saved_order = new_order.insert(txn).await.map_err(|e| {
                    ORDER_CREATION_FAILURES.inc();
                    error!("Failed to save order: {}", e);
                    ServiceError::DatabaseError
                })?;

                // Insert order items
                for item in &self.items {
                    let new_item = order_item_entity::ActiveModel {
                        order_id: Set(saved_order.id),
                        product_id: Set(item.product_id),
                        quantity: Set(item.quantity),
                        // Set other fields as needed
                        ..Default::default()
                    };
                    new_item.insert(txn).await.map_err(|e| {
                        ORDER_CREATION_FAILURES.inc();
                        error!("Failed to save order item: {}", e);
                        ServiceError::DatabaseError
                    })?;
                }

                Ok(saved_order)
            })
        }).await?;

        // Trigger an event
        if let Err(e) = event_sender.send(Event::OrderCreated(result.id)).await {
            ORDER_CREATION_FAILURES.inc();
            error!("Failed to send OrderCreated event: {}", e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_CREATIONS.inc();

        info!("Order created successfully: {:?}", result);

        Ok(result)
    }
}