use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "production_metrics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub production_date: NaiveDate,
    pub shift: Option<String>,
    pub production_line_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub robot_model: Option<String>,
    pub planned_quantity: Option<i32>,
    pub actual_quantity: Option<i32>,
    pub quantity_passed: Option<i32>,
    pub quantity_failed: Option<i32>,
    pub quantity_rework: Option<i32>,
    pub first_pass_yield: Option<Decimal>,
    pub scrap_rate: Option<Decimal>,
    pub planned_hours: Option<Decimal>,
    pub actual_hours: Option<Decimal>,
    pub downtime_hours: Option<Decimal>,
    pub downtime_reason: Option<String>,
    pub oee: Option<Decimal>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::production_line::Entity",
        from = "Column::ProductionLineId",
        to = "super::production_line::Column::Id"
    )]
    ProductionLine,
}

impl Related<super::production_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProductionLine.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            if let ActiveValue::NotSet = self.id {
                self.id = ActiveValue::Set(Uuid::new_v4());
            }

            if let ActiveValue::NotSet = self.created_at {
                self.created_at = ActiveValue::Set(Utc::now());
            }
        }

        Ok(self)
    }
}

impl Model {
    /// Calculate first pass yield
    pub fn calculate_first_pass_yield(&self) -> Option<Decimal> {
        if let (Some(passed), Some(actual)) = (self.quantity_passed, self.actual_quantity) {
            if actual > 0 {
                Some((Decimal::from(passed) / Decimal::from(actual)) * Decimal::from(100))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Calculate scrap rate
    pub fn calculate_scrap_rate(&self) -> Option<Decimal> {
        if let (Some(failed), Some(actual)) = (self.quantity_failed, self.actual_quantity) {
            if actual > 0 {
                Some((Decimal::from(failed) / Decimal::from(actual)) * Decimal::from(100))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Calculate OEE (Overall Equipment Effectiveness)
    /// OEE = Availability × Performance × Quality
    pub fn calculate_oee(&self) -> Option<Decimal> {
        if let (Some(planned_hrs), Some(actual_hrs), Some(downtime)) =
            (self.planned_hours, self.actual_hours, self.downtime_hours)
        {
            if planned_hrs > Decimal::ZERO {
                // Availability = (Planned Time - Downtime) / Planned Time
                let availability = (planned_hrs - downtime) / planned_hrs;

                // Performance = Actual Units / Planned Units
                let performance = if let (Some(actual_qty), Some(planned_qty)) =
                    (self.actual_quantity, self.planned_quantity)
                {
                    if planned_qty > 0 {
                        Decimal::from(actual_qty) / Decimal::from(planned_qty)
                    } else {
                        Decimal::ONE
                    }
                } else {
                    Decimal::ONE
                };

                // Quality = Good Units / Total Units
                let quality = if let (Some(passed), Some(actual)) =
                    (self.quantity_passed, self.actual_quantity)
                {
                    if actual > 0 {
                        Decimal::from(passed) / Decimal::from(actual)
                    } else {
                        Decimal::ONE
                    }
                } else {
                    Decimal::ONE
                };

                Some(availability * performance * quality * Decimal::from(100))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Check if metrics meet target (80% OEE is industry standard)
    pub fn meets_target_oee(&self) -> bool {
        if let Some(oee) = self.oee {
            oee >= Decimal::from(80)
        } else {
            false
        }
    }
}
