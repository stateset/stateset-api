use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use validator::{Validate, ValidationError};
use async_trait::async_trait;

/// Line item status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum LineItemStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,

    #[sea_orm(string_value = "Counted")]
    Counted,

    #[sea_orm(string_value = "Verified")]
    Verified,

    #[sea_orm(string_value = "Adjusted")]
    Adjusted,

    #[sea_orm(string_value = "Skipped")]
    Skipped,
}

impl std::fmt::Display for LineItemStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LineItemStatus::Pending => write!(f, "Pending"),
            LineItemStatus::Counted => write!(f, "Counted"),
            LineItemStatus::Verified => write!(f, "Verified"),
            LineItemStatus::Adjusted => write!(f, "Adjusted"),
            LineItemStatus::Skipped => write!(f, "Skipped"),
        }
    }
}

/// Custom error type for cycle count operations
#[derive(Error, Debug)]
pub enum CycleCountError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Validation error: {0}")]
    Validation(#[from] validator::ValidationError),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

/// Cycle Count Line Item entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "cycle_count_line_items")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    pub cycle_count_number: Option<i32>,

    pub status: LineItemStatus,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Part number must be between 1-100 characters"
    ))]
    pub part: String,

    pub location: Option<String>,

    pub standard_tracking: Option<String>,

    pub serialized_tracking: Option<bool>,

    pub lot_tracked: Option<bool>,

    pub quantity_expected: i32,

    pub quantity_counted: Option<i32>,

    pub variance_quantity: Option<i32>,

    pub variance_cost: Option<Decimal>,

    pub unit_cost: Option<Decimal>,

    #[validate(length(max = 500, message = "Explanation cannot exceed 500 characters"))]
    pub explanation: Option<String>,

    #[sea_orm(column_type = "Uuid")]
    pub cycle_count_id: Uuid,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub counted_by: Option<String>,

    pub count_date: Option<NaiveDate>,
}

/// Line item database relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::cyclecounts::Entity",
        from = "Column::CycleCountId",
        to = "super::cyclecounts::Column::Id",
        on_delete = "Cascade"
    )]
    CycleCount,
}

impl Related<super::cyclecounts::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CycleCount.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(
        self,
        _db: &C,
        insert: bool,
    ) -> Result<Self, DbErr> {
        let mut active_model = self;
        if insert {
            active_model.set_id_if_needed();
        }
        Ok(active_model)
    }
}

impl ActiveModel {
    fn set_id_if_needed(&mut self) {
        if self.id.is_not_set() {
            self.id = Set(Uuid::new_v4());
        }
    }
}

impl Model {
    /// Create a new cycle count line item
    pub fn new(
        cycle_count_id: Uuid,
        part: String,
        quantity_expected: i32,
        location: Option<String>,
        unit_cost: Option<Decimal>,
    ) -> Result<Self, validator::ValidationError> {
        let now = Utc::now();
        let item = Self {
            id: Uuid::new_v4(),
            cycle_count_number: None,
            status: LineItemStatus::Pending,
            part,
            location,
            standard_tracking: None,
            serialized_tracking: None,
            lot_tracked: None,
            quantity_expected,
            quantity_counted: None,
            variance_quantity: None,
            variance_cost: None,
            unit_cost,
            explanation: None,
            cycle_count_id,
            created_at: now,
            updated_at: now,
            counted_by: None,
            count_date: None,
        };

        item.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(item)
    }

    /// Record the counted quantity and calculate variance
    pub fn record_count(
        &mut self,
        quantity_counted: i32,
        counted_by: Option<String>,
    ) -> Result<(), validator::ValidationError> {
        self.quantity_counted = Some(quantity_counted);
        let variance = quantity_counted - self.quantity_expected;
        self.variance_quantity = Some(variance);

        // Calculate variance cost if unit cost is available
        if let Some(unit_cost) = self.unit_cost {
            self.variance_cost = Some(unit_cost * Decimal::from(variance));
        }

        self.status = LineItemStatus::Counted;
        self.counted_by = counted_by;
        self.count_date = Some(Utc::now().date_naive());
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Set the variance cost for this line item
    pub fn set_variance_cost(&mut self, cost: Decimal) -> Result<(), validator::ValidationError> {
        self.variance_cost = Some(cost);
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Mark this line item as verified
    pub fn verify(&mut self) -> Result<(), CycleCountError> {
        if self.status != LineItemStatus::Counted {
            return Err(CycleCountError::InvalidOperation(
                "Can only verify items that have been counted".to_string(),
            ));
        }

        self.status = LineItemStatus::Verified;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Mark this line item as adjusted in the inventory system
    pub fn adjust(&mut self) -> Result<(), CycleCountError> {
        if self.status != LineItemStatus::Counted && self.status != LineItemStatus::Verified {
            return Err(CycleCountError::InvalidOperation(
                "Can only adjust items that have been counted or verified".to_string(),
            ));
        }

        self.status = LineItemStatus::Adjusted;
        self.updated_at = Utc::now();

        Ok(())
    }

    /// Skip this line item with an explanation
    pub fn skip(&mut self, explanation: String) -> Result<(), validator::ValidationError> {
        self.status = LineItemStatus::Skipped;
        self.explanation = Some(explanation);
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Add an explanation for variance
    pub fn add_explanation(
        &mut self,
        explanation: String,
    ) -> Result<(), validator::ValidationError> {
        self.explanation = Some(explanation);
        self.updated_at = Utc::now();

        self.validate().map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Save this line item to the database
    pub async fn save(&self, db: &DatabaseConnection) -> Result<Model, CycleCountError> {
        // Validate before saving
        self.validate().map_err(|_| ValidationError::new("Validation failed"))?;

        let model: ActiveModel = self.clone().into();
        let result = match self.cycle_count_number {
            None => model.insert(db).await?,
            Some(_) => model.update(db).await?,
        };

        Ok(result)
    }
}
