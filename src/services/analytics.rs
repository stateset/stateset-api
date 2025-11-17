use chrono::{DateTime, Duration, TimeZone, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tracing::info;
use utoipa::ToSchema;

use crate::{
    entities::{
        commerce::cart::{self, Entity as CartEntity},
        inventory_items::{Column as InventoryColumn, Entity as InventoryEntity},
        order::{Column as OrderColumn, Entity as OrderEntity},
        shipment::{Column as ShipmentColumn, Entity as ShipmentEntity},
    },
    errors::ServiceError,
};

const DEFAULT_LOW_STOCK_THRESHOLD: i32 = 10;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct SalesMetrics {
    pub total_orders: i64,
    pub total_revenue: Decimal,
    pub average_order_value: Decimal,
    pub orders_today: i64,
    pub revenue_today: Decimal,
    pub orders_this_week: i64,
    pub revenue_this_week: Decimal,
    pub orders_this_month: i64,
    pub revenue_this_month: Decimal,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct InventoryMetrics {
    pub total_products: i64,
    pub low_stock_items: i64,
    pub out_of_stock_items: i64,
    pub total_value: Decimal,
    pub average_stock_level: f64,
    pub low_stock_threshold: i32,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ShipmentMetrics {
    pub total_shipments: i64,
    pub pending_shipments: i64,
    pub shipped_today: i64,
    pub delivered_today: i64,
    pub average_delivery_time_hours: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DashboardMetrics {
    pub sales: SalesMetrics,
    pub inventory: InventoryMetrics,
    pub shipments: ShipmentMetrics,
    pub carts: CartMetrics,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CartMetrics {
    pub total_carts: i64,
    pub active_carts: i64,
    pub abandoned_carts: i64,
    pub converted_carts: i64,
    pub average_cart_value: Decimal,
}

/// Point-in-time revenue data used for sales trend charts.
#[derive(Debug, Serialize, Deserialize, ToSchema, PartialEq)]
pub struct SalesTrendPoint {
    /// ISO 8601 date string (YYYY-MM-DD)
    pub date: String,
    /// Total revenue captured on the date
    pub revenue: Decimal,
}

/// Analytics service for generating business intelligence reports
#[derive(Clone)]
pub struct AnalyticsService {
    db: Arc<DatabaseConnection>,
}

impl AnalyticsService {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    /// Get comprehensive dashboard metrics
    pub async fn get_dashboard_metrics(&self) -> Result<DashboardMetrics, ServiceError> {
        info!("Generating dashboard metrics");

        let sales = self.get_sales_metrics().await?;
        let inventory = self
            .get_inventory_metrics(DEFAULT_LOW_STOCK_THRESHOLD)
            .await?;
        let shipments = self.get_shipment_metrics().await?;
        let carts = self.get_cart_metrics().await?;

        Ok(DashboardMetrics {
            sales,
            inventory,
            shipments,
            carts,
            generated_at: Utc::now(),
        })
    }

    /// Get sales performance metrics
    pub async fn get_sales_metrics(&self) -> Result<SalesMetrics, ServiceError> {
        let db = &*self.db;
        let now = Utc::now();
        let today_start = Utc.from_utc_datetime(
            &now.date_naive()
                .and_hms_opt(0, 0, 0)
                .expect("valid start of day"),
        );
        let week_start = Utc.from_utc_datetime(
            &(now - Duration::days(7))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .expect("valid start of day"),
        );
        let month_start = Utc.from_utc_datetime(
            &(now - Duration::days(30))
                .date_naive()
                .and_hms_opt(0, 0, 0)
                .expect("valid start of day"),
        );

        // Total metrics
        let total_orders = OrderEntity::find()
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let all_orders = OrderEntity::find()
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let total_revenue: Decimal = all_orders
            .iter()
            .fold(Decimal::ZERO, |acc, o| acc + o.total_amount);

        let average_order_value = if total_orders > 0 {
            total_revenue / Decimal::from(total_orders)
        } else {
            Decimal::ZERO
        };

        // Today's metrics
        let orders_today = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(today_start))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let today_orders = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(today_start))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let revenue_today: Decimal = today_orders
            .iter()
            .fold(Decimal::ZERO, |acc, o| acc + o.total_amount);

        // This week's metrics
        let orders_this_week = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(week_start))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let week_orders = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(week_start))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let revenue_this_week: Decimal = week_orders
            .iter()
            .fold(Decimal::ZERO, |acc, o| acc + o.total_amount);

        // This month's metrics
        let orders_this_month = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(month_start))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let month_orders = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(month_start))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let revenue_this_month: Decimal = month_orders
            .iter()
            .fold(Decimal::ZERO, |acc, o| acc + o.total_amount);

        Ok(SalesMetrics {
            total_orders: total_orders as i64,
            total_revenue,
            average_order_value,
            orders_today: orders_today as i64,
            revenue_today,
            orders_this_week: orders_this_week as i64,
            revenue_this_week,
            orders_this_month: orders_this_month as i64,
            revenue_this_month,
        })
    }

    /// Get inventory health metrics
    pub async fn get_inventory_metrics(
        &self,
        low_stock_threshold: i32,
    ) -> Result<InventoryMetrics, ServiceError> {
        let db = &*self.db;

        let total_products = InventoryEntity::find()
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let low_stock_items = InventoryEntity::find()
            .filter(InventoryColumn::Available.lte(low_stock_threshold))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let out_of_stock_items = InventoryEntity::find()
            .filter(InventoryColumn::Available.eq(0))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let all_inventory = InventoryEntity::find()
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let total_value: Decimal = all_inventory.iter().fold(Decimal::ZERO, |acc, item| {
            let unit_cost = item.unit_cost.unwrap_or(Decimal::ZERO);
            acc + (unit_cost * Decimal::from(item.available))
        });

        let average_stock_level = if !all_inventory.is_empty() {
            let total_stock: i32 = all_inventory.iter().map(|i| i.available).sum();
            total_stock as f64 / all_inventory.len() as f64
        } else {
            0.0
        };

        Ok(InventoryMetrics {
            total_products: total_products as i64,
            low_stock_items: low_stock_items as i64,
            out_of_stock_items: out_of_stock_items as i64,
            total_value,
            average_stock_level,
            low_stock_threshold,
        })
    }

    /// Get shipment performance metrics
    pub async fn get_shipment_metrics(&self) -> Result<ShipmentMetrics, ServiceError> {
        let db = &*self.db;
        let today_start = {
            let now = Utc::now();
            Utc.from_utc_datetime(
                &now.date_naive()
                    .and_hms_opt(0, 0, 0)
                    .expect("valid start of day"),
            )
        };

        let total_shipments = ShipmentEntity::find()
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let pending_shipments = ShipmentEntity::find()
            .filter(ShipmentColumn::Status.eq("pending"))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let shipped_today = ShipmentEntity::find()
            .filter(ShipmentColumn::CreatedAt.gte(today_start))
            .filter(ShipmentColumn::Status.eq("shipped"))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let delivered_today = ShipmentEntity::find()
            .filter(ShipmentColumn::CreatedAt.gte(today_start))
            .filter(ShipmentColumn::Status.eq("delivered"))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Calculate average delivery time (simplified)
        let completed_shipments = ShipmentEntity::find()
            .filter(ShipmentColumn::Status.eq("delivered"))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let average_delivery_time_hours = if !completed_shipments.is_empty() {
            let total_hours: i64 = completed_shipments
                .iter()
                .map(|s| (s.updated_at - s.created_at).num_hours())
                .sum();
            Some(total_hours as f64 / completed_shipments.len() as f64)
        } else {
            None
        };

        Ok(ShipmentMetrics {
            total_shipments: total_shipments as i64,
            pending_shipments: pending_shipments as i64,
            shipped_today: shipped_today as i64,
            delivered_today: delivered_today as i64,
            average_delivery_time_hours,
        })
    }

    pub async fn get_cart_metrics(&self) -> Result<CartMetrics, ServiceError> {
        let db = &*self.db;

        let total_carts = CartEntity::find()
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        let active_carts = CartEntity::find()
            .filter(cart::Column::Status.eq(cart::CartStatus::Active))
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        let abandoned_carts = CartEntity::find()
            .filter(cart::Column::Status.eq(cart::CartStatus::Abandoned))
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        let converted_carts = CartEntity::find()
            .filter(cart::Column::Status.eq(cart::CartStatus::Converted))
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        let carts = CartEntity::find()
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        let average_cart_value = if carts.is_empty() {
            Decimal::ZERO
        } else {
            let sum: Decimal = carts.iter().map(|c| c.total).sum();
            sum / Decimal::from(carts.len() as u64)
        };

        Ok(CartMetrics {
            total_carts: total_carts as i64,
            active_carts: active_carts as i64,
            abandoned_carts: abandoned_carts as i64,
            converted_carts: converted_carts as i64,
            average_cart_value,
        })
    }

    /// Get sales trends over time
    pub async fn get_sales_trends(
        &self,
        days: i32,
        status: Option<String>,
    ) -> Result<Vec<SalesTrendPoint>, ServiceError> {
        let db = &*self.db;
        let start_date = Utc::now() - Duration::days(days as i64);

        let mut query = OrderEntity::find().filter(OrderColumn::CreatedAt.gte(start_date));
        if let Some(status) = status {
            query = query.filter(OrderColumn::Status.eq(status));
        }

        let orders = query
            .order_by_asc(OrderColumn::CreatedAt)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        // Group by date and sum revenue
        let mut daily_revenue: BTreeMap<String, Decimal> = BTreeMap::new();

        for order in orders {
            let date_key = order.created_at.format("%Y-%m-%d").to_string();
            *daily_revenue.entry(date_key).or_insert(Decimal::ZERO) += order.total_amount;
        }

        Ok(daily_revenue
            .into_iter()
            .map(|(date, revenue)| SalesTrendPoint { date, revenue })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sales_trend_point_serializes_to_json_object() {
        let point = SalesTrendPoint {
            date: "2024-02-01".to_string(),
            revenue: Decimal::new(12345, 2),
        };

        let json = serde_json::to_value(point).expect("serialize trend point");
        assert_eq!(json["date"], serde_json::json!("2024-02-01"));
        assert_eq!(json["revenue"], serde_json::json!("123.45"));
    }
}
