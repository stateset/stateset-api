use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{NaiveDate, Utc, DateTime};
use uuid::Uuid;
use rust_decimal::Decimal;
use std::fmt;
use thiserror::Error;

/// Cycle count status enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum CycleCountStatus {
    #[sea_orm(string_value = "Draft")]
    Draft,
    
    #[sea_orm(string_value = "Scheduled")]
    Scheduled,
    
    #[sea_orm(string_value = "InProgress")]
    InProgress,
    
    #[sea_orm(string_value = "Completed")]
    Completed,
    
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

impl fmt::Display for CycleCountStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CycleCountStatus::Draft => write!(f, "Draft"),
            CycleCountStatus::Scheduled => write!(f, "Scheduled"),
            CycleCountStatus::InProgress => write!(f, "In Progress"),
            CycleCountStatus::Completed => write!(f, "Completed"),
            CycleCountStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

/// Cycle count type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum CycleCountType {
    #[sea_orm(string_value = "ABC")]
    Abc,
    
    #[sea_orm(string_value = "Random")]
    Random,
    
    #[sea_orm(string_value = "HighValue")]
    HighValue,
    
    #[sea_orm(string_value = "FullInventory")]
    FullInventory,
    
    #[sea_orm(string_value = "LocationBased")]
    LocationBased,
}

impl fmt::Display for CycleCountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CycleCountType::Abc => write!(f, "ABC Analysis"),
            CycleCountType::Random => write!(f, "Random Selection"),
            CycleCountType::HighValue => write!(f, "High Value Items"),
            CycleCountType::FullInventory => write!(f, "Full Inventory"),
            CycleCountType::LocationBased => write!(f, "Location Based"),
        }
    }
}

/// Cycle count method enumeration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum CycleCountMethod {
    #[sea_orm(string_value = "Blind")]
    Blind,
    
    #[sea_orm(string_value = "NonBlind")]
    NonBlind,
    
    #[sea_orm(string_value = "TwoCount")]
    TwoCount,
}

impl fmt::Display for CycleCountMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CycleCountMethod::Blind => write!(f, "Blind Count"),
            CycleCountMethod::NonBlind => write!(f, "Non-Blind Count"),
            CycleCountMethod::TwoCount => write!(f, "Two-Person Count"),
        }
    }
}

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

impl fmt::Display for LineItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

/// Cycle Count entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "cycle_counts")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    
    #[sea_orm(unique)]
    pub number: Option<i32>,
    
    #[validate(length(min = 1, max = 100, message = "Site must be between 1-100 characters"))]
    pub site: String,
    
    #[sea_orm(column_name = "type")]
    pub cycle_type: CycleCountType,
    
    pub method: CycleCountMethod,
    
    pub status: CycleCountStatus,
    
    pub scheduled_start_date: Option<NaiveDate>,
    
    pub scheduled_end_date: Option<NaiveDate>,
    
    pub completed_date: Option<NaiveDate>,
    
    #[validate(length(max = 100, message = "Assigned user cannot exceed 100 characters"))]
    pub assigned_user: Option<String>,
    
    pub created_at: DateTime<Utc>,
    
    pub updated_at: DateTime<Utc>,
    
    #[validate(length(max = 500, message = "Notes cannot exceed 500 characters"))]
    pub notes: Option<String>,
    
    pub created_by: Option<String>,
}

/// Database relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::cycle_count_line_item::Entity")]
    LineItems,
}

impl Related<super::cycle_count_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::LineItems.def()
    }
}

/// Active model behavior for database hooks
impl ActiveModelBehavior for ActiveModel {
    /// Hook that is triggered before insert/update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        let now = Utc::now();
        self.updated_at = Set(now);
        
        if insert {
            self.created_at = Set(now);
        }
        
        Ok(self)
    }
}

/// Cycle Count Line Item entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "cycle_count_line_items")]
pub struct LineItemModel {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    
    pub cycle_count_number: Option<i32>,
    
    pub status: LineItemStatus,
    
    #[validate(length(min = 1, max = 100, message = "Part number must be between 1-100 characters"))]
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
pub enum LineItemRelation {
    #[sea_orm(
        belongs_to = "super::cycle_count::Entity",
        from = "Column::CycleCountId",
        to = "super::cycle_count::Column::Id",
        on_delete = "Cascade"
    )]
    CycleCount,
}

