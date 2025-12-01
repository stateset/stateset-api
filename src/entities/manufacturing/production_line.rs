use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(50))")]
pub enum LineType {
    #[sea_orm(string_value = "assembly")]
    Assembly,
    #[sea_orm(string_value = "subassembly")]
    Subassembly,
    #[sea_orm(string_value = "testing")]
    Testing,
    #[sea_orm(string_value = "packaging")]
    Packaging,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(50))")]
pub enum LineStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "maintenance")]
    Maintenance,
    #[sea_orm(string_value = "inactive")]
    Inactive,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "production_lines")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub line_number: String,
    pub name: String,
    pub line_type: Option<LineType>,
    pub location: Option<String>,
    pub capacity_units_per_day: Option<Decimal>,
    pub status: LineStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();

        if insert {
            if let ActiveValue::NotSet = self.id {
                self.id = ActiveValue::Set(Uuid::new_v4());
            }

            if let ActiveValue::NotSet = self.created_at {
                self.created_at = ActiveValue::Set(now);
            }

            if let ActiveValue::NotSet = self.status {
                self.status = ActiveValue::Set(LineStatus::Active);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Check if line is available for production
    pub fn is_available(&self) -> bool {
        matches!(self.status, LineStatus::Active)
    }

    /// Calculate capacity utilization
    pub fn capacity_utilization(&self, actual_units: Decimal) -> Option<Decimal> {
        self.capacity_units_per_day.map(|capacity| {
            if capacity > Decimal::ZERO {
                (actual_units / capacity) * Decimal::from(100)
            } else {
                Decimal::ZERO
            }
        })
    }
}
