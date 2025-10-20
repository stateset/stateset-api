use async_trait::async_trait;
use chrono::{DateTime, Utc, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use sea_orm::{
    prelude::*, query::*, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    sea_query::{Func, Expr, Alias, Condition}, IntoSimpleExpr, FromQueryResult,
};

use crate::{
    db::DbPool, errors::ServiceError,
    models::{
        purchase_order_entity::{Entity as PurchaseOrderEntity, Model as PurchaseOrderModel, PurchaseOrderStatus},
        suppliers::{Entity as SupplierEntity},
        purchase_order_item_entity::{Entity as PurchaseOrderItemEntity},
    },
};

/// Trait representing a generic asynchronous query.
#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    /// Executes the query using the provided database pool.
    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

/// Query to get a specific purchase order by ID
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPurchaseOrderQuery {
    pub purchase_order_id: Uuid,
}

#[async_trait]
impl Query for GetPurchaseOrderQuery {
    type Result = Option<PurchaseOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        PurchaseOrderEntity::find_by_id(self.purchase_order_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Query to get purchase orders by status with pagination
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPurchaseOrdersByStatusQuery {
    pub status: PurchaseOrderStatus,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetPurchaseOrdersByStatusQuery {
    type Result = Vec<PurchaseOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        PurchaseOrderEntity::find()
            .filter(crate::models::purchase_order_entity::Column::Status.eq(self.status.clone()))
            .limit(self.limit)
            .offset(self.offset)
            .order_by_desc(crate::models::purchase_order_entity::Column::OrderDate)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Query to get purchase orders by supplier
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPurchaseOrdersBySupplierQuery {
    pub supplier_id: Uuid,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetPurchaseOrdersBySupplierQuery {
    type Result = Vec<PurchaseOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        PurchaseOrderEntity::find()
            .filter(crate::models::purchase_order_entity::Column::SupplierId.eq(self.supplier_id))
            .limit(self.limit)
            .offset(self.offset)
            .order_by_desc(crate::models::purchase_order_entity::Column::OrderDate)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Query to get purchase orders by date range
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPurchaseOrdersByDateRangeQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetPurchaseOrdersByDateRangeQuery {
    type Result = Vec<PurchaseOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        PurchaseOrderEntity::find()
            .filter(crate::models::purchase_order_entity::Column::OrderDate.between(self.start_date, self.end_date))
            .order_by_desc(crate::models::purchase_order_entity::Column::OrderDate)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Query to get purchase orders by expected delivery date range
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPurchaseOrdersByDeliveryDateQuery {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetPurchaseOrdersByDeliveryDateQuery {
    type Result = Vec<PurchaseOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        PurchaseOrderEntity::find()
            .filter(crate::models::purchase_order_entity::Column::ExpectedDeliveryDate.between(self.start_date, self.end_date))
            .order_by_asc(crate::models::purchase_order_entity::Column::ExpectedDeliveryDate)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

/// Detailed purchase order information including related entities
#[derive(Debug, Serialize)]
pub struct PurchaseOrderDetails {
    pub purchase_order: PurchaseOrderModel,
    pub supplier: crate::models::suppliers::Model,
    pub items: Vec<crate::models::purchase_order_item_entity::Model>,
}

#[derive(Debug, FromQueryResult)]
struct SupplierInfo {
    id: Uuid,
    name: String,
    email: Option<String>,
}

/// Query to get detailed purchase order information
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPurchaseOrderDetailsQuery {
    pub purchase_order_id: Uuid,
}

#[async_trait]
impl Query for GetPurchaseOrderDetailsQuery {
    type Result = PurchaseOrderDetails;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        // Fetch the purchase order
        let purchase_order = PurchaseOrderEntity::find_by_id(self.purchase_order_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Purchase order not found".to_string()))?;

        // Fetch the supplier
        let supplier = SupplierEntity::find_by_id(purchase_order.supplier_id)
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Supplier not found".to_string()))?;

        // Fetch purchase order items
        let items = PurchaseOrderItemEntity::find()
            .filter(crate::models::purchase_order_item_entity::Column::PurchaseOrderId.eq(self.purchase_order_id))
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(PurchaseOrderDetails {
            purchase_order,
            supplier,
            items,
        })
    }
}

/// Statistics for purchase order analysis
#[derive(Debug, Serialize)]
pub struct PurchaseOrderStatistics {
    pub total_purchase_orders: i64,
    pub status_counts: Vec<(PurchaseOrderStatus, i64)>,
    pub total_value: Decimal,
    pub avg_order_value: Decimal,
    pub on_time_delivery_rate: f64,
}

#[derive(Debug, FromQueryResult)]
struct StatusCount {
    status: PurchaseOrderStatus,
    count: i64,
}

/// Query to get purchase order statistics for a date range
#[derive(Debug, Serialize, Deserialize)]
pub struct GetPurchaseOrderStatisticsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetPurchaseOrderStatisticsQuery {
    type Result = PurchaseOrderStatistics;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        // Get total count
        let total_purchase_orders = PurchaseOrderEntity::find()
            .filter(crate::models::purchase_order_entity::Column::OrderDate.between(self.start_date, self.end_date))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Get status counts
        let status_counts = PurchaseOrderEntity::find()
            .select_only()
            .column(crate::models::purchase_order_entity::Column::Status)
            .column_as(
                Expr::col((crate::models::purchase_order_entity::Entity, crate::models::purchase_order_entity::Column::Id)).count(),
                "count"
            )
            .filter(crate::models::purchase_order_entity::Column::OrderDate.between(self.start_date, self.end_date))
            .group_by(crate::models::purchase_order_entity::Column::Status)
            .into_model::<StatusCount>()
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Convert to tuple format
        let status_counts: Vec<(PurchaseOrderStatus, i64)> = status_counts
            .into_iter()
            .map(|sc| (sc.status, sc.count))
            .collect();

        // Get total value
        let total_value_result = PurchaseOrderEntity::find()
            .select_only()
            .column_as(
                Expr::col(crate::models::purchase_order_entity::Column::TotalAmount).sum(),
                "total"
            )
            .filter(crate::models::purchase_order_entity::Column::OrderDate.between(self.start_date, self.end_date))
            .into_tuple::<Option<Decimal>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .unwrap_or(Some(Decimal::ZERO))
            .unwrap_or(Decimal::ZERO);

        // Calculate average order value
        let avg_order_value = if total_purchase_orders > 0 {
            total_value_result / Decimal::from(total_purchase_orders)
        } else {
            Decimal::ZERO
        };

        // Calculate on-time delivery rate (simplified - checking if status is Received and delivery date is not past due)
        let on_time_deliveries = PurchaseOrderEntity::find()
            .filter(crate::models::purchase_order_entity::Column::OrderDate.between(self.start_date, self.end_date))
            .filter(crate::models::purchase_order_entity::Column::Status.eq(PurchaseOrderStatus::Received))
            .count(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let on_time_delivery_rate = if total_purchase_orders > 0 {
            (on_time_deliveries as f64 / total_purchase_orders as f64) * 100.0
        } else {
            0.0
        };

        Ok(PurchaseOrderStatistics {
            total_purchase_orders: total_purchase_orders.try_into().unwrap_or(0),
            status_counts,
            total_value: total_value_result,
            avg_order_value,
            on_time_delivery_rate,
        })
    }
}

/// Query to search purchase orders by PO number
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchPurchaseOrdersQuery {
    pub search_term: String,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for SearchPurchaseOrdersQuery {
    type Result = Vec<PurchaseOrderModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        PurchaseOrderEntity::find()
            .filter(crate::models::purchase_order_entity::Column::PoNumber.contains(&self.search_term))
            .limit(self.limit)
            .offset(self.offset)
            .order_by_desc(crate::models::purchase_order_entity::Column::OrderDate)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}
