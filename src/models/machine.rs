use chrono::{DateTime, Duration, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveModelBehavior, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use validator::{Validate, ValidationError};

/// Custom error type for machine operations
#[derive(Error, Debug)]
pub enum MachineError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Parse error: {0}")]
    Parse(String),
}

/// Machine status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum MachineStatus {
    #[sea_orm(string_value = "Operational")]
    Operational,

    #[sea_orm(string_value = "UnderMaintenance")]
    UnderMaintenance,

    #[sea_orm(string_value = "Breakdown")]
    Breakdown,

    #[sea_orm(string_value = "Retired")]
    Retired,

    #[sea_orm(string_value = "Installing")]
    Installing,
}

impl fmt::Display for MachineStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MachineStatus::Operational => write!(f, "Operational"),
            MachineStatus::UnderMaintenance => write!(f, "Under Maintenance"),
            MachineStatus::Breakdown => write!(f, "Breakdown"),
            MachineStatus::Retired => write!(f, "Retired"),
            MachineStatus::Installing => write!(f, "Installing"),
        }
    }
}

/// Maintenance schedule type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum MaintenanceScheduleType {
    #[sea_orm(string_value = "Daily")]
    Daily,

    #[sea_orm(string_value = "Weekly")]
    Weekly,

    #[sea_orm(string_value = "BiWeekly")]
    BiWeekly,

    #[sea_orm(string_value = "Monthly")]
    Monthly,

    #[sea_orm(string_value = "Quarterly")]
    Quarterly,

    #[sea_orm(string_value = "BiAnnually")]
    BiAnnually,

    #[sea_orm(string_value = "Annually")]
    Annually,

    #[sea_orm(string_value = "Custom")]
    Custom,
}

impl fmt::Display for MaintenanceScheduleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaintenanceScheduleType::Daily => write!(f, "Daily"),
            MaintenanceScheduleType::Weekly => write!(f, "Weekly"),
            MaintenanceScheduleType::BiWeekly => write!(f, "Bi-Weekly"),
            MaintenanceScheduleType::Monthly => write!(f, "Monthly"),
            MaintenanceScheduleType::Quarterly => write!(f, "Quarterly"),
            MaintenanceScheduleType::BiAnnually => write!(f, "Bi-Annually"),
            MaintenanceScheduleType::Annually => write!(f, "Annually"),
            MaintenanceScheduleType::Custom => write!(f, "Custom"),
        }
    }
}

impl FromStr for MaintenanceScheduleType {
    type Err = MachineError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "daily" => Ok(MaintenanceScheduleType::Daily),
            "weekly" => Ok(MaintenanceScheduleType::Weekly),
            "biweekly" | "bi-weekly" => Ok(MaintenanceScheduleType::BiWeekly),
            "monthly" => Ok(MaintenanceScheduleType::Monthly),
            "quarterly" => Ok(MaintenanceScheduleType::Quarterly),
            "biannually" | "bi-annually" | "semi-annually" => {
                Ok(MaintenanceScheduleType::BiAnnually)
            }
            "annually" | "yearly" => Ok(MaintenanceScheduleType::Annually),
            "custom" => Ok(MaintenanceScheduleType::Custom),
            _ => Err(MachineError::Parse(format!(
                "Unknown maintenance schedule: {}",
                s
            ))),
        }
    }
}

/// Maintenance record type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum MaintenanceType {
    #[sea_orm(string_value = "Preventive")]
    Preventive,

    #[sea_orm(string_value = "Corrective")]
    Corrective,

    #[sea_orm(string_value = "Predictive")]
    Predictive,

    #[sea_orm(string_value = "Inspection")]
    Inspection,

    #[sea_orm(string_value = "Upgrade")]
    Upgrade,
}

impl fmt::Display for MaintenanceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MaintenanceType::Preventive => write!(f, "Preventive"),
            MaintenanceType::Corrective => write!(f, "Corrective"),
            MaintenanceType::Predictive => write!(f, "Predictive"),
            MaintenanceType::Inspection => write!(f, "Inspection"),
            MaintenanceType::Upgrade => write!(f, "Upgrade"),
        }
    }
}

