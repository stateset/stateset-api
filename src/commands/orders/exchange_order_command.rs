use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::{
    order_entity, order_entity::Entity as Order,
    order_item_entity, order_item_entity::Entity as OrderItem,
    return_item_entity, return_item_entity::Entity as ReturnItem,
    OrderStatus
}};
use crate::events::{Event, EventSender};
use validator::Validate;
use tracing::{info, error, instrument};
use prometheus::IntCounter;

lazy_static! {
    static ref ORDER_EXCHANGES: IntCounter = 
        IntCounter::new("order_exchanges_total", "Total number of order exchanges")
            .expect("metric can be created");

    static ref ORDER_EXCHANGE_FAILURES: IntCounter = 
        IntCounter::new("order_exchange_failures_total", "Total number of failed order exchanges")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ExchangeOrderCommand {
    #[validate(range(min = 1))]
    pub order_id: i32,

    #[validate(length(min = 1))]
    pub return_items: Vec<return_item_entity::Model>,

    #[validate(length(min = 1))]
    pub new_items: Vec<order_item_entity::Model>,
}

#[async_trait]
impl Command for ExchangeOrderCommand {
    type Result = order_entity::Model;

    #[instrument(skip(db_pool, event_sender))]
    async fn execute(&self, db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|e| {
            ORDER_EXCHANGE_FAILURES.inc();
            error!("Failed to get database connection: {}", e);
            ServiceError::DatabaseError
        })?;

        let updated_order = db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                // Insert return items into return_items table
                for item in &self.return_items {
                    let return_item = return_item_entity::ActiveModel {
                        order_id: Set(self.order_id),
                        product_id: Set(item.product_id),
                        quantity: Set(item.quantity),
                        // Set other fields as needed
                        ..Default::default()
                    };
                    return_item.insert(txn).await.map_err(|e| {
                        ORDER_EXCHANGE_FAILURES.inc();
                        error!("Failed to insert return item for order ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?;
                }

                // Insert new items into order_items table
                for item in &self.new_items {
                    let new_item = order_item_entity::ActiveModel {
                        order_id: Set(self.order_id),
                        product_id: Set(item.product_id),
                        quantity: Set(item.quantity),
                        // Set other fields as needed
                        ..Default::default()
                    };
                    new_item.insert(txn).await.map_err(|e| {
                        ORDER_EXCHANGE_FAILURES.inc();
                        error!("Failed to insert new order item for order ID {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?;
                }

                // Update order status to Exchanged
                let order = Order::find_by_id(self.order_id)
                    .one(txn)
                    .await
                    .map_err(|e| {
                        ORDER_EXCHANGE_FAILURES.inc();
                        error!("Failed to find order {}: {}", self.order_id, e);
                        ServiceError::DatabaseError
                    })?
                    .ok_or_else(|| {
                        ORDER_EXCHANGE_FAILURES.inc();
                        error!("Order {} not found", self.order_id);
                        ServiceError::NotFound
                    })?;

                let mut order: order_entity::ActiveModel = order.into();
                order.status = Set(OrderStatus::Exchanged.to_string());

                let updated_order = order.update(txn).await.map_err(|e| {
                    ORDER_EXCHANGE_FAILURES.inc();
                    error!("Failed to update order status to Exchanged for order ID {}: {}", self.order_id, e);
                    ServiceError::DatabaseError
                })?;

                Ok(updated_order)
            })
        }).await?;

        // Trigger an event
        if let Err(e) = event_sender.send(Event::OrderExchanged(self.order_id)).await {
            ORDER_EXCHANGE_FAILURES.inc();
            error!("Failed to send OrderExchanged event for order ID {}: {}", self.order_id, e);
            return Err(ServiceError::EventError(e.to_string()));
        }

        ORDER_EXCHANGES.inc();

        info!(
            order_id = %self.order_id,
            "Order exchanged successfully"
        );

        Ok(updated_order)
    }
}