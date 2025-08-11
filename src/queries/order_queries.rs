use async_trait::async_trait;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::{
    prelude::*, ColumnTrait, Condition, DatabaseTransaction, DbErr, EntityTrait, FromQueryResult,
    Order as SeaOrder, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    sea_query::Func,
};
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use crate::{
    db::{DatabaseAccess, DbPool},
    entities::*,
    errors::ServiceError,
    models::*,
};

/// Database model imports
// These should be updated to use the new entity pattern
use crate::models::{
    customer::{self, Entity as Customer},
    order_entity::{self, Entity as Order},
    order_item_entity::{self, Entity as OrderItem},
    product::{self, Entity as Product},
};

/// Trait representing a generic asynchronous query.
#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    /// Executes the query using the provided database connection
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError>;
}

/// Struct to get a specific order by ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderQuery {
    pub order_id: Uuid,
}

#[async_trait]
impl Query for GetOrderQuery {
    type Result = Option<order_entity::Model>;

    #[instrument(skip(self, db), fields(order_id = %self.order_id))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetOrderQuery");
        let pool = db.get_pool();

        Order::find_by_id(self.order_id)
            .one(pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))
    }
}

/// Struct to get all orders for a specific customer.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetCustomerOrdersQuery {
    pub customer_id: Uuid,
}

#[async_trait]
impl Query for GetCustomerOrdersQuery {
    type Result = Vec<order_entity::Model>;

    #[instrument(skip(self, db), fields(customer_id = %self.customer_id))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetCustomerOrdersQuery");
        let pool = db.get_pool();

        Order::find()
            .filter(order_entity::Column::CustomerId.eq(self.customer_id))
            .all(pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))
    }
}

/// Order status enum - should be replaced with a proper enum from the model
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Pending,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Returned,
    OnHold,
    Completed,
}

impl ToString for OrderStatus {
    fn to_string(&self) -> String {
        match self {
            OrderStatus::Pending => "pending",
            OrderStatus::Processing => "processing",
            OrderStatus::Shipped => "shipped",
            OrderStatus::Delivered => "delivered",
            OrderStatus::Cancelled => "cancelled",
            OrderStatus::Returned => "returned",
            OrderStatus::OnHold => "on_hold",
            OrderStatus::Completed => "completed",
        }
        .to_string()
    }
}

/// Struct to get orders filtered by status with pagination.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrdersByStatusQuery {
    pub status: OrderStatus,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetOrdersByStatusQuery {
    type Result = Vec<order_entity::Model>;

    #[instrument(skip(self, db), fields(status = %self.status.to_string(), limit = %self.limit, offset = %self.offset))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetOrdersByStatusQuery");
        let pool = db.get_pool();

        Order::find()
            .filter(order_entity::Column::Status.eq(self.status.to_string()))
            .limit(self.limit)
            .offset(self.offset)
            .all(pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))
    }
}

/// Struct to get orders within a specific date range with pagination.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrdersInDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetOrdersInDateRangeQuery {
    type Result = Vec<order_entity::Model>;

    #[instrument(skip(self, db), fields(start_date = %self.start_date, end_date = %self.end_date))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetOrdersInDateRangeQuery");
        let pool = db.get_pool();

        Order::find()
            .filter(
                Condition::all()
                    .add(order_entity::Column::CreatedAt.gte(self.start_date.naive_utc()))
                    .add(order_entity::Column::CreatedAt.lte(self.end_date.naive_utc())),
            )
            .order_by_desc(order_entity::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e))
    }
}

/// Struct to get top-selling products within a date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopSellingProductsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

/// Struct representing a top-selling product with total sold quantity.
#[derive(Debug, Serialize)]
pub struct TopSellingProduct {
    pub product: product::Model,
    pub total_sold: i64,
}

#[async_trait]
impl Query for GetTopSellingProductsQuery {
    type Result = Vec<TopSellingProduct>;

    #[instrument(skip(self, db), fields(start_date = %self.start_date, end_date = %self.end_date, limit = %self.limit))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetTopSellingProductsQuery");

        db.transaction(|txn| {
            Box::pin(async move {
                // First, get the product IDs and total quantities
                let result: Vec<(Uuid, i64)> = OrderItem::find()
                    .select_only()
                    .column(order_item_entity::Column::ProductId)
                    .column_as(
                        Func::sum(order_item_entity::Column::Quantity),
                        "total_sold",
                    )
                    .join(
                        sea_orm::JoinType::InnerJoin,
                        <crate::models::order_item_entity::Entity as sea_orm::EntityTrait>::Relation::Order.def(),
                    )
                    .filter(
                        Condition::all()
                            .add(order_entity::Column::CreatedAt.gte(self.start_date.naive_utc()))
                            .add(order_entity::Column::CreatedAt.lte(self.end_date.naive_utc())),
                    )
                    .group_by(order_item_entity::Column::ProductId)
                    .order_by_desc(Expr::col(order_item_entity::Column::Quantity).sum())
                    .limit(self.limit)
                    .into_tuple()
                    .all(txn)
                    .await?;

                // If no results, return empty
                if result.is_empty() {
                    return Ok(Vec::new());
                }

                // Extract product IDs
                let product_ids: Vec<Uuid> = result.iter().map(|(id, _)| *id).collect();

                // Get the product details
                let products = Product::find()
                    .filter(product::Column::Id.is_in(product_ids.clone()))
                    .all(txn)
                    .await?;

                // Create a map of product id -> product
                let product_map: std::collections::HashMap<Uuid, product::Model> =
                    products.into_iter().map(|p| (p.id, p)).collect();

                // Build the final result
                let top_products = result
                    .into_iter()
                    .filter_map(|(product_id, total_sold)| {
                        product_map
                            .get(&product_id)
                            .map(|product| TopSellingProduct {
                                product: product.clone(),
                                total_sold,
                            })
                    })
                    .collect();

                Ok::<_, DbErr>(top_products)
            })
        })
        .await
        .map_err(|e| ServiceError::DatabaseError(e))
    }
}