/// Machine entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "machines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Name must be between 1 and 100 characters"
    ))]
    pub name: String,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Model must be between 1 and 100 characters"
    ))]
    pub model: String,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Serial number must be between 1 and 100 characters"
    ))]
    pub serial_number: String,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Manufacturer must be between 1 and 100 characters"
    ))]
    pub manufacturer: String,

    pub installation_date: NaiveDate,

    pub maintenance_schedule_type: MaintenanceScheduleType,

    pub custom_schedule_days: Option<i32>,

    pub last_maintenance_date: Option<NaiveDate>,

    pub next_maintenance_date: Option<NaiveDate>,

    pub status: MachineStatus,

    pub location: Option<String>,

    pub department: Option<String>,

    pub purchase_cost: Option<f64>,

    pub purchase_date: Option<NaiveDate>,

    pub warranty_expiry_date: Option<NaiveDate>,

    pub expected_lifetime_years: Option<i32>,

    pub power_requirements: Option<String>,

    pub technical_specifications: Option<String>,

    pub operating_instructions: Option<String>,

    pub safety_guidelines: Option<String>,

    pub maintenance_manual_url: Option<String>,

    pub notes: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub created_by: Option<String>,

    pub updated_by: Option<String>,

    pub image_url: Option<String>,

    pub qr_code_url: Option<String>,

    pub total_downtime_hours: f64,

    pub total_maintenance_cost: f64,

    pub maintenance_count: i32,
}

/// Database relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::maintenance_record::Entity")]
    MaintenanceRecords,
}

impl Related<super::maintenance_record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MaintenanceRecords.def()
    }
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(self, _db: &C, insert: bool) -> Result<Self, DbErr> {
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
            // i32 primary key: let the database assign it
        }
    }
}

impl Model {
    /// Create a new machine
    pub fn new(
        name: String,
        model: String,
        serial_number: String,
        manufacturer: String,
        installation_date: NaiveDate,
        maintenance_schedule_type: MaintenanceScheduleType,
        custom_schedule_days: Option<i32>,
    ) -> Result<Self, ValidationError> {
        // Validate custom schedule days if maintenance type is Custom
        if maintenance_schedule_type == MaintenanceScheduleType::Custom
            && custom_schedule_days.is_none()
        {
            return Err(ValidationError::new(
                "Custom schedule requires specifying days",
            ));
        }

        let now = Utc::now();
        let machine = Self {
            id: 0, // Will be set by database
            name,
            model,
            serial_number,
            manufacturer,
            installation_date,
            maintenance_schedule_type,
            custom_schedule_days,
            last_maintenance_date: None,
            next_maintenance_date: Some(calculate_next_maintenance_date(
                installation_date,
                maintenance_schedule_type,
                custom_schedule_days,
                None,
            )),
            status: MachineStatus::Installing,
            location: None,
            department: None,
            purchase_cost: None,
            purchase_date: None,
            warranty_expiry_date: None,
            expected_lifetime_years: None,
            power_requirements: None,
            technical_specifications: None,
            operating_instructions: None,
            safety_guidelines: None,
            maintenance_manual_url: None,
            notes: None,
            created_at: now,
            updated_at: now,
            created_by: None,
            updated_by: None,
            image_url: None,
            qr_code_url: None,
            total_downtime_hours: 0.0,
            total_maintenance_cost: 0.0,
            maintenance_count: 0,
        };

        machine
            .validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(machine)
    }

