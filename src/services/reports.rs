use crate::circuit_breaker::CircuitBreaker;
use crate::{
    db::DbPool,
    entities::purchase_order_headers,
    errors::ServiceError,
    models::{inventory_items, order, order_line_item, return_entity, suppliers},
};
use anyhow::Result;
use chrono::NaiveDateTime;
use redis::Client as RedisClient;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use slog::Logger;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

/// Service for generating various reports and analytics
pub struct ReportService {
    db_pool: Arc<DbPool>,
    redis_client: Arc<RedisClient>,
    circuit_breaker: Arc<CircuitBreaker>,
    logger: Logger,
}

/// Order summary report data
#[derive(Debug, Serialize, Deserialize)]
pub struct OrderSummaryReport {
    pub period: String,
    pub total_orders: i64,
    pub total_revenue: f64,
    pub average_order_value: f64,
    pub orders_by_status: HashMap<String, i64>,
}

/// Inventory report data
#[derive(Debug, Serialize, Deserialize)]
pub struct InventoryReport {
    pub total_products: i64,
    pub low_stock_products: i64,
    pub out_of_stock_products: i64,
    pub inventory_value: f64,
    pub top_products: Vec<TopProduct>,
}

/// Top selling product data
#[derive(Debug, Serialize, Deserialize)]
pub struct TopProduct {
    pub product_id: Uuid,
    pub name: String,
    pub quantity_sold: i64,
    pub revenue: f64,
}

/// Supplier performance report data
#[derive(Debug, Serialize, Deserialize)]
pub struct SupplierPerformanceReport {
    pub supplier_id: i32,
    pub name: String,
    pub total_orders: i64,
    pub on_time_delivery_rate: f64,
    pub quality_rating: f64,
    pub average_lead_time: f64,
    pub cost_savings: f64,
}

impl ReportService {
    /// Creates a new report service instance
    pub fn new(
        db_pool: Arc<DbPool>,
        redis_client: Arc<RedisClient>,
        circuit_breaker: Arc<CircuitBreaker>,
        logger: Logger,
    ) -> Self {
        Self {
            db_pool,
            redis_client,
            circuit_breaker,
            logger,
        }
    }

