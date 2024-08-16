use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{order_item_entity, order_item_entity::Entity as OrderItem}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_ITEMS_ADDED: IntCounter = 
        IntCounter::new("order_items_added_total", "Total number of items added to orders")
            .expect("metric can be created");

    static ref ORDER_ITEM_ADD_FAILURES: IntCounter = 
        IntCounter::new("order_item_add_failures_total", "Total number of failed item additions to orders")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddItemToOrderCommand {
    pub order_id: i32,
    pub product_id: i32,
    pub quantity: i32,
}

#[async_trait]
impl Command for AddItemToOrderCommand {
    type Result = order_item_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            ORDER_ITEM_ADD_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        // Create a new OrderItem to be added to the order
        let new_item = order_item_entity::ActiveModel {
            order_id: Set(self.order_id),
            product_id: Set(self.product_id),
            quantity: Set(self.quantity),
            unit_price: Set(0.0), // Assume price is calculated elsewhere
            ..Default::default() // This will set default values for other fields
        };

        // Insert the new item into the order_items table
        let saved_item = new_item.insert(&db).await.map_err(|e| {
            ORDER_ITEM_ADD_FAILURES.inc();
            error!("Failed to add item to order {}: {}", self.order_id, e);
            ServiceError::DatabaseError
        })?;

        // Trigger an event indicating that an item was added to the order
        if let Err(e) = event_sender.send(Event::OrderItemAdded(self.order_id, saved_item.id)).await {
            ORDER_ITEM_ADD_FAILURES.inc();
            error!("Failed to send OrderItemAdded event for order {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_ITEMS_ADDED.inc();

        info!(
            order_id = %self.order_id,
            product_id = %self.product_id,
            quantity = %self.quantity,
            "Item added to order successfully"
        );

        Ok(saved_item)
    }
}