    /// Calculate if machine is due for maintenance based on schedule and last maintenance
    pub fn is_due_for_maintenance(&self, current_date: NaiveDate) -> bool {
        if let Some(next_date) = self.next_maintenance_date {
            current_date >= next_date
        } else {
            // If no next maintenance date is set, use the original algorithm
            let days_since_installation = current_date
                .signed_duration_since(self.installation_date)
                .num_days();
            let last_maintenance_days = if let Some(last_date) = self.last_maintenance_date {
                current_date.signed_duration_since(last_date).num_days()
            } else {
                days_since_installation // If no maintenance has been performed yet
            };

            match self.maintenance_schedule_type {
                MaintenanceScheduleType::Daily => last_maintenance_days >= 1,
                MaintenanceScheduleType::Weekly => last_maintenance_days >= 7,
                MaintenanceScheduleType::BiWeekly => last_maintenance_days >= 14,
                MaintenanceScheduleType::Monthly => last_maintenance_days >= 30,
                MaintenanceScheduleType::Quarterly => last_maintenance_days >= 90,
                MaintenanceScheduleType::BiAnnually => last_maintenance_days >= 182,
                MaintenanceScheduleType::Annually => last_maintenance_days >= 365,
                MaintenanceScheduleType::Custom => {
                    if let Some(days) = self.custom_schedule_days {
                        last_maintenance_days >= days as i64
                    } else {
                        false
                    }
                }
            }
        }
    }

    /// Update maintenance schedule
    pub fn update_maintenance_schedule(
        &mut self,
        schedule_type: MaintenanceScheduleType,
        custom_days: Option<i32>,
    ) -> Result<(), ValidationError> {
        if schedule_type == MaintenanceScheduleType::Custom && custom_days.is_none() {
            return Err(ValidationError::new(
                "Custom schedule requires specifying days",
            ));
        }

        self.maintenance_schedule_type = schedule_type;
        self.custom_schedule_days = custom_days;

        // Recalculate next maintenance date
        self.next_maintenance_date = Some(calculate_next_maintenance_date(
            self.installation_date,
            schedule_type,
            custom_days,
            self.last_maintenance_date,
        ));

        self.updated_at = Utc::now();

        Ok(())
    }

    /// Calculate machine age in days
    pub fn machine_age(&self, current_date: NaiveDate) -> i64 {
        current_date
            .signed_duration_since(self.installation_date)
            .num_days()
    }

    /// Calculate machine age in years (with decimal precision)
    pub fn machine_age_years(&self, current_date: NaiveDate) -> f64 {
        let days = self.machine_age(current_date) as f64;
        days / 365.25
    }

    /// Check if machine is under warranty
    pub fn is_under_warranty(&self, current_date: NaiveDate) -> bool {
        if let Some(warranty_date) = self.warranty_expiry_date {
            current_date <= warranty_date
        } else {
            false
        }
    }

    /// Update machine status
    pub fn update_status(
        &mut self,
        new_status: MachineStatus,
        updater: Option<String>,
    ) -> Result<(), ValidationError> {
        // Special handling for status transitions
        match (self.status, new_status) {
            (MachineStatus::Operational, MachineStatus::UnderMaintenance) => {
                // Record start of maintenance period
                // You might want to create a maintenance record here
            }
            (MachineStatus::UnderMaintenance, MachineStatus::Operational) => {
                // Record end of maintenance period
            }
            (MachineStatus::Operational, MachineStatus::Breakdown) => {
                // Record start of breakdown
            }
            (MachineStatus::Breakdown, MachineStatus::Operational) => {
                // Record end of breakdown
            }
            _ => {}
        }

        self.status = new_status;
        self.updated_at = Utc::now();
        self.updated_by = updater;

        Ok(())
    }

    /// Record a completed maintenance
    pub fn record_maintenance(
        &mut self,
        maintenance_date: NaiveDate,
        cost: Option<f64>,
    ) -> Result<(), ValidationError> {
        self.last_maintenance_date = Some(maintenance_date);

        // Calculate the next maintenance date
        self.next_maintenance_date = Some(calculate_next_maintenance_date(
            self.installation_date,
            self.maintenance_schedule_type,
            self.custom_schedule_days,
            Some(maintenance_date),
        ));

        // Update maintenance statistics
        self.maintenance_count += 1;

        if let Some(maintenance_cost) = cost {
            self.total_maintenance_cost += maintenance_cost;
        }

        self.updated_at = Utc::now();

        Ok(())
    }

