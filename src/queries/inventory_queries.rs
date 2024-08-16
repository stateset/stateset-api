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

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryItemQuery {
    pub product_id: i32,
}

#[async_trait]
impl Query for GetInventoryItemQuery {
    type Result = inventory_item_entity::Model;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        inventory_item_entity::Entity::find()
            .filter(inventory_item_entity::Column::ProductId.eq(self.product_id))
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .ok_or(ServiceError::NotFound)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetLowStockItemsQuery {
    pub threshold: i32,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetLowStockItemsQuery {
    type Result = Vec<(product_entity::Model, inventory_item_entity::Model)>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        product_entity::Entity::find()
            .join_rev(
                JoinType::InnerJoin,
                inventory_item_entity::Entity::belongs_to(product_entity::Entity)
                    .from(inventory_item_entity::Column::ProductId)
                    .to(product_entity::Column::Id)
                    .into(),
            )
            .filter(inventory_item_entity::Column::Quantity.le(self.threshold))
            .order_by_asc(inventory_item_entity::Column::Quantity)
            .limit(self.limit)
            .offset(self.offset)
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryValueQuery {}

#[derive(Debug, Serialize)]
pub struct InventoryValue {
    pub total_value: f64,
    pub total_items: i64,
}

#[async_trait]
impl Query for GetInventoryValueQuery {
    type Result = InventoryValue;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let result = inventory_item_entity::Entity::find()
            .select_only()
            .column_as(
                sum(inventory_item_entity::Column::Quantity.cast::<f64>() * inventory_item_entity::Column::UnitCost),
                "total_value",
            )
            .column_as(count(inventory_item_entity::Column::Id), "total_items")
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(InventoryValue {
            total_value: result.0.unwrap_or(0.0),
            total_items: result.1.unwrap_or(0),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryMovementsQuery {
    pub product_id: i32,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetInventoryMovementsQuery {
    type Result = Vec<inventory_movement_entity::Model>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        inventory_movement_entity::Entity::find()
            .filter(inventory_movement_entity::Column::ProductId.eq(self.product_id))
            .filter(inventory_movement_entity::Column::Timestamp.between(self.start_date, self.end_date))
            .order_by_desc(inventory_movement_entity::Column::Timestamp)
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
    pub product_id: i32,
    pub product_name: String,
    pub quantity_sold: i64,
    pub total_revenue: f64,
}

#[async_trait]
impl Query for GetTopSellingProductsQuery {
    type Result = Vec<TopSellingProduct>;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let top_selling = order_item_entity::Entity::find()
            .inner_join(product_entity::Entity)
            .inner_join(order_entity::Entity)
            .filter(order_entity::Column::OrderDate.between(self.start_date, self.end_date))
            .group_by(product_entity::Column::Id)
            .order_by_desc(sum(order_item_entity::Column::Quantity))
            .select_only()
            .column(product_entity::Column::Id)
            .column(product_entity::Column::Name)
            .column_as(sum(order_item_entity::Column::Quantity), "quantity_sold")
            .column_as(
                sum(order_item_entity::Column::Quantity.cast::<f64>() * order_item_entity::Column::UnitPrice),
                "total_revenue",
            )
            .into_tuple()
            .all(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?;

        Ok(top_selling
            .into_iter()
            .map(|(product_id, product_name, quantity_sold, total_revenue)| TopSellingProduct {
                product_id,
                product_name,
                quantity_sold: quantity_sold.unwrap_or(0),
                total_revenue: total_revenue.unwrap_or(0.0),
            })
            .collect())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryTurnoverRatioQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct InventoryTurnoverRatio {
    pub ratio: f64,
    pub average_inventory_value: f64,
    pub cost_of_goods_sold: f64,
}

#[async_trait]
impl Query for GetInventoryTurnoverRatioQuery {
    type Result = InventoryTurnoverRatio;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let avg_inventory = inventory_snapshot_entity::Entity::find()
            .filter(inventory_snapshot_entity::Column::Date.between(self.start_date, self.end_date))
            .select_only()
            .column_as(avg(inventory_snapshot_entity::Column::TotalValue), "average_inventory_value")
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        let cogs = order_item_entity::Entity::find()
            .inner_join(order_entity::Entity)
            .filter(order_entity::Column::OrderDate.between(self.start_date, self.end_date))
            .select_only()
            .column_as(sum(order_item_entity::Column::Quantity.cast::<f64>() * order_item_entity::Column::UnitCost), "cost_of_goods_sold")
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::DatabaseError)?
            .unwrap_or(0.0);

        let ratio = if avg_inventory > 0.0 { cogs / avg_inventory } else { 0.0 };

        Ok(InventoryTurnoverRatio {
            ratio,
            average_inventory_value: avg_inventory,
            cost_of_goods_sold: cogs,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryForecastQuery {
    pub product_id: i32,
    pub forecast_period: i32, // in days
}

#[derive(Debug, Serialize)]
pub struct InventoryForecast {
    pub product_id: i32,
    pub current_stock: i32,
    pub forecasted_demand: i32,
    pub recommended_reorder: i32,
}

#[async_trait]
impl Query for GetInventoryForecastQuery {
    type Result = InventoryForecast;

    async fn execute(&self, db_pool: Arc<DbPool>) -> Result<Self::Result, ServiceError> {
        let db = db_pool.get().map_err(|_| ServiceError::DatabaseError)?;

        let current_stock = inventory_item_entity::Entity::find()
            .filter(inventory_item_entity::Column::ProductId.eq(self.product_id))
            .select_only()
            .column(inventory_item_entity::Column::Quantity)
            .into_tuple()
            .one(&db)
            .await
            .map_err(|_| ServiceError::NotFound)?
            .unwrap_or(0);

        let end_date = Utc
    }
}