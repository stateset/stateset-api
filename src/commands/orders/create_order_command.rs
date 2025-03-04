use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_item_entity::{self, Entity as OrderItem},
        OrderStatus,
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, Counter};
use lazy_static::lazy_static;
use chrono::{DateTime, Utc};

lazy_static! {
    static ref ORDER_CREATIONS: IntCounter = 
        IntCounter::new("order_creations_total", "Total number of orders created")
            .expect("metric can be created");

    static ref ORDER_CREATION_FAILURES: IntCounter = 
        IntCounter::new("order_creation_failures_total", "Total number of failed order creations")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateOrderCommand {
    pub customer_id: Uuid,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<OrderItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderItem {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderResult {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub items: Vec<OrderItem>,
}

#[async_trait::async_trait]
impl Command for CreateOrderCommand {
    type Result = CreateOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            ORDER_CREATION_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let saved_order = self.create_order(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_order).await?;

        ORDER_CREATIONS.inc();

        Ok(CreateOrderResult {
            id: saved_order.id,
            customer_id: saved_order.customer_id,
            status: saved_order.status,
            created_at: saved_order.created_at.and_utc(),
            items: self.items.clone(),
        })
    }
}

impl CreateOrderCommand {
    async fn create_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                let new_order = order_entity::ActiveModel {
                    customer_id: Set(self.customer_id),
                    status: Set(OrderStatus::Pending.to_string()),
                    created_at: Set(Utc::now().naive_utc()),
                    ..Default::default()
                };

                let saved_order = new_order.insert(txn).await.map_err(|e| {
                    let msg = format!("Failed to save order: {}", e);
                    error!("{}", msg);
                    ServiceError::DatabaseError(msg)
                })?;

                for item in &self.items {
                    let new_item = order_item_entity::ActiveModel {
                        order_id: Set(saved_order.id),
                        product_id: Set(item.product_id),
                        quantity: Set(item.quantity),
                        ..Default::default()
                    };
                    new_item.insert(txn).await.map_err(|e| {
                        let msg = format!("Failed to save order item: {}", e);
                        error!("{}", msg);
                        ServiceError::DatabaseError(msg)
                    })?;
                }

                Ok(saved_order)
            })
        }).await
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %saved_order.id,
            customer_id = %self.customer_id,
            items_count = %self.items.len(),
            "Order created successfully"
        );

        event_sender
            .send(Event::OrderCreated(saved_order.id))
            .await
            .map_err(|e| {
                ORDER_CREATION_FAILURES.inc();
                let msg = format!("Failed to send event for created order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}