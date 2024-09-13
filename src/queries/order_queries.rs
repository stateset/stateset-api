use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use sea_orm::{
    query::{Condition, Expr, Function},
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
};
use chrono::{DateTime, Utc};

use crate::{
    errors::ServiceError,
    db::DbPool,
    models::*,
    billofmaterials::BillOfMaterials,
    inventory_item::InventoryItem,
    order::Order,
    shipment::Shipment,
    tracking_event::TrackingEvent,
    work_order::WorkOrder,
    return_entity::ReturnEntity,
    order_item::OrderItem,
    product::Product,
    customer::Customer,
    warehouse::Warehouse,
    manufacture_order_component_entity::ManufactureOrderComponent,
    manufacture_order_operation_entity::ManufactureOrderOperation,
    manufacture_order_entity::ManufactureOrder,
    manufacture_order_status::ManufactureOrderStatus,
};

/// Trait representing a generic asynchronous query.
#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    /// Executes the query using the provided database pool.
    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError>;
}

/// Helper function to obtain a database connection from the pool.
async fn get_db(pool: &Arc<DbPool>) -> Result<sea_orm::DatabaseConnection, ServiceError> {
    pool.get()
        .await
        .map_err(|_| ServiceError::DatabaseError)
}

/// Struct to get a specific order by ID.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetOrderQuery {
    type Result = Option<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        Order::find_by_id(self.order_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetOrderQuery: {:?}", e);
                ServiceError::DatabaseError
            })
    }
}

/// Struct to get all orders for a specific customer.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetCustomerOrdersQuery {
    pub customer_id: i32,
}

#[async_trait]
impl Query for GetCustomerOrdersQuery {
    type Result = Vec<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        Order::find()
            .filter(Order::Column::CustomerId.eq(self.customer_id))
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetCustomerOrdersQuery: {:?}", e);
                ServiceError::DatabaseError
            })
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
    type Result = Vec<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        Order::find()
            .filter(Order::Column::Status.eq(self.status))
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetOrdersByStatusQuery: {:?}", e);
                ServiceError::DatabaseError
            })
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
    type Result = Vec<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;
        Order::find()
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(Order::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetOrdersInDateRangeQuery: {:?}", e);
                ServiceError::DatabaseError
            })
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
    pub product: Product,
    pub total_sold: i64,
}

#[async_trait]
impl Query for GetTopSellingProductsQuery {
    type Result = Vec<TopSellingProduct>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        // Perform a join between OrderItem and Product to fetch all necessary data in one query
        let order_items = OrderItem::find()
            .select_only()
            .column(OrderItem::Column::ProductId)
            .column_as(Function::Sum(OrderItem::Column::Quantity), "total_sold")
            .join(
                sea_orm::JoinType::InnerJoin,
                OrderItem::Relation::Order.def(),
            )
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(OrderItem::Column::ProductId)
            .order_by_desc(Function::Sum(OrderItem::Column::Quantity))
            .limit(self.limit)
            .into_model::<(i32, i64)>()
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetTopSellingProductsQuery: {:?}", e);
                ServiceError::DatabaseError
            })?;

        // Fetch all products in a single query to minimize database calls
        let product_ids: Vec<i32> = order_items.iter().map(|(product_id, _)| *product_id).collect();
        let products = Product::find()
            .filter(Product::Column::Id.is_in(product_ids.clone()))
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching products: {:?}", e);
                ServiceError::DatabaseError
            })?;

        let product_map: std::collections::HashMap<i32, Product> =
            products.into_iter().map(|p| (p.id, p)).collect();

        // Map the results to TopSellingProduct structs
        let top_selling_products = order_items
            .into_iter()
            .filter_map(|(product_id, total_sold)| {
                product_map.get(&product_id).map(|product| TopSellingProduct {
                    product: product.clone(),
                    total_sold,
                })
            })
            .collect();

        Ok(top_selling_products)
    }
}

/// Struct to get detailed information about a specific order.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderDetailsQuery {
    pub order_id: i32,
}

/// Struct representing detailed information of an order.
#[derive(Debug, Serialize)]
pub struct OrderDetails {
    pub order: Order,
    pub customer: Customer,
    pub items: Vec<(OrderItem, Product)>,
}

#[async_trait]
impl Query for GetOrderDetailsQuery {
    type Result = OrderDetails;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        // Fetch the order
        let order = Order::find_by_id(self.order_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching order: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch the customer
        let customer = Customer::find_by_id(order.customer_id)
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching customer: {:?}", e);
                ServiceError::DatabaseError
            })?
            .ok_or(ServiceError::NotFound)?;

        // Fetch order items along with their associated products
        let items = OrderItem::find()
            .filter(OrderItem::Column::OrderId.eq(self.order_id))
            .find_also_related(Product)
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error fetching order items: {:?}", e);
                ServiceError::DatabaseError
            })?
            .into_iter()
            .filter_map(|(item, product)| product.map(|p| (item, p)))
            .collect();

        Ok(OrderDetails {
            order,
            customer,
            items,
        })
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
    pub status: OrderStatus,
    pub count: i64,
}

#[async_trait]
impl Query for GetOrderStatusSummaryQuery {
    type Result = Vec<OrderStatusSummary>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        let summaries = Order::find()
            .select_only()
            .column(Order::Column::Status)
            .column_as(Function::Count(Order::Column::Id), "count")
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(Order::Column::Status)
            .order_by_asc(Order::Column::Status)
            .into_model::<(OrderStatus, i64)>()
            .all(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetOrderStatusSummaryQuery: {:?}", e);
                ServiceError::DatabaseError
            })?;

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

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = get_db(&db_pool).await?;

        let average = Order::find()
            .select_only()
            .column_as(Function::Avg(Order::Column::TotalAmount), "average_value")
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .into_model::<Option<f64>>()
            .one(&db)
            .await
            .map_err(|e| {
                log::error!("Database error in GetAverageOrderValueQuery: {:?}", e);
                ServiceError::DatabaseError
            })?
            .unwrap_or(None)
            .unwrap_or(0.0);

        Ok(average)
    }
}
