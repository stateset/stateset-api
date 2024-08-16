use async_trait::async_trait;;
use serde::{Serialize, Deserialize};
use std::sync::Arc;
use sea_orm::*;
use crate::{errors::ServiceError, db::DbPool, models::*};
use chrono::{DateTime, Utc};

use crate::billofmaterials::BillOfMaterials;
use crate::inventory_item::InventoryItem;
use crate::order::Order;
use crate::shipment::Shipment;
use crate::tracking_event::TrackingEvent;
use crate::work_order::WorkOrder;
use crate::return_entity::ReturnEntity;
use crate::order_item::OrderItem;   
use crate::product::Product;
use crate::customer::Customer;
use crate::order::Order;
use crate::warehouse::Warehouse;
use crate::manufacture_order_component_entity::ManufactureOrderComponent;
use crate::manufacture_order_operation_entity::ManufactureOrderOperation;
use crate::manufacture_order_entity::ManufactureOrder;
use crate::manufacture_order_status::ManufactureOrderStatus;

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderQuery {
    pub order_id: i32,
}

#[async_trait]
impl Query for GetOrderQuery {
    type Result = Option<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Order::find_by_id(self.order_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCustomerOrdersQuery {
    pub customer_id: i32,
}

#[async_trait]
impl Query for GetCustomerOrdersQuery {
    type Result = Vec<Order>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Order::find()
            .filter(Order::Column::CustomerId.eq(self.customer_id))
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Order::find()
            .filter(Order::Column::Status.eq(self.status))
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        Order::find()
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(Order::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopSellingProductsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

#[derive(Debug, Serialize)]
pub struct TopSellingProduct {
    pub product: product_entity::Model,
    pub total_sold: i64,
}

#[async_trait]
impl Query for GetTopSellingProductsQuery {
    type Result = Vec<TopSellingProduct>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let result = OrderItem::find()
            .select_only()
            .column(OrderItem::Column::ProductId)
            .column_as(sum(OrderItem::Column::Quantity), "total_sold")
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
                .group_by(OrderItem::Column::ProductId)
                .order_by_desc(sum(OrderItem::Column::Quantity))
            .limit(self.limit)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let top_selling_products = result
            .into_iter()
            .map(|res| {
                let product = product_entity::Entity::find_by_id(res.product_id).one(&db);
                let total_sold = res.total_sold;
                TopSellingProduct {
                    product: product.unwrap(), // Assuming product always exists for the ID
                    total_sold,
                }
            })
            .collect::<Vec<TopSellingProduct>>();

        Ok(top_selling_products)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderDetailsQuery {
    pub order_id: i32,
}

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
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let order = Order::find_by_id(self.order_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?;

        let customer = Customer::find_by_id(order.customer_id)
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?;

        let items = OrderItem::find()
            .filter(OrderItem::Column::OrderId.eq(self.order_id))
            .find_also_related(Product)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(OrderDetails {
            order: order.unwrap(),
            customer: customer.unwrap(),
            items,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetOrderStatusSummaryQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct OrderStatusSummary {
    pub status: OrderStatus,
    pub count: i64,
}

#[async_trait]
impl Query for GetOrderStatusSummaryQuery {
    type Result = Vec<OrderStatusSummary>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let result = Order::find()
            .select_only()
            .column(Order::Column::Status)
            .column_as(count(Order::Column::Id), "count")
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(Order::Column::Status)
            .order_by_asc(Order::Column::Status)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        let status_summary = result
            .into_iter()
            .map(|res| OrderStatusSummary {
                status: res.status,
                count: res.count,
            })
            .collect();

        Ok(status_summary)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAverageOrderValueQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetAverageOrderValueQuery {
    type Result = f64;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;
        let avg_value = Order::find()
            .select_only()
            .column_as(avg(Order::Column::TotalAmount), "average_value")
            .filter(Order::Column::CreatedAt.between(self.start_date, self.end_date))
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(avg_value.unwrap_or(0.0))
    }
}