/// Struct to get detailed information about a specific order.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderDetailsQuery {
    pub order_id: Uuid,
}

/// Struct representing detailed information of an order.
#[derive(Debug, Serialize)]
pub struct OrderDetails {
    pub order: order_entity::Model,
    pub customer: customer::Model,
    pub items: Vec<(order_item_entity::Model, product::Model)>,
}

#[async_trait]
impl Query for GetOrderDetailsQuery {
    type Result = OrderDetails;

    #[instrument(skip(self, db), fields(order_id = %self.order_id))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetOrderDetailsQuery");

        db.transaction(|txn| {
            Box::pin(async move {
                // Fetch the order
                let order = Order::find_by_id(self.order_id)
                    .one(txn)
                    .await?
                    .ok_or_else(|| {
                        DbErr::RecordNotFound(format!("Order with ID {} not found", self.order_id))
                    })?;

                // Fetch the customer
                let customer = Customer::find_by_id(order.customer_id)
                    .one(txn)
                    .await?
                    .ok_or_else(|| {
                        DbErr::RecordNotFound(format!(
                            "Customer with ID {} not found for order {}",
                            order.customer_id, self.order_id
                        ))
                    })?;

                // Fetch order items along with their associated products
                let items = OrderItem::find()
                    .filter(order_item_entity::Column::OrderId.eq(self.order_id))
                    .find_with_related(Product)
                    .all(txn)
                    .await?
                    .into_iter()
                    .map(|(item, products)| (item, products.first().cloned().unwrap_or_default()))
                    .collect();

                Ok::<_, DbErr>(OrderDetails {
                    order,
                    customer,
                    items,
                })
            })
        })
        .await
        .map_err(|e| ServiceError::DatabaseError(e))
    }
}

/// Struct to get a summary of order statuses within a date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderStatusSummaryQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

/// Struct representing a summary of a specific order status.
#[derive(Debug, Serialize)]
pub struct OrderStatusSummary {
    pub status: String,
    pub count: i64,
}

#[async_trait]
impl Query for GetOrderStatusSummaryQuery {
    type Result = Vec<OrderStatusSummary>;

    #[instrument(skip(self, db), fields(start_date = %self.start_date, end_date = %self.end_date))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetOrderStatusSummaryQuery");

        // The original query might need adjustment based on the actual schema
        // This is an approximation - use DB-specific SQL if needed

        let summaries: Vec<(String, i64)> = db
            .execute("get_order_status_summary", |conn| {
                Order::find()
                    .select_only()
                    .column(order_entity::Column::Status)
                    .column_as(Expr::count(order_entity::Column::Id), "count")
                    .filter(
                        Condition::all()
                            .add(order_entity::Column::CreatedAt.gte(self.start_date.naive_utc()))
                            .add(order_entity::Column::CreatedAt.lte(self.end_date.naive_utc())),
                    )
                    .group_by(order_entity::Column::Status)
                    .order_by_asc(order_entity::Column::Status)
                    .into_tuple()
                    .all(conn)
            })
            .await?;

        Ok(summaries
            .into_iter()
            .map(|(status, count)| OrderStatusSummary { status, count })
            .collect())
    }
}

/// Struct to get the average value of orders within a date range.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetAverageOrderValueQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetAverageOrderValueQuery {
    type Result = f64;

    #[instrument(skip(self, db), fields(start_date = %self.start_date, end_date = %self.end_date))]
    async fn execute(&self, db: &DatabaseAccess) -> Result<Self::Result, ServiceError> {
        debug!("Executing GetAverageOrderValueQuery");

        // Use raw SQL for this complex aggregation
        let sql = r#"
            SELECT AVG(total_amount) AS average_value 
            FROM orders 
            WHERE created_at BETWEEN $1 AND $2
        "#;

        let params = vec![
            sea_orm::Value::from(self.start_date.naive_utc()),
            sea_orm::Value::from(self.end_date.naive_utc()),
        ];

        let result: Option<f64> = db.execute_raw(sql, params).await.map_err(|e| {
            ServiceError::DatabaseError(format!("Failed to get average order value: {}", e))
        })?;

        Ok(result.unwrap_or(0.0))
    }
}