    /// Calculate total cost of ownership (TCO)
    pub fn calculate_tco(&self) -> f64 {
        let purchase_cost = self.purchase_cost.unwrap_or(0.0);
        purchase_cost + self.total_maintenance_cost
    }

    /// Calculate average maintenance cost per year
    pub fn average_yearly_maintenance_cost(&self, current_date: NaiveDate) -> f64 {
        let years = self.machine_age_years(current_date);
        if years > 0.0 {
            self.total_maintenance_cost / years
        } else {
            0.0
        }
    }

    /// Save the machine to database
    pub async fn save(&self, db: &DatabaseConnection) -> Result<Model, MachineError> {
        // Validate before saving
        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;

        let model: ActiveModel = self.clone().into();
        let result = match self.id {
            0 => model.insert(db).await?,
            _ => model.update(db).await?,
        };

        Ok(result)
    }

    /// Find a machine by ID
    pub async fn find_by_id(db: &DatabaseConnection, id: i32) -> Result<Option<Model>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    /// Find machines by status
    pub async fn find_by_status(
        db: &DatabaseConnection,
        status: MachineStatus,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(status))
            .all(db)
            .await
    }

    /// Find machines due for maintenance
    pub async fn find_due_for_maintenance(
        db: &DatabaseConnection,
        current_date: NaiveDate,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::NextMaintenanceDate.is_not_null())
            .filter(Column::NextMaintenanceDate.lte(current_date))
            .filter(Column::Status.ne(MachineStatus::Retired))
            .all(db)
            .await
    }

    /// Find machines by manufacturer
    pub async fn find_by_manufacturer(
        db: &DatabaseConnection,
        manufacturer: &str,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Manufacturer.eq(manufacturer))
            .all(db)
            .await
    }

    /// Get recent maintenance records for this machine
    pub async fn get_recent_maintenance_records(
        &self,
        db: &DatabaseConnection,
        limit: u64,
    ) -> Result<Vec<super::maintenance_record::Model>, DbErr> {
        use crate::models::maintenance_record::{
            Column as MaintenanceColumn, Entity as MaintenanceRecord,
        };

        MaintenanceRecord::find()
            .filter(MaintenanceColumn::MachineId.eq(self.id))
            .order_by_desc(MaintenanceColumn::MaintenanceDate)
            .limit(limit)
            .all(db)
            .await
    }
}

/// Calculate the next maintenance date based on schedule type
pub fn calculate_next_maintenance_date(
    installation_date: NaiveDate,
    schedule_type: MaintenanceScheduleType,
    custom_days: Option<i32>,
    last_maintenance_date: Option<NaiveDate>,
) -> NaiveDate {
    // Start from the last maintenance date if available, otherwise installation date
    let start_date = last_maintenance_date.unwrap_or(installation_date);

    // Calculate next date based on schedule type
    match schedule_type {
        MaintenanceScheduleType::Daily => start_date + Duration::days(1),
        MaintenanceScheduleType::Weekly => start_date + Duration::days(7),
        MaintenanceScheduleType::BiWeekly => start_date + Duration::days(14),
        MaintenanceScheduleType::Monthly => {
            // Approximate a month as 30 days
            start_date + Duration::days(30)
        }
        MaintenanceScheduleType::Quarterly => {
            // Approximate a quarter as 91 days
            start_date + Duration::days(91)
        }
        MaintenanceScheduleType::BiAnnually => {
            // Approximate half a year as 182 days
            start_date + Duration::days(182)
        }
        MaintenanceScheduleType::Annually => {
            // Approximate a year as 365 days
            start_date + Duration::days(365)
        }
        MaintenanceScheduleType::Custom => {
            if let Some(days) = custom_days {
                start_date + Duration::days(days as i64)
            } else {
                // Default to monthly if custom with no days specified
                start_date + Duration::days(30)
            }
        }
    }
}

// Maintenance Record model has been moved to models/maintenance_record/mod.rs
