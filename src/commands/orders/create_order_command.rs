use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self},
        order_item_entity::{self},
        OrderItemStatus, OrderStatus,
    },
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{Set, *};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDER_CREATIONS: IntCounter =
        IntCounter::new("order_creations_total", "Total number of orders created")
            .expect("metric can be created");
    static ref ORDER_CREATION_FAILURES: IntCounter = IntCounter::new(
        "order_creation_failures_total",
        "Total number of failed order creations"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateOrderCommand {
    pub customer_id: Uuid,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<CreateOrderItem>,
}

#[derive(Debug, Serialize, Deserialize, Validate, Clone)]
pub struct CreateOrderItem {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub unit_price: BigDecimal,
    pub product_name: Option<String>,
    pub product_sku: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateOrderResult {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub items: Vec<CreateOrderItem>,
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

        self.log_and_trigger_event(&event_sender, &saved_order)
            .await?;

        ORDER_CREATIONS.inc();

        Ok(CreateOrderResult {
            id: saved_order.id,
            customer_id: saved_order.customer_id,
            status: saved_order.status.to_string(),
            created_at: saved_order.created_at,
            items: self.items.clone(),
        })
    }
}

impl CreateOrderCommand {
    async fn create_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        let customer_id = self.customer_id;
        let items = self.items.clone();

        db.transaction::<_, order_entity::Model, ServiceError>(move |txn| {
            let items = items.clone();
            Box::pin(async move {
                let new_order = order_entity::ActiveModel {
                    customer_id: Set(customer_id),
                    status: Set(OrderStatus::Pending),
                    created_at: Set(Utc::now()),
                    updated_at: Set(Utc::now()),
                    ..Default::default()
                };

                let saved_order = new_order.insert(txn).await.map_err(|e| {
                    let msg = format!("Failed to create order for customer {}: {}", customer_id, e);
                    error!("{}", msg);
                    ServiceError::db_error(e)
                })?;

                for item in &items {
                    let new_item = order_item_entity::ActiveModel {
                        id: Set(Uuid::new_v4()),
                        order_id: Set(saved_order.id),
                        product_id: Set(item.product_id),
                        product_name: Set(item.product_name.clone().unwrap_or_default()),
                        product_sku: Set(item.product_sku.clone().unwrap_or_default()),
                        quantity: Set(item.quantity),
                        unit_price: Set(f64::from_str(&item.unit_price.to_string()).unwrap_or(0.0)),
                        total_price: Set(f64::from_str(
                            &(item.unit_price.clone() * BigDecimal::from(item.quantity))
                                .to_string(),
                        )
                        .unwrap_or(0.0)),
                        discount_amount: Set(0.0),
                        tax_amount: Set(0.0),
                        status: Set(OrderItemStatus::Pending),
                        notes: Set(None),
                        created_at: Set(Utc::now()),
                        updated_at: Set(Utc::now()),
                        ..Default::default()
                    };
                    new_item.insert(txn).await.map_err(|e| {
                        let msg = format!(
                            "Failed to create order item for order {}: {}",
                            saved_order.id, e
                        );
                        error!("{}", msg);
                        ServiceError::db_error(e)
                    })?;
                }

                Ok(saved_order)
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
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
