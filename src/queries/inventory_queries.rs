use crate::{
    db::DbPool, 
    errors::ServiceError, 
    models::{
        inventory_item_entity::{Entity as InventoryItem, Model as InventoryItemModel},
        inventory_snapshot::{Entity as InventorySnapshot, Model as InventorySnapshotModel},
        product_entity::{Entity as Product, Model as ProductModel},
        inventory_transaction_entity::{Entity as InventoryMovement, Model as InventoryMovementModel},
        order_item_entity::{Entity as OrderItem, Model as OrderItemModel},
        order_entity::{Entity as Order, Model as OrderModel},
    }
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    prelude::*, query::*, EntityTrait, QueryFilter, QueryOrder, QuerySelect, RelationTrait,
    sea_query::{Func, SimpleExpr, Alias}, DatabaseConnection, IntoSimpleExpr, FromQueryResult,
};
use sea_orm::sea_query::func::*;
use sea_orm::sea_query::Func as SeaFunc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Models are imported via wildcard above

#[async_trait]
pub trait Query: Send + Sync {
    type Result: Send + Sync;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryItemQuery {
    pub product_id: i32,
}

#[async_trait]
impl Query for GetInventoryItemQuery {
    type Result = InventoryItemModel;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        InventoryItem::find()
            .filter(<crate::models::inventory_item_entity::Entity as sea_orm::EntityTrait>::Column::ProductId.eq(self.product_id))
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Inventory item not found".to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryItemByProductQuery {
    pub product_id: i32,
}

#[async_trait]
impl Query for GetInventoryItemByProductQuery {
    type Result = InventoryItemModel;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        InventoryItem::find()
            .filter(crate::models::inventory_item_entity::Column::ProductId.eq(self.product_id))
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| ServiceError::NotFound("Inventory item not found".to_string()))
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
    type Result = Vec<(InventoryItemModel, Option<ProductModel>)>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        InventoryItem::find()
            .find_also_related(Product)
            .filter(crate::models::inventory_item_entity::Column::Quantity.le(self.threshold))
            .order_by_asc(crate::models::inventory_item_entity::Column::Quantity)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
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

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        use sea_orm::sea_query::Expr;
        
        let result = InventoryItem::find()
            .select_only()
            .column_as(
                Expr::col((crate::models::inventory_item_entity::Entity, crate::models::inventory_item_entity::Column::Quantity))
                    .cast_as(Alias::new("float8"))
                    .mul(Expr::col((crate::models::inventory_item_entity::Entity, crate::models::inventory_item_entity::Column::UnitPrice)))
                    .sum(),
                "total_value",
            )
            .column_as(
                Expr::col((crate::models::inventory_item_entity::Entity, crate::models::inventory_item_entity::Column::Id)).count(),
                "total_items"
            )
            .into_tuple::<(Option<f64>, Option<i64>)>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        match result {
            Some((total_value, total_items)) => Ok(InventoryValue {
                total_value: total_value.unwrap_or(0.0),
                total_items: total_items.unwrap_or(0),
            }),
            None => Ok(InventoryValue {
                total_value: 0.0,
                total_items: 0,
            }),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetInventoryMovementsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
    pub offset: u64,
}

#[async_trait]
impl Query for GetInventoryMovementsQuery {
    type Result = Vec<InventoryMovementModel>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        
        InventoryMovement::find()
            .filter(crate::models::inventory_transaction_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .order_by_desc(crate::models::inventory_transaction_entity::Column::CreatedAt)
            .limit(self.limit)
            .offset(self.offset)
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetTopSellingProductsQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub limit: u64,
}

#[derive(Debug, Serialize, FromQueryResult)]
pub struct TopSellingProduct {
    pub product_id: i32,
    pub product_name: String,
    pub quantity_sold: i64,
    pub total_revenue: f64,
}

#[async_trait]
impl Query for GetTopSellingProductsQuery {
    type Result = Vec<TopSellingProduct>;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        use sea_orm::sea_query::Expr;
        
