use std::sync::Arc;
use sea_orm::*;
use crate::{
    db::DbPool,
    errors::InventoryError,
    events::{Event, EventSender},
    models::{
        inventory_level_entity::{self, Entity as InventoryLevel},
        safety_stock_entity::{self, Entity as SafetyStock},
        safety_stock_alert_entity::{self, Entity as SafetyStockAlert},
    },
};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use uuid::Uuid;
use validator::Validate;
use prometheus::{IntCounter, IntGauge};
use lazy_static::lazy_static;
use chrono::{DateTime, Duration, Utc};

lazy_static! {
    static ref SAFETY_STOCK_QUERIES: IntCounter = 
        IntCounter::new("safety_stock_queries_total", "Total number of safety stock queries")
            .expect("metric can be created");

    static ref SAFETY_STOCK_QUERY_FAILURES: IntCounter = 
        IntCounter::new("safety_stock_query_failures_total", "Total number of failed safety stock queries")
            .expect("metric can be created");
            
    static ref ITEMS_BELOW_SAFETY_STOCK: IntGauge = 
        IntGauge::new("items_below_safety_stock_total", "Total number of items below safety stock level")
            .expect("metric can be created");
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct GetStockSafetyCommand {
    pub warehouse_id: String,
    pub categories: Option<Vec<String>>,
    pub as_of_date: Option<DateTime<Utc>>,
    #[validate(range(min = 1, max = 90))]
    pub lookback_days: Option<i32>,
    #[validate(length(max = 100))]
    pub sku_filters: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SafetyStockLevel {
    pub product_id: Uuid,
    pub sku: String,
    pub category: String,
    pub current_stock: i32,
    pub safety_stock_level: i32,
    pub reorder_point: i32,
    pub avg_daily_demand: f64,
    pub stock_coverage_days: f64,
    pub last_replenishment_date: Option<DateTime<Utc>>,
    pub next_replenishment_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetStockSafetyResult {
    pub warehouse_id: String,
    pub as_of_date: DateTime<Utc>,
    pub items: Vec<SafetyStockLevel>,
    pub summary: SafetyStockSummary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SafetyStockSummary {
    pub total_items: i32,
    pub items_below_safety_stock: i32,
    pub items_below_reorder_point: i32,
    pub average_coverage_days: f64,
    pub categories_at_risk: Vec<String>,
}

#[async_trait::async_trait]
impl Command for GetStockSafetyCommand {
    type Result = GetStockSafetyResult;

    #[instrument(skip(self, db_pool, event_sender))]
    async fn execute(
        &self,
        db_pool: Arc<DbPool>,
        event_sender: Arc<EventSender>,
    ) -> Result<Self::Result, InventoryError> {
        self.validate().map_err(|e| {
            SAFETY_STOCK_QUERY_FAILURES.inc();
            let msg = format!("Invalid input: {}", e);
            error!("{}", msg);
            InventoryError::ValidationError(msg)
        })?;

        let db = db_pool.as_ref();
        
        // Get current stock levels and safety stock configurations
        let safety_levels = self.get_safety_stock_levels(db).await?;
        
        // Calculate summary statistics
        let summary = self.calculate_summary(&safety_levels);
        
        // Update metrics
        SAFETY_STOCK_QUERIES.inc();
        ITEMS_BELOW_SAFETY_STOCK.set(summary.items_below_safety_stock as i64);

        // Trigger alerts if necessary
        self.check_and_trigger_alerts(db, event_sender.as_ref(), &safety_levels).await?;

        Ok(GetStockSafetyResult {
            warehouse_id: self.warehouse_id.clone(),
            as_of_date: self.as_of_date.unwrap_or_else(Utc::now),
            items: safety_levels,
            summary,
        })
    }
}

impl GetStockSafetyCommand {
    async fn get_safety_stock_levels(
        &self,
        db: &DatabaseConnection,
    ) -> Result<Vec<SafetyStockLevel>, InventoryError> {
        let mut query = InventoryLevel::find()
            .filter(inventory_level_entity::Column::WarehouseId.eq(&self.warehouse_id));

        // Apply category filter if specified
        if let Some(categories) = &self.categories {
            query = query.filter(inventory_level_entity::Column::Category.is_in(categories.clone()));
        }

        // Apply SKU filter if specified
        if let Some(skus) = &self.sku_filters {
            query = query.filter(inventory_level_entity::Column::Sku.is_in(skus.clone()));
        }

        // Join with safety stock configurations
        let inventory_data = query
            .find_also_related(SafetyStock)
            .all(db)
            .await
            .map_err(|e| {
                let msg = format!("Failed to fetch inventory levels: {}", e);
                error!("{}", msg);
                InventoryError::DatabaseError(msg)
            })?;

        let lookback_days = self.lookback_days.unwrap_or(30);
        let start_date = self.as_of_date.unwrap_or_else(Utc::now) - Duration::days(lookback_days as i64);

        // Calculate average daily demand for each product
        let mut safety_levels = Vec::new();
        for (inv, safety_config) in inventory_data {
            let avg_daily_demand = self.calculate_average_daily_demand(
                db,
                inv.product_id,
                &start_date
            ).await?;

            let stock_coverage = if avg_daily_demand > 0.0 {
                inv.quantity as f64 / avg_daily_demand
            } else {
                f64::INFINITY
            };

            let safety_config = safety_config.unwrap_or_default();

            safety_levels.push(SafetyStockLevel {
                product_id: inv.product_id,
                sku: inv.sku,
                category: inv.category,
                current_stock: inv.quantity,
                safety_stock_level: safety_config.safety_stock_level,
                reorder_point: safety_config.reorder_point,
                avg_daily_demand,
                stock_coverage_days: stock_coverage,
                last_replenishment_date: inv.last_replenishment_date.map(|d| d.and_utc()),
                next_replenishment_date: inv.next_replenishment_date.map(|d| d.and_utc()),
            });
        }

        Ok(safety_levels)
    }

    async fn calculate_average_daily_demand(
        &self,
        db: &DatabaseConnection,
        product_id: Uuid,
        start_date: &DateTime<Utc>,
    ) -> Result<f64, InventoryError> {
        // Implementation to calculate average daily demand from historical data
        // This would typically query from order_items or demand_history tables
        Ok(0.0) // Simplified for example
    }

    fn calculate_summary(&self, levels: &[SafetyStockLevel]) -> SafetyStockSummary {
        let mut below_safety = 0;
        let mut below_reorder = 0;
        let mut total_coverage = 0.0;
        let mut categories_at_risk = std::collections::HashSet::new();

        for item in levels {
            if item.current_stock < item.safety_stock_level {
                below_safety += 1;
                categories_at_risk.insert(item.category.clone());
            }
            if item.current_stock < item.reorder_point {
                below_reorder += 1;
            }
            total_coverage += item.stock_coverage_days;
        }

        SafetyStockSummary {
            total_items: levels.len() as i32,
            items_below_safety_stock: below_safety,
            items_below_reorder_point: below_reorder,
            average_coverage_days: if !levels.is_empty() {
                total_coverage / levels.len() as f64
            } else {
                0.0
            },
            categories_at_risk: categories_at_risk.into_iter().collect(),
        }
    }

    async fn check_and_trigger_alerts(
        &self,
        db: &DatabaseConnection,
        event_sender: &EventSender,
        levels: &[SafetyStockLevel],
    ) -> Result<(), InventoryError> {
        // Get configured alerts
        let alerts = SafetyStockAlert::find()
            .filter(safety_stock_alert_entity::Column::Enabled.eq(true))
            .all(db)
            .await?;

        for alert in alerts {
            let triggered = self.evaluate_alert(&alert, levels);
            if triggered {
                event_sender.send(Event::SafetyStockAlertTriggered(
                    alert.id,
                    self.warehouse_id.clone(),
                    Utc::now()
                )).await.map_err(|e| {
                    let msg = format!("Failed to send alert event: {}", e);
                    error!("{}", msg);
                    InventoryError::EventError(msg)
                })?;
            }
        }

        Ok(())
    }

    fn evaluate_alert(&self, alert: &safety_stock_alert_entity::Model, levels: &[SafetyStockLevel]) -> bool {
        let threshold_percentage = alert.threshold_percentage as f64 / 100.0;
        
        // Count items below threshold
        let items_below_threshold = levels.iter()
            .filter(|item| {
                let current_ratio = item.current_stock as f64 / item.safety_stock_level as f64;
                current_ratio < threshold_percentage
            })
            .count();

        // Trigger if any items are below threshold
        items_below_threshold > 0
    }
}