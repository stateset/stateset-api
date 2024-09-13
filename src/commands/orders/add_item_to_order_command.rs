use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_item_entity::{self, Entity as OrderItem},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::IntCounter;
use lazy_static::lazy_static;

lazy_static! {
    static ref ORDER_ITEMS_ADDED: IntCounter = 
        IntCounter::new("order_items_added_total", "Total number of items added to orders")
            .expect("metric can be created");

    static ref ORDER_ITEM_ADD_FAILURES: IntCounter = 
        IntCounter::new("order_item_add_failures_total", "Total number of failed item additions to orders")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct AddItemToOrderCommand {
    pub order_id: Uuid,
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
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

        let saved_item = self.add_item_to_order(db).await?;

        self.log_and_trigger_event(&event_sender, &saved_item).await?;

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
    async fn add_item_to_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_item_entity::Model, ServiceError> {
        let new_item = order_item_entity::ActiveModel {
            order_id: Set(self.order_id),
            product_id: Set(self.product_id),
            quantity: Set(self.quantity),
            unit_price: Set(0.0), // Assume price is calculated elsewhere
            ..Default::default()
        };

        new_item.insert(db).await.map_err(|e| {
            ORDER_ITEM_ADD_FAILURES.inc();
            let msg = format!("Failed to add item to order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(msg)
        })
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
            .send(Event::OrderItemAdded(self.order_id, saved_item.id))
            .await
            .map_err(|e| {
                ORDER_ITEM_ADD_FAILURES.inc();
                let msg = format!("Failed to send event for added order item: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}