impl Related<super::cycle_count::Entity> for LineItemEntity {
    fn to() -> RelationDef {
        LineItemRelation::CycleCount.def()
    }
}

/// Active model behavior for line items
impl ActiveModelBehavior for LineItemActiveModel {
    /// Hook that is triggered before insert/update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        let now = Utc::now();
        self.updated_at = Set(now);
        
        if insert {
            self.created_at = Set(now);
        }
        
        Ok(self)
    }
}

impl Model {
    /// Create a new cycle count
    pub fn new(
        site: String, 
        cycle_type: CycleCountType, 
        method: CycleCountMethod
    ) -> Result<Self, validator::ValidationError> {
        let now = Utc::now();
        let cycle_count = Self {
            id: Uuid::new_v4(),
            number: None,
            site,
            cycle_type,
            method,
            status: CycleCountStatus::Draft,
            scheduled_start_date: None,
            scheduled_end_date: None,
            completed_date: None,
            assigned_user: None,
            created_at: now,
            updated_at: now,
            notes: None,
            created_by: None,
        };
        
        cycle_count.validate()?;
        Ok(cycle_count)
    }

    /// Schedule the cycle count with start and end dates
    pub fn schedule(
        &mut self, 
        start: NaiveDate, 
        end: NaiveDate
    ) -> Result<(), CycleCountError> {
        if end < start {
            return Err(CycleCountError::InvalidOperation(
                "End date cannot be before start date".to_string()
            ));
        }
        
        self.scheduled_start_date = Some(start);
        self.scheduled_end_date = Some(end);
        self.status = CycleCountStatus::Scheduled;
        self.updated_at = Utc::now();
        
        Ok(())
    }

    /// Assign a user to this cycle count
    pub fn assign_user(
        &mut self, 
        user: String
    ) -> Result<(), validator::ValidationError> {
        self.assigned_user = Some(user);
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }

    /// Set the cycle count status to in progress
    pub fn start(&mut self) -> Result<(), CycleCountError> {
        match self.status {
            CycleCountStatus::Draft | CycleCountStatus::Scheduled => {
                self.status = CycleCountStatus::InProgress;
                self.updated_at = Utc::now();
                Ok(())
            },
            _ => Err(CycleCountError::InvalidOperation(
                format!("Cannot start a cycle count with status '{}'", self.status)
            )),
        }
    }

    /// Complete the cycle count
    pub fn complete(
        &mut self, 
        completion_date: NaiveDate
    ) -> Result<(), CycleCountError> {
        match self.status {
            CycleCountStatus::InProgress => {
                self.status = CycleCountStatus::Completed;
                self.completed_date = Some(completion_date);
                self.updated_at = Utc::now();
                Ok(())
            },
            _ => Err(CycleCountError::InvalidOperation(
                format!("Cannot complete a cycle count with status '{}'", self.status)
            )),
        }
    }

    /// Cancel this cycle count
    pub fn cancel(
        &mut self, 
        reason: Option<String>
    ) -> Result<(), validator::ValidationError> {
        if self.status == CycleCountStatus::Completed {
            return Err(validator::ValidationError::new(
                "Cannot cancel a completed cycle count"
            ));
        }
        
        self.status = CycleCountStatus::Cancelled;
        self.notes = reason;
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }

    /// Add notes to this cycle count
    pub fn add_notes(
        &mut self, 
        notes: String
    ) -> Result<(), validator::ValidationError> {
        self.notes = Some(notes);
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }

    /// Create and add a new line item to this cycle count
    pub async fn add_line_item(
        &self,
        db: &DatabaseConnection,
        part: String,
        quantity_expected: i32,
        location: Option<String>,
        unit_cost: Option<Decimal>
    ) -> Result<LineItemModel, CycleCountError> {
        let line_item = LineItemModel::new(
            self.id,
            part,
            quantity_expected,
            location,
            unit_cost
        )?;
        
        let active_model: LineItemActiveModel = line_item.into();
        let result = active_model.insert(db).await?;
        
        Ok(result)
    }
    
    /// Save this cycle count to the database
    pub async fn save(
        &self,
        db: &DatabaseConnection
    ) -> Result<Model, CycleCountError> {
        // Validate before saving
        self.validate()?;
        
        let model: ActiveModel = self.clone().into();
        let result = if self.number.is_none() {
            // New record
            model.insert(db).await?
        } else {
            // Update existing
            model.update(db).await?
        };
        
        Ok(result)
    }
    
