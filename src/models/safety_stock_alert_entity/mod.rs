use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Safety Stock Alert Status enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum AlertStatus {
    #[sea_orm(string_value = "Open")]
    Open,

    #[sea_orm(string_value = "Acknowledged")]
    Acknowledged,

    #[sea_orm(string_value = "Resolved")]
    Resolved,

    #[sea_orm(string_value = "Ignored")]
    Ignored,
}

/// Safety Stock Alert entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "safety_stock_alerts")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub safety_stock_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    pub product_name: String,

    pub product_sku: String,

    pub current_stock_level: i32,

    pub safety_stock_level: i32,

    pub deficit_amount: i32,

    pub deficit_percentage: f64,

    pub alert_message: String,

    pub status: AlertStatus,

    pub acknowledged_by: Option<String>,

    pub resolved_by: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,
}

/// Safety Stock Alert entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::safety_stock_entity::Entity",
        from = "Column::SafetyStockId",
        to = "crate::models::safety_stock_entity::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    SafetyStock,
}

impl Related<crate::models::safety_stock_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SafetyStock.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new Safety Stock Alert.
    pub fn new(
        safety_stock_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        product_name: String,
        product_sku: String,
        current_stock_level: i32,
        safety_stock_level: i32,
    ) -> Self {
        let now = Utc::now();

        let deficit_amount = safety_stock_level - current_stock_level;
        let deficit_percentage = if safety_stock_level > 0 {
            (deficit_amount as f64 / safety_stock_level as f64) * 100.0
        } else {
            0.0
        };

        let alert_message = format!(
            "Safety stock alert: current stock ({}) is below safety level ({}), deficit: {} units ({}%)",
            current_stock_level,
            safety_stock_level,
            deficit_amount,
            deficit_percentage.round()
        );

        Self {
            id: Uuid::new_v4(),
            safety_stock_id,
            product_id,
            warehouse_id,
            product_name,
            product_sku,
            current_stock_level,
            safety_stock_level,
            deficit_amount,
            deficit_percentage,
            alert_message,
            status: AlertStatus::Open,
            acknowledged_by: None,
            resolved_by: None,
            created_at: now,
            updated_at: now,
        }
    }

    /// Acknowledges the alert.
    pub fn acknowledge(&mut self, acknowledged_by: String) {
        self.status = AlertStatus::Acknowledged;
        self.acknowledged_by = Some(acknowledged_by);
        self.updated_at = Utc::now();
    }

    /// Resolves the alert.
    pub fn resolve(&mut self, resolved_by: String) {
        self.status = AlertStatus::Resolved;
        self.resolved_by = Some(resolved_by);
        self.updated_at = Utc::now();
    }

    /// Ignores the alert.
    pub fn ignore(&mut self, ignored_by: String) {
        self.status = AlertStatus::Ignored;
        self.acknowledged_by = Some(ignored_by);
        self.updated_at = Utc::now();
    }

    /// Updates the current stock level and recalculates the deficit.
    pub fn update_stock_level(&mut self, current_stock_level: i32) {
        self.current_stock_level = current_stock_level;

        let new_deficit = self.safety_stock_level - current_stock_level;
        self.deficit_amount = new_deficit;

        self.deficit_percentage = if self.safety_stock_level > 0 {
            (new_deficit as f64 / self.safety_stock_level as f64) * 100.0
        } else {
            0.0
        };

        self.alert_message = format!(
            "Safety stock alert: current stock ({}) is below safety level ({}), deficit: {} units ({}%)",
            current_stock_level,
            self.safety_stock_level,
            new_deficit,
            self.deficit_percentage.round()
        );

        // Automatically resolve if current stock is above safety stock
        if current_stock_level >= self.safety_stock_level {
            self.status = AlertStatus::Resolved;
            self.resolved_by = Some("System".to_string());
        }

        self.updated_at = Utc::now();
    }
}
