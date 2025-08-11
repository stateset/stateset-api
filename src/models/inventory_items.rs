use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use validator::{Validate, ValidationError};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "inventory_items")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Text")]
    pub id: String,
    pub sku: String,
    pub description: String,
    pub size: String,
    #[validate(range(min = 0, message = "Incoming quantity must be non-negative"))]
    pub incoming: i32,
    pub color: String,
    pub warehouse: i32,
    pub arriving: NaiveDate,
    pub purchase_order_id: String,
    #[validate(range(min = 0, message = "Available quantity must be non-negative"))]
    pub available: i32,
    pub delivery_date: NaiveDate,
    pub arrival_date: NaiveDate,
    pub upc: String,
    pub restock_date: Option<NaiveDate>,
    pub lot_number: Option<String>,
    pub expiration_date: Option<DateTime<Utc>>,
    pub unit_cost: Option<Decimal>,
    pub cogs_amount: Option<Decimal>,
    pub cogs_currency: Option<String>,
    pub cogs_exchange_rate_id: Option<String>,
    pub cogs_last_updated: Option<DateTime<Utc>>,
    pub cogs_method: Option<String>,
    pub total_value: Option<Decimal>,
    pub average_cost: Option<Decimal>,
    pub fifo_layers: Option<JsonValue>,
    pub lifo_layers: Option<JsonValue>,
    pub quality_status: Option<String>,
    pub sustainability_score: Option<Decimal>,
    pub last_stocktake_date: Option<NaiveDate>,
    #[validate(range(min = 0, message = "Stocktake quantity must be non-negative"))]
    pub stocktake_quantity: Option<i32>,
    pub stocktake_variance: Option<Decimal>,
    #[validate(range(min = 0, message = "Allocated quantity must be non-negative"))]
    pub allocated_quantity: Option<i32>,
    #[validate(range(min = 0, message = "Reserved quantity must be non-negative"))]
    pub reserved_quantity: Option<i32>,
    #[validate(range(min = 0, message = "Damaged quantity must be non-negative"))]
    pub damaged_quantity: Option<i32>,
    pub manufacturing_date: Option<NaiveDate>,
    pub supplier_id: Option<String>,
    pub cost_center: Option<String>,
    pub abc_classification: Option<String>,
    pub turnover_rate: Option<Decimal>,
    #[validate(range(min = 0, message = "Reorder point must be non-negative"))]
    pub reorder_point: Option<i32>,
    #[validate(range(min = 0, message = "Economic order quantity must be non-negative"))]
    pub economic_order_quantity: Option<i32>,
    #[validate(range(min = 0, message = "Safety stock level must be non-negative"))]
    pub safety_stock_level: Option<i32>,
    pub weight: Option<Decimal>,
    pub weight_unit: Option<String>,
    pub volume: Option<Decimal>,
    pub volume_unit: Option<String>,
    pub location_in_warehouse: Option<String>,
    pub last_movement_date: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        id: String,
        sku: String,
        description: String,
        size: String,
        incoming: i32,
        color: String,
        warehouse: i32,
        arriving: NaiveDate,
        purchase_order_id: String,
        available: i32,
        delivery_date: NaiveDate,
        arrival_date: NaiveDate,
        upc: String,
    ) -> Result<Self, ValidationError> {
        let inventory_item = Self {
            id,
            sku,
            description,
            size,
            incoming,
            color,
            warehouse,
            arriving,
            purchase_order_id,
            available,
            delivery_date,
            arrival_date,
            upc,
            restock_date: None,
            lot_number: None,
            expiration_date: None,
            unit_cost: None,
            cogs_amount: None,
            cogs_currency: None,
            cogs_exchange_rate_id: None,
            cogs_last_updated: None,
            cogs_method: None,
            total_value: None,
            average_cost: None,
            fifo_layers: None,
            lifo_layers: None,
            quality_status: None,
            sustainability_score: None,
            last_stocktake_date: None,
            stocktake_quantity: None,
            stocktake_variance: None,
            allocated_quantity: None,
            reserved_quantity: None,
            damaged_quantity: None,
            manufacturing_date: None,
            supplier_id: None,
            cost_center: None,
            abc_classification: None,
            turnover_rate: None,
            reorder_point: None,
            economic_order_quantity: None,
            safety_stock_level: None,
            weight: None,
            weight_unit: None,
            volume: None,
            volume_unit: None,
            location_in_warehouse: None,
            last_movement_date: None,
        };
        inventory_item.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(inventory_item)
    }

    pub fn update_stock(&mut self, quantity: i32) -> Result<(), String> {
        if self.available + quantity < 0 {
            return Err("Stock cannot be negative".into());
        }
        self.available += quantity;
        self.last_movement_date = Some(Utc::now());
        Ok(())
    }

    pub fn update_quality_status(&mut self, status: String) {
        self.quality_status = Some(status);
    }

    pub fn calculate_total_value(&self) -> Option<Decimal> {
        match (self.available, self.unit_cost) {
            (available, Some(cost)) => Some(Decimal::from(available) * cost),
            _ => None,
        }
    }

    pub fn is_below_reorder_point(&self) -> bool {
        match self.reorder_point {
            Some(point) => self.available <= point,
            None => false,
        }
    }
}
