use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Safety Stock entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "safety_stocks")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    pub product_name: String,

    pub product_sku: String,

    pub safety_stock_level: i32,

    pub lead_time_days: i32,

    pub average_daily_demand: f64,

    pub service_level_factor: f64,

    pub demand_variability: f64,

    pub calculation_method: String,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub created_by: Option<String>,
}

/// Safety Stock entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "crate::models::safety_stock_alert_entity::Entity")]
    Alerts,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new Safety Stock.
    pub fn new(
        product_id: Uuid,
        warehouse_id: Uuid,
        product_name: String,
        product_sku: String,
        safety_stock_level: i32,
        lead_time_days: i32,
        average_daily_demand: f64,
        service_level_factor: f64,
        demand_variability: f64,
        calculation_method: String,
        created_by: Option<String>,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            product_id,
            warehouse_id,
            product_name,
            product_sku,
            safety_stock_level,
            lead_time_days,
            average_daily_demand,
            service_level_factor,
            demand_variability,
            calculation_method,
            created_at: now,
            updated_at: now,
            created_by,
        }
    }

    /// Calculates safety stock level using standard deviation method.
    pub fn calculate_safety_stock_std_dev(&mut self) {
        // Safety Stock = Z * σ * √L
        // Where:
        // Z = service level factor
        // σ = standard deviation of demand
        // L = lead time
        let safety_stock = (self.service_level_factor
            * self.demand_variability
            * (self.lead_time_days as f64).sqrt()) as i32;

        self.safety_stock_level = safety_stock;
        self.calculation_method = "StandardDeviation".to_string();
        self.updated_at = Utc::now();
    }

    /// Calculates safety stock level using simple percentage method.
    pub fn calculate_safety_stock_percentage(&mut self, percentage: f64) {
        // Safety Stock = Average Daily Demand * Lead Time * Percentage
        let safety_stock =
            (self.average_daily_demand * (self.lead_time_days as f64) * percentage) as i32;

        self.safety_stock_level = safety_stock;
        self.calculation_method = "Percentage".to_string();
        self.updated_at = Utc::now();
    }

    /// Updates the safety stock parameters.
    pub fn update_parameters(
        &mut self,
        lead_time_days: i32,
        average_daily_demand: f64,
        service_level_factor: f64,
        demand_variability: f64,
    ) {
        self.lead_time_days = lead_time_days;
        self.average_daily_demand = average_daily_demand;
        self.service_level_factor = service_level_factor;
        self.demand_variability = demand_variability;
        self.updated_at = Utc::now();
    }

    /// Updates the safety stock level directly.
    pub fn set_safety_stock_level(&mut self, safety_stock_level: i32, calculation_method: String) {
        self.safety_stock_level = safety_stock_level;
        self.calculation_method = calculation_method;
        self.updated_at = Utc::now();
    }
}