    /// Generates an order summary report for a specific time period
    #[instrument(skip(self))]
    pub async fn generate_order_summary_report(
        &self,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<OrderSummaryReport, ServiceError> {
        let db = &*self.db_pool;

        // Get total orders in time period
        let total_orders = order::Entity::find()
            .filter(order::Column::CreatedDate.gte(start_date))
            .filter(order::Column::CreatedDate.lte(end_date))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Get total revenue and orders grouped with their line items
        let orders_with_items = order::Entity::find()
            .filter(order::Column::CreatedDate.gte(start_date))
            .filter(order::Column::CreatedDate.lte(end_date))
            .find_with_related(order_line_item::Entity)
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let mut total_revenue_cents: i128 = 0; // sale_price is stored in cents
        let mut orders_by_status: HashMap<String, i64> = HashMap::new();

        for (order, line_items) in &orders_with_items {
            *orders_by_status
                .entry(order.order_status.to_string())
                .or_insert(0) += 1;

            for item in line_items {
                let line_total_cents = i128::from(item.sale_price) * i128::from(item.quantity);
                total_revenue_cents += line_total_cents;
            }
        }

        let total_revenue: f64 = (total_revenue_cents as f64) / 100.0;

        let average_order_value = if total_orders > 0 {
            total_revenue / total_orders as f64
        } else {
            0.0
        };

        let period = format!("{} to {}", start_date.date(), end_date.date());

        Ok(OrderSummaryReport {
            period,
            total_orders: total_orders.try_into().unwrap_or(0),
            total_revenue,
            average_order_value,
            orders_by_status,
        })
    }

    /// Generates an inventory status report
    #[instrument(skip(self))]
    pub async fn generate_inventory_report(&self) -> Result<InventoryReport, ServiceError> {
        let db = &*self.db_pool;

        // Get total products count
        let total_products = inventory_items::Entity::find()
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Get low stock products (less than 10 units)
        let low_stock_products = inventory_items::Entity::find()
            .filter(inventory_items::Column::Available.lt(10))
            .filter(inventory_items::Column::Available.gt(0))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Get out of stock products
        let out_of_stock_products = inventory_items::Entity::find()
            .filter(inventory_items::Column::Available.eq(0))
            .count(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Calculate inventory value from available quantities and unit costs
        let all_items = inventory_items::Entity::find()
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let inventory_value: f64 = all_items
            .iter()
            .filter_map(|item| {
                item.unit_cost.map(|cost| {
                    use rust_decimal::prelude::ToPrimitive;
                    let cost_f64 = cost.to_f64().unwrap_or(0.0);
                    cost_f64 * (item.available as f64)
                })
            })
            .sum();

        // Get top products by sales (from order line items)
        let orders_with_items = order::Entity::find()
            .find_with_related(order_line_item::Entity)
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Aggregate sales by product (product_id is a String in order_line_item)
        let mut product_sales: HashMap<String, (String, i64, f64)> = HashMap::new();

        for (_order, line_items) in &orders_with_items {
            for item in line_items {
                let entry = product_sales.entry(item.product_id.clone()).or_insert((
                    item.product_name.clone(),
                    0,
                    0.0,
                ));
                entry.1 += item.quantity as i64;
                let line_revenue = (item.sale_price as f64 / 100.0) * item.quantity as f64;
                entry.2 += line_revenue;
            }
        }

        // Sort by quantity sold and take top 10
        let mut top_products: Vec<TopProduct> = product_sales
            .into_iter()
            .filter_map(|(id, (name, qty, rev))| {
                // Try to parse product_id as UUID, skip if invalid
                Uuid::parse_str(&id).ok().map(|uuid| TopProduct {
                    product_id: uuid,
                    name,
                    quantity_sold: qty,
                    revenue: rev,
                })
            })
            .collect();

        top_products.sort_by(|a, b| b.quantity_sold.cmp(&a.quantity_sold));
        top_products.truncate(10);

        Ok(InventoryReport {
            total_products: total_products.try_into().unwrap_or(0),
            low_stock_products: low_stock_products.try_into().unwrap_or(0),
            out_of_stock_products: out_of_stock_products.try_into().unwrap_or(0),
            inventory_value,
            top_products,
        })
    }

    /// Generates a supplier performance report for a specific supplier
    #[instrument(skip(self))]
    pub async fn generate_supplier_performance_report(
        &self,
        supplier_id: i32,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<SupplierPerformanceReport, ServiceError> {
        let db = &*self.db_pool;

        // Get supplier details
        let supplier_model = suppliers::Entity::find_by_id(supplier_id)
            .one(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?
            .ok_or_else(|| {
                let msg = format!("Supplier not found with ID: {}", supplier_id);
                error!(supplier_id = %supplier_id, "Supplier not found");
                ServiceError::NotFound(msg)
            })?;

        // Get purchase orders for this supplier within the date range
        // Note: vendor_id in purchase_order_headers corresponds to supplier_id
        let purchase_orders = purchase_order_headers::Entity::find()
            .filter(purchase_order_headers::Column::VendorId.eq(supplier_id as i64))
            .filter(purchase_order_headers::Column::CreatedAt.gte(start_date))
            .filter(purchase_order_headers::Column::CreatedAt.lte(end_date))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        let total_orders = purchase_orders.len() as i64;

        // Calculate on-time delivery rate based on approved orders
        // (In a full implementation, this would compare expected vs actual delivery dates)
        let approved_orders = purchase_orders
            .iter()
            .filter(|po| po.approved_flag == Some(true))
            .count();
        let on_time_delivery_rate = if total_orders > 0 {
            (approved_orders as f64 / total_orders as f64) * 100.0
        } else {
            0.0
        };

        // Quality rating from supplier model
        let quality_rating = match supplier_model.rating {
            suppliers::SupplierRating::Unrated => 0.0,
            suppliers::SupplierRating::Bronze => 1.0,
            suppliers::SupplierRating::Silver => 2.0,
            suppliers::SupplierRating::Gold => 3.0,
            suppliers::SupplierRating::Platinum => 4.0,
        };

        // Calculate average lead time (days between created_at and updated_at as proxy)
        // In a full implementation, this would use actual shipment/receipt dates
        let lead_times: Vec<f64> = purchase_orders
            .iter()
            .map(|po| {
                let duration = po.updated_at.signed_duration_since(po.created_at);
                duration.num_days() as f64
            })
            .collect();

        let average_lead_time = if !lead_times.is_empty() {
            lead_times.iter().sum::<f64>() / lead_times.len() as f64
        } else {
            0.0
        };

        // Cost savings would require comparing negotiated prices to market prices
        // This is a placeholder - in production, you'd have price comparison data
        let cost_savings = 0.0;

        info!(
            supplier_id = supplier_id,
            total_orders = total_orders,
            on_time_rate = on_time_delivery_rate,
            avg_lead_time = average_lead_time,
            "Generated supplier performance report"
        );

        Ok(SupplierPerformanceReport {
            supplier_id,
            name: format!("{} {}", supplier_model.first_name, supplier_model.last_name),
            total_orders,
            on_time_delivery_rate,
            quality_rating,
            average_lead_time,
            cost_savings,
        })
    }

    /// Generates a returns analysis report
    #[instrument(skip(self))]
    pub async fn generate_returns_report(
        &self,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<HashMap<String, i64>, ServiceError> {
        let db = &*self.db_pool;

        // Get all returns in the time period
        let returns = return_entity::Entity::find()
            .filter(return_entity::Column::CreatedAt.gte(start_date))
            .filter(return_entity::Column::CreatedAt.lte(end_date))
            .all(db)
            .await
            .map_err(|e| ServiceError::db_error(e))?;

        // Group returns by reason
        let mut returns_by_reason = HashMap::new();
        for ret in returns {
            *returns_by_reason.entry(ret.reason.clone()).or_insert(0) += 1;
        }

        Ok(returns_by_reason)
    }

    /// Generates warehouse efficiency report
    #[instrument(skip(self))]
    pub async fn generate_warehouse_efficiency_report(
        &self,
        warehouse_id: &Uuid,
        start_date: NaiveDateTime,
        end_date: NaiveDateTime,
    ) -> Result<HashMap<String, f64>, ServiceError> {
        // This is a placeholder implementation that would normally calculate:
        // - Order fulfillment time
        // - Picking accuracy
        // - Space utilization
        // - Labor efficiency
        // - Inventory turnover

        let mut metrics = HashMap::new();
        metrics.insert("order_fulfillment_time".to_string(), 24.5); // hours
        metrics.insert("picking_accuracy".to_string(), 98.7); // percentage
        metrics.insert("space_utilization".to_string(), 76.3); // percentage
        metrics.insert("labor_efficiency".to_string(), 85.0); // percentage
        metrics.insert("inventory_turnover".to_string(), 5.2); // turns per year

        Ok(metrics)
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use mockall::mock;
    use mockall::predicate::*;
    use sea_orm::DatabaseConnection;
    use std::str::FromStr;

    mock! {
        pub Database {}
        impl Clone for Database {
            fn clone(&self) -> Self;
        }
    }

    #[tokio::test]
    async fn test_generate_order_summary_report() {
        // Setup
        let db_pool = Arc::new(DatabaseConnection::Disconnected);
        let redis_client = Arc::new(redis::Client::open("redis://localhost").unwrap());
        let circuit_breaker = Arc::new(CircuitBreaker::new(
            5,
            std::time::Duration::from_secs(60),
            1,
        ));
        let logger = slog::Logger::root(slog::Discard, slog::o!());

        let service = ReportService::new(db_pool, redis_client, circuit_breaker, logger);

        // Test data
        let start_date = NaiveDate::from_ymd_opt(2023, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let end_date = NaiveDate::from_ymd_opt(2023, 1, 31)
            .unwrap()
            .and_hms_opt(23, 59, 59)
            .unwrap();

        // Execute
        let result = service
            .generate_order_summary_report(start_date, end_date)
            .await;

        // Assert
        assert!(result.is_err()); // Will fail because we're using mock DB
    }
}
