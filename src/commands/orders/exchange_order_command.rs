use crate::{
    commands::Command,
    db::DbPool,
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        order_entity::{self, Entity as Order},
        order_item_entity::{self, Entity as OrderItem},
        return_item_entity::{self, Entity as ReturnItem},
        OrderStatus,
    },
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use prometheus::IntCounter;
use sea_orm::{*, Set, TransactionError};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;

lazy_static! {
    static ref ORDER_EXCHANGES: IntCounter =
        IntCounter::new("order_exchanges_total", "Total number of order exchanges")
            .expect("metric can be created");
    static ref ORDER_EXCHANGE_FAILURES: IntCounter = IntCounter::new(
        "order_exchange_failures_total",
        "Total number of failed order exchanges"
    )
    .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ExchangeOrderCommand {
    pub order_id: Uuid,
    pub return_id: Uuid,

    #[validate(length(min = 1, message = "At least one return item is required"))]
    pub return_items: Vec<ReturnItemInput>,

    #[validate(length(min = 1, message = "At least one new item is required"))]
    pub new_items: Vec<OrderItemInput>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ReturnItemInput {
    pub order_item_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct OrderItemInput {
    pub product_id: Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExchangeOrderResult {
    pub id: Uuid,
    pub status: String,
    pub exchanged_at: DateTime<Utc>,
    pub returned_items_count: usize,
    pub new_items_count: usize,
}

#[async_trait::async_trait]
impl Command for ExchangeOrderCommand {
    type Result = ExchangeOrderResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, ServiceError> {
        self.validate().map_err(|e| {
            ORDER_EXCHANGE_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            ServiceError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();

        let updated_order = self.exchange_order(db).await?;

        self.log_and_trigger_event(&event_sender, &updated_order)
            .await?;

        ORDER_EXCHANGES.inc();

        Ok(ExchangeOrderResult {
            id: updated_order.id,
            status: updated_order.status.to_string(),
            exchanged_at: updated_order.updated_at,
            returned_items_count: self.return_items.len(),
            new_items_count: self.new_items.len(),
        })
    }
}

impl ExchangeOrderCommand {
    async fn exchange_order(
        &self,
        db: &DatabaseConnection,
    ) -> Result<order_entity::Model, ServiceError> {
        db.transaction::<_, order_entity::Model, ServiceError>(|txn| {
            Box::pin(async move {
                self.insert_return_items(txn).await?;
                self.insert_new_items(txn).await?;
                self.update_order_status(txn).await
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::DatabaseError(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
    }

    async fn insert_return_items(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        for item in &self.return_items {
            let return_item = return_item_entity::ActiveModel {
                return_id: Set(self.return_id),
                order_item_id: Set(item.order_item_id),
                quantity: Set(item.quantity),
                reason: Set(item.reason.clone()),
                created_at: Set(Utc::now()),
                ..Default::default()
            };
            return_item.insert(txn).await.map_err(|e| {
                let msg = format!("Failed to insert return item: {}", e);
                error!("{}", msg);
                ORDER_EXCHANGE_FAILURES.inc();
                let msg = format!(
                    "Failed to insert return item for order ID {}: {}",
                    self.order_id, e
                );
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;
        }
        Ok(())
    }

    async fn insert_new_items(&self, txn: &DatabaseTransaction) -> Result<(), ServiceError> {
        for item in &self.new_items {
            let new_item = order_item_entity::ActiveModel {
                order_id: Set(self.order_id),
                product_id: Set(item.product_id),
                quantity: Set(item.quantity),
                created_at: Set(Utc::now().naive_utc()),
                ..Default::default()
            };
            new_item.insert(txn).await.map_err(|e| {
                ORDER_EXCHANGE_FAILURES.inc();
                let msg = format!(
                    "Failed to insert new order item for order ID {}: {}",
                    self.order_id, e
                );
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?;
        }
        Ok(())
    }

    async fn update_order_status(
        &self,
        txn: &DatabaseTransaction,
    ) -> Result<order_entity::Model, ServiceError> {
        let order = Order::find_by_id(self.order_id)
            .one(txn)
            .await
            .map_err(|e| {
                ORDER_EXCHANGE_FAILURES.inc();
                let msg = format!("Failed to find order {}: {}", self.order_id, e);
                error!("{}", msg);
                ServiceError::DatabaseError(msg)
            })?
            .ok_or_else(|| {
                ORDER_EXCHANGE_FAILURES.inc();
                let msg = format!("Order {} not found", self.order_id);
                error!("{}", msg);
                ServiceError::NotFound(msg)
            })?;

        let mut order: order_entity::ActiveModel = order.into();
        order.status = Set(OrderStatus::Exchanged);
        order.updated_at = Set(Utc::now());

        order.update(txn).await.map_err(|e| {
            let msg = format!("Failed to update Order {}: {}", self.order_id, e);
            error!("{}", msg);
            ServiceError::DatabaseError(e)
        })?;
    }

    async fn log_and_trigger_event(
        &self,
        event_sender: &EventSender,
        _updated_order: &order_entity::Model,
    ) -> Result<(), ServiceError> {
        info!(
            order_id = %self.order_id,
            returned_items = %self.return_items.len(),
            new_items = %self.new_items.len(),
            "Order exchanged successfully"
        );

        event_sender
            .send(Event::OrderExchanged(self.order_id))
            .await
            .map_err(|e| {
                ORDER_EXCHANGE_FAILURES.inc();
                let msg = format!("Failed to send event for exchanged order: {}", e);
                error!("{}", msg);
                ServiceError::EventError(msg)
            })
    }
}
