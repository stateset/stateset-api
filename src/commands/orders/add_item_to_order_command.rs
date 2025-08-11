use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::order_item_entity::{self, Entity as OrderItem},
};
use bigdecimal::BigDecimal;
use chrono::Utc;
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDER_ITEMS_ADDED: IntCounter = IntCounter::new(
        "order_items_added_total",
        "Total number of items added to orders"
    )
    .expect("metric can be created");
    static ref ORDER_ITEM_ADD_FAILURES: IntCounter = IntCounter::new(
        "order_item_add_failures_total",
        "Total number of failed item additions to orders"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddItemToOrderCommand {
    pub order_id: Uuid,
    pub product_id: Uuid,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    #[validate(range(min = 0.01, message = "Unit price must be positive"))]
    pub unit_price: BigDecimal,
    pub product_name: Option<String>,
    pub product_sku: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddItemToOrderResult {
    pub id: Uuid,
    pub order_id: Uuid,
    pub product_id: Uuid,
    pub quantity: i32,
    pub unit_price: f64,
}

#[async_trait::async_trait]
impl Command for AddItemToOrderCommand {
    type Result = AddItemToOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            ORDER_ITEM_ADD_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let saved_item = self.add_item(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_item)
            .await?;

        ORDER_ITEMS_ADDED.inc();

        Ok(AddItemToOrderResult {
            id: saved_item.id,
            order_id: saved_item.order_id,
            product_id: saved_item.product_id,
            quantity: saved_item.quantity,
            unit_price: saved_item.unit_price,
        })
    }
}

impl AddItemToOrderCommand {
    async fn add_item(&self, db: &DatabaseConnection) -> Result<order_item_entity::Model, ServiceError> {
        let new_item = order_item_entity::ActiveModel {
            id: Set(Uuid::new_v4()),
            order_id: Set(self.order_id),
            product_id: Set(self.product_id),
            product_name: Set(self.product_name.clone().unwrap_or_default()),
            product_sku: Set(self.product_sku.clone().unwrap_or_default()),
            quantity: Set(self.quantity),
            unit_price: Set(self.unit_price.to_f64().unwrap_or(0.0)),
            total_price: Set((self.unit_price.clone() * rust_decimal::Decimal::from(self.quantity)).to_f64().unwrap_or(0.0)),
            discount_amount: Set(0.0),
            tax_amount: Set(0.0),
            status: Set(order_item_entity::OrderItemStatus::Pending),
            notes: Set(None),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        };

        let saved_item = new_item.insert(db).await.map_err(|e| {
            ORDER_ITEM_ADD_FAILURES.inc();
            let msg = format!("Failed to add item to order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(e)
        })?;

        Ok(saved_item)
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        saved_item: &order_item_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            product_id = %self.product_id,
            quantity = %self.quantity,
            "Item added to order successfully"
        );

        event_sender
            .send(Event::OrderUpdated(self.order_id))
            .await
            .map_err(|e| {
                ORDER_ITEM_ADD_FAILURES.inc();
                let msg = format!("Failed to send order item added event: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })?;
    }
}