        let results = OrderItem::find()
            .inner_join(Order)
            .inner_join(Product)
            .filter(crate::models::order_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .group_by(crate::models::product_entity::Column::Id)
            .group_by(crate::models::product_entity::Column::Name)
            .select_only()
            .column(crate::models::product_entity::Column::Id)
            .column(crate::models::product_entity::Column::Name)
            .column_as(
                Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::Quantity)).sum(),
                "quantity_sold"
            )
            .column_as(
                Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::Quantity))
                    .cast_as(Alias::new("float8"))
                    .mul(Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::UnitPrice)))
                    .sum(),
                "revenue"
            )
            .order_by_desc(Expr::col(Alias::new("quantity_sold")))
            .limit(self.limit)
            .into_model::<TopSellingProduct>()
            .all(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(results)
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

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        use sea_orm::sea_query::Expr;
        
        // Get average inventory value
        let avg_inventory_result = InventorySnapshot::find()
            .filter(crate::models::inventory_snapshot::Column::Date.between(self.start_date, self.end_date))
            .select_only()
            .column_as(
                Expr::col((crate::models::inventory_snapshot::Entity, crate::models::inventory_snapshot::Column::TotalValue)).avg(),
                "avg_value"
            )
            .into_tuple::<Option<rust_decimal::Decimal>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let avg_inventory_value = avg_inventory_result
            .flatten()
            .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0))
            .unwrap_or(0.0);

        // Get cost of goods sold
        let cogs_result = OrderItem::find()
            .inner_join(Order)
            .filter(crate::models::order_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .select_only()
            .column_as(
                Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::Quantity))
                    .cast_as(Alias::new("float8"))
                    .mul(Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::UnitPrice)))
                    .sum(),
                "cogs"
            )
            .into_tuple::<Option<f64>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let cost_of_goods_sold = cogs_result.flatten().unwrap_or(0.0);

        let ratio = if avg_inventory_value > 0.0 {
            cost_of_goods_sold / avg_inventory_value
        } else {
            0.0
        };

        Ok(InventoryTurnoverRatio {
            ratio,
            average_inventory_value: avg_inventory_value,
            cost_of_goods_sold,
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

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        use sea_orm::sea_query::Expr;
        
        let current_stock_result = InventoryItem::find()
            .filter(crate::models::inventory_item_entity::Column::ProductId.eq(self.product_id))
            .select_only()
            .column(crate::models::inventory_item_entity::Column::Quantity)
            .into_tuple::<i32>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let current_stock = current_stock_result.unwrap_or(0);

        let end_date = Utc::now();
        let start_date = end_date - Duration::days(self.forecast_period as i64);

        let total_sold_result = OrderItem::find()
            .inner_join(Order)
            .filter(crate::models::order_item_entity::Column::ProductId.eq(uuid::Uuid::from_u128(self.product_id as u128)))
            .filter(crate::models::order_entity::Column::CreatedAt.between(start_date, end_date))
            .select_only()
            .column_as(
                Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::Quantity)).sum(),
                "total_sold"
            )
            .into_tuple::<Option<i64>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let total_sold = total_sold_result.flatten().unwrap_or(0) as i32;
        let average_daily_demand = total_sold as f64 / self.forecast_period as f64;
        let forecasted_demand = (average_daily_demand * self.forecast_period as f64).round() as i32;
        let recommended_reorder = if forecasted_demand > current_stock {
            forecasted_demand - current_stock
        } else {
            0
        };

        Ok(InventoryForecast {
            product_id: self.product_id,
            current_stock,
            forecasted_demand,
            recommended_reorder,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetAverageInventoryValueQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetAverageInventoryValueQuery {
    type Result = f64;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        use sea_orm::sea_query::Expr;
        
        let avg_value = InventorySnapshot::find()
            .filter(crate::models::inventory_snapshot::Column::Date.between(self.start_date, self.end_date))
            .select_only()
            .column_as(
                Expr::col((crate::models::inventory_snapshot::Entity, crate::models::inventory_snapshot::Column::TotalValue)).avg(),
                "avg_value"
            )
            .into_tuple::<Option<rust_decimal::Decimal>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(avg_value.flatten().map(|d| d.to_string().parse::<f64>().unwrap_or(0.0)).unwrap_or(0.0))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCostOfGoodsSoldQuery {
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
}

#[async_trait]
impl Query for GetCostOfGoodsSoldQuery {
    type Result = f64;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        use sea_orm::sea_query::Expr;
        
        let cogs = OrderItem::find()
            .inner_join(Order)
            .filter(crate::models::order_entity::Column::CreatedAt.between(self.start_date, self.end_date))
            .select_only()
            .column_as(
                Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::Quantity))
                    .cast_as(Alias::new("float8"))
                    .mul(Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::UnitPrice)))
                    .sum(),
                "cogs"
            )
            .into_tuple::<Option<f64>>()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        Ok(cogs.flatten().unwrap_or(0.0))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ForecastDemandQuery {
    pub product_id: i32,
    pub days_ahead: i64,
}

#[derive(Debug, Serialize)]
pub struct DemandForecast {
    pub product_id: i32,
    pub forecasted_demand: i32,
    pub current_stock: i32,
    pub needs_reorder: bool,
}

#[async_trait]
impl Query for ForecastDemandQuery {
    type Result = DemandForecast;

    async fn execute(&self, db_pool: &DatabaseConnection) -> Result<Self::Result, ServiceError> {
        use sea_orm::sea_query::Expr;
        
        let end_date = Utc::now();
        let start_date = end_date - Duration::days(30);
        
        let current_stock: Option<i32> = InventoryItem::find()
            .filter(crate::models::inventory_item_entity::Column::ProductId.eq(self.product_id))
            .select_only()
            .column(crate::models::inventory_item_entity::Column::Quantity)
            .into_tuple()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let avg_daily_sales: Option<f64> = OrderItem::find()
            .inner_join(Order)
            .filter(crate::models::order_item_entity::Column::ProductId.eq(uuid::Uuid::from_u128(self.product_id as u128)))
            .filter(crate::models::order_entity::Column::CreatedAt.between(start_date, end_date))
            .select_only()
            .column_as(
                Expr::col((crate::models::order_item_entity::Entity, crate::models::order_item_entity::Column::Quantity))
                    .sum()
                    .div(30.0),
                "avg_daily_sales"
            )
            .into_tuple()
            .one(db_pool)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .flatten();

        let forecasted_demand = (avg_daily_sales.unwrap_or(0.0) * self.days_ahead as f64).round() as i32;
        let current_stock = current_stock.unwrap_or(0);
        let needs_reorder = forecasted_demand > current_stock;

        Ok(DemandForecast {
            product_id: self.product_id,
            forecasted_demand,
            current_stock,
            needs_reorder,
        })
    }
}