    /// Find a cycle count by ID
    pub async fn find_by_id(
        db: &DatabaseConnection,
        id: Uuid
    ) -> Result<Option<Model>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }
    
    /// Find a cycle count by number
    pub async fn find_by_number(
        db: &DatabaseConnection,
        number: i32
    ) -> Result<Option<Model>, DbErr> {
        Entity::find()
            .filter(Column::Number.eq(number))
            .one(db)
            .await
    }
    
    /// Find all line items for this cycle count
    pub async fn get_line_items(
        &self,
        db: &DatabaseConnection
    ) -> Result<Vec<LineItemModel>, DbErr> {
        LineItemEntity::find()
            .filter(LineItemColumn::CycleCountId.eq(self.id))
            .all(db)
            .await
    }
    
    /// Get summary statistics for this cycle count
    pub async fn get_summary(
        &self,
        db: &DatabaseConnection
    ) -> Result<CycleCountSummary, CycleCountError> {
        let line_items = self.get_line_items(db).await?;
        
        let total_items = line_items.len();
        let counted_items = line_items.iter()
            .filter(|item| item.status != LineItemStatus::Pending && item.status != LineItemStatus::Skipped)
            .count();
        
        let total_variance = line_items.iter()
            .filter_map(|item| item.variance_quantity)
            .sum::<i32>();
            
        let total_variance_cost = line_items.iter()
            .filter_map(|item| match (item.variance_cost, item.variance_quantity) {
                (Some(cost), _) => Some(cost),
                (None, Some(qty)) => item.unit_cost.map(|uc| uc * Decimal::from(qty)),
                _ => None
            })
            .fold(Decimal::ZERO, |acc, cost| acc + cost);
            
        let accuracy_rate = if total_items > 0 {
            let items_with_no_variance = line_items.iter()
                .filter(|item| item.variance_quantity.unwrap_or(1) == 0)
                .count();
                
            (items_with_no_variance as f64 / total_items as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(CycleCountSummary {
            total_items,
            counted_items,
            pending_items: total_items - counted_items,
            total_variance,
            total_variance_cost,
            accuracy_rate,
        })
    }
}

/// Summary statistics for a cycle count
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleCountSummary {
    pub total_items: usize,
    pub counted_items: usize,
    pub pending_items: usize,
    pub total_variance: i32,
    pub total_variance_cost: Decimal,
    pub accuracy_rate: f64,
}

impl LineItemModel {
    /// Create a new cycle count line item
    pub fn new(
        cycle_count_id: Uuid,
        part: String,
        quantity_expected: i32,
        location: Option<String>,
        unit_cost: Option<Decimal>
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
        
        item.validate()?;
        Ok(item)
    }

    /// Record the counted quantity and calculate variance
    pub fn record_count(
        &mut self,
        quantity_counted: i32,
        counted_by: Option<String>
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
        
        self.validate()?;
        Ok(())
    }

    /// Set the variance cost for this line item
    pub fn set_variance_cost(
        &mut self,
        cost: Decimal
    ) -> Result<(), validator::ValidationError> {
        self.variance_cost = Some(cost);
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }

    /// Mark this line item as verified
    pub fn verify(&mut self) -> Result<(), CycleCountError> {
        if self.status != LineItemStatus::Counted {
            return Err(CycleCountError::InvalidOperation(
                "Can only verify items that have been counted".to_string()
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
                "Can only adjust items that have been counted or verified".to_string()
            ));
        }
        
        self.status = LineItemStatus::Adjusted;
        self.updated_at = Utc::now();
        
        Ok(())
    }

    /// Skip this line item with an explanation
    pub fn skip(
        &mut self,
        explanation: String
    ) -> Result<(), validator::ValidationError> {
        self.status = LineItemStatus::Skipped;
        self.explanation = Some(explanation);
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }

    /// Add an explanation for variance
    pub fn add_explanation(
        &mut self,
        explanation: String
    ) -> Result<(), validator::ValidationError> {
        self.explanation = Some(explanation);
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }
    
    /// Save this line item to the database
    pub async fn save(
        &self,
        db: &DatabaseConnection
    ) -> Result<LineItemModel, CycleCountError> {
        // Validate before saving
        self.validate()?;
        
        let model: LineItemActiveModel = self.clone().into();
        let result = match self.cycle_count_number {
            None => model.insert(db).await?,
            Some(_) => model.update(db).await?,
        };
        
        Ok(result)
    }
}