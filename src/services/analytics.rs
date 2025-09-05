use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use sea_orm::{DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, ColumnTrait};
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use rust_decimal::Decimal;

use crate::{
    errors::ServiceError,
    entities::{
        order::{Entity as OrderEntity, Column as OrderColumn},
        inventory_items::{Entity as InventoryEntity, Column as InventoryColumn},
        shipment::{Entity as ShipmentEntity, Column as ShipmentColumn},
    },
};

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryMetrics {
    pub total_products: i64,
    pub low_stock_items: i64,
    pub out_of_stock_items: i64,
    pub total_value: Decimal,
    pub average_stock_level: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ShipmentMetrics {
    pub total_shipments: i64,
    pub pending_shipments: i64,
    pub shipped_today: i64,
    pub delivered_today: i64,
    pub average_delivery_time_hours: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub sales: SalesMetrics,
    pub inventory: InventoryMetrics,
    pub shipments: ShipmentMetrics,
    pub generated_at: DateTime<Utc>,
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
        let inventory = self.get_inventory_metrics().await?;
        let shipments = self.get_shipment_metrics().await?;

        Ok(DashboardMetrics {
            sales,
            inventory,
            shipments,
            generated_at: Utc::now(),
        })
    }

    /// Get sales performance metrics
    pub async fn get_sales_metrics(&self) -> Result<SalesMetrics, ServiceError> {
        let db = &*self.db;
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let week_start = (now - Duration::days(7)).date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let month_start = (now - Duration::days(30)).date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

        // Total metrics
        let total_orders = OrderEntity::find().count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let all_orders = OrderEntity::find().all(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let total_revenue: Decimal = all_orders.iter()
            .map(|o| o.total_amount)
            .sum();

        let average_order_value = if total_orders > 0 {
            total_revenue / Decimal::from(total_orders)
        } else {
            Decimal::ZERO
        };

        // Today's metrics
        let orders_today = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(today_start))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let today_orders = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(today_start))
            .all(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let revenue_today: Decimal = today_orders.iter()
            .map(|o| o.total_amount)
            .sum();

        // This week's metrics
        let orders_this_week = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(week_start))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let week_orders = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(week_start))
            .all(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let revenue_this_week: Decimal = week_orders.iter()
            .map(|o| o.total_amount)
            .sum();

        // This month's metrics
        let orders_this_month = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(month_start))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let month_orders = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(month_start))
            .all(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let revenue_this_month: Decimal = month_orders.iter()
            .map(|o| o.total_amount)
            .sum();

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
    pub async fn get_inventory_metrics(&self) -> Result<InventoryMetrics, ServiceError> {
        let db = &*self.db;

        let total_products = InventoryEntity::find().count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let low_stock_items = InventoryEntity::find()
            .filter(InventoryColumn::Available.lte(10))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let out_of_stock_items = InventoryEntity::find()
            .filter(InventoryColumn::Available.eq(0))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let all_inventory = InventoryEntity::find().all(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let total_value: Decimal = all_inventory.iter()
            .filter_map(|i| {
                i.unit_cost.zip(i.available.into())
                    .map(|(cost, qty)| cost * Decimal::from(qty))
            })
            .sum();

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
        })
    }

    /// Get shipment performance metrics
    pub async fn get_shipment_metrics(&self) -> Result<ShipmentMetrics, ServiceError> {
        let db = &*self.db;
        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

        let total_shipments = ShipmentEntity::find().count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let pending_shipments = ShipmentEntity::find()
            .filter(ShipmentColumn::Status.eq("pending"))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let shipped_today = ShipmentEntity::find()
            .filter(ShipmentColumn::CreatedAt.gte(today_start))
            .filter(ShipmentColumn::Status.eq("shipped"))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let delivered_today = ShipmentEntity::find()
            .filter(ShipmentColumn::CreatedAt.gte(today_start))
            .filter(ShipmentColumn::Status.eq("delivered"))
            .count(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        // Calculate average delivery time (simplified)
        let completed_shipments = ShipmentEntity::find()
            .filter(ShipmentColumn::Status.eq("delivered"))
            .all(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        let average_delivery_time_hours = if !completed_shipments.is_empty() {
            let total_hours: i64 = completed_shipments.iter()
                .filter_map(|s| {
                    s.created_at.zip(s.updated_at).map(|(created, updated)| {
                        (updated - created).num_hours()
                    })
                })
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

    /// Get sales trends over time
    pub async fn get_sales_trends(&self, days: i32) -> Result<Vec<(String, Decimal)>, ServiceError> {
        let db = &*self.db;
        let start_date = Utc::now() - Duration::days(days as i64);

        let orders = OrderEntity::find()
            .filter(OrderColumn::CreatedAt.gte(start_date))
            .order_by_asc(OrderColumn::CreatedAt)
            .all(db).await
            .map_err(|e| ServiceError::DatabaseError(e.into()))?;

        // Group by date and sum revenue
        let mut daily_revenue: std::collections::HashMap<String, Decimal> = std::collections::HashMap::new();

        for order in orders {
            let date_key = order.created_at.format("%Y-%m-%d").to_string();
            *daily_revenue.entry(date_key).or_insert(Decimal::ZERO) += order.total_amount;
        }

        let mut result: Vec<(String, Decimal)> = daily_revenue.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));

        Ok(result)
    }
}
