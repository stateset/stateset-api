use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(20))")]
pub enum SupplierRating {
    #[sea_orm(string_value = "excellent")]
    Excellent,
    #[sea_orm(string_value = "good")]
    Good,
    #[sea_orm(string_value = "acceptable")]
    Acceptable,
    #[sea_orm(string_value = "needs_improvement")]
    NeedsImprovement,
    #[sea_orm(string_value = "unacceptable")]
    Unacceptable,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "supplier_performance")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub supplier_id: Uuid,
    pub evaluation_period_start: NaiveDate,
    pub evaluation_period_end: NaiveDate,
    pub on_time_delivery_rate: Option<Decimal>,
    pub quality_acceptance_rate: Option<Decimal>,
    pub defect_rate: Option<Decimal>,
    pub responsiveness_score: Option<i32>,
    pub cost_competitiveness_score: Option<i32>,
    pub overall_score: Option<Decimal>,
    pub rating: Option<SupplierRating>,
    pub evaluated_by: Option<Uuid>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

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
    /// Calculate overall rating based on scores
    pub fn calculate_rating(&self) -> SupplierRating {
        let score = self.overall_score.unwrap_or(Decimal::ZERO);

        if score >= Decimal::from(90) {
            SupplierRating::Excellent
        } else if score >= Decimal::from(75) {
            SupplierRating::Good
        } else if score >= Decimal::from(60) {
            SupplierRating::Acceptable
        } else if score >= Decimal::from(40) {
            SupplierRating::NeedsImprovement
        } else {
            SupplierRating::Unacceptable
        }
    }

    /// Check if supplier meets minimum standards
    pub fn meets_standards(&self) -> bool {
        !matches!(
            self.rating,
            Some(SupplierRating::NeedsImprovement) | Some(SupplierRating::Unacceptable)
        )
    }
}
