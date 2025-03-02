use sea_orm::entity::prelude::*;
use sea_orm::{DatabaseConnection, Set};
use serde::{Deserialize, Serialize};
use validator::{Validate, ValidationError};
use chrono::{NaiveDate, DateTime, Utc, Duration};
use std::fmt;
use thiserror::Error;
use std::str::FromStr;

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
            "biannually" | "bi-annually" | "semi-annually" => Ok(MaintenanceScheduleType::BiAnnually),
            "annually" | "yearly" => Ok(MaintenanceScheduleType::Annually),
            "custom" => Ok(MaintenanceScheduleType::Custom),
            _ => Err(MachineError::Parse(format!("Unknown maintenance schedule: {}", s))),
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
    
    #[validate(length(min = 1, max = 100, message = "Name must be between 1 and 100 characters"))]
    pub name: String,
    
    #[validate(length(min = 1, max = 100, message = "Model must be between 1 and 100 characters"))]
    pub model: String,
    
    #[validate(length(min = 1, max = 100, message = "Serial number must be between 1 and 100 characters"))]
    pub serial_number: String,
    
    #[validate(length(min = 1, max = 100, message = "Manufacturer must be between 1 and 100 characters"))]
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
    
    #[sea_orm(has_many = "super::machine_part::Entity")]
    MachineParts,
    
    #[sea_orm(has_many = "super::machine_document::Entity")]
    Documents,
}

impl Related<super::maintenance_record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MaintenanceRecords.def()
    }
}

impl Related<super::machine_part::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::MachineParts.def()
    }
}

impl Related<super::machine_document::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Documents.def()
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
            self.maintenance_count = Set(0);
            self.total_downtime_hours = Set(0.0);
            self.total_maintenance_cost = Set(0.0);
            
            // Set default values
            if self.status.is_none() {
                self.status = Set(MachineStatus::Installing);
            }
        }
        
        Ok(self)
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
        if maintenance_schedule_type == MaintenanceScheduleType::Custom && custom_schedule_days.is_none() {
            return Err(ValidationError::new("Custom schedule requires specifying days"));
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
        
        machine.validate()?;
        Ok(machine)
    }

    /// Calculate if machine is due for maintenance based on schedule and last maintenance
    pub fn is_due_for_maintenance(&self, current_date: NaiveDate) -> bool {
        if let Some(next_date) = self.next_maintenance_date {
            current_date >= next_date
        } else {
            // If no next maintenance date is set, use the original algorithm
            let days_since_installation = current_date.signed_duration_since(self.installation_date).num_days();
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
                        last_maintenance_days >= days
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
            return Err(ValidationError::new("Custom schedule requires specifying days"));
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
        current_date.signed_duration_since(self.installation_date).num_days()
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
            },
            (MachineStatus::UnderMaintenance, MachineStatus::Operational) => {
                // Record end of maintenance period
            },
            (MachineStatus::Operational, MachineStatus::Breakdown) => {
                // Record start of breakdown
            },
            (MachineStatus::Breakdown, MachineStatus::Operational) => {
                // Record end of breakdown
            },
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
    pub async fn save(
        &self,
        db: &DatabaseConnection
    ) -> Result<Model, MachineError> {
        // Validate before saving
        self.validate()?;
        
        let model: ActiveModel = self.clone().into();
        let result = match self.id {
            0 => model.insert(db).await?,
            _ => model.update(db).await?,
        };
        
        Ok(result)
    }
    
    /// Find a machine by ID
    pub async fn find_by_id(
        db: &DatabaseConnection,
        id: i32
    ) -> Result<Option<Model>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }
    
    /// Find machines by status
    pub async fn find_by_status(
        db: &DatabaseConnection,
        status: MachineStatus
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(status))
            .all(db)
            .await
    }
    
    /// Find machines due for maintenance
    pub async fn find_due_for_maintenance(
        db: &DatabaseConnection,
        current_date: NaiveDate
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
        manufacturer: &str
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
        limit: u64
    ) -> Result<Vec<super::maintenance_record::Model>, DbErr> {
        use super::maintenance_record::{Entity as MaintenanceRecord, Column as MaintenanceColumn};
        
        MaintenanceRecord::find()
            .filter(MaintenanceColumn::MachineId.eq(self.id))
            .order_by_desc(MaintenanceColumn::MaintenanceDate)
            .limit(limit)
            .all(db)
            .await
    }
}

/// Calculate the next maintenance date based on schedule type
fn calculate_next_maintenance_date(
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
        },
        MaintenanceScheduleType::Quarterly => {
            // Approximate a quarter as 91 days
            start_date + Duration::days(91)
        },
        MaintenanceScheduleType::BiAnnually => {
            // Approximate half a year as 182 days
            start_date + Duration::days(182)
        },
        MaintenanceScheduleType::Annually => {
            // Approximate a year as 365 days
            start_date + Duration::days(365)
        },
        MaintenanceScheduleType::Custom => {
            if let Some(days) = custom_days {
                start_date + Duration::days(days as i64)
            } else {
                // Default to monthly if custom with no days specified
                start_date + Duration::days(30)
            }
        },
    }
}

/// Maintenance Record entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "maintenance_records")]
pub struct MaintenanceRecordModel {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    pub machine_id: i32,
    
    pub maintenance_date: NaiveDate,
    
    #[validate(length(min = 1, max = 500, message = "Description must be between 1 and 500 characters"))]
    pub description: String,
    
    pub maintenance_type: MaintenanceType,
    
    #[validate(length(min = 1, max = 100, message = "Performed by must be between 1 and 100 characters"))]
    pub performed_by: String,
    
    pub cost: Option<f64>,
    
    pub hours_spent: Option<f32>,
    
    pub downtime_hours: Option<f32>,
    
    pub parts_replaced: Option<String>,
    
    pub issues_found: Option<String>,
    
    pub follow_up_required: bool,
    
    pub follow_up_date: Option<NaiveDate>,
    
    pub follow_up_notes: Option<String>,
    
    pub preventive_actions: Option<String>,
    
    pub created_at: DateTime<Utc>,
    
    pub updated_at: DateTime<Utc>,
    
    pub created_by: Option<String>,
    
    pub updated_by: Option<String>,
    
    pub attachment_urls: Option<String>,
}

/// Database relations for maintenance records
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum MaintenanceRecordRelation {
    #[sea_orm(
        belongs_to = "super::machine::Entity",
        from = "Column::MachineId",
        to = "super::machine::Column::Id",
        on_delete = "Cascade"
    )]
    Machine,
}

impl Related<super::machine::Entity> for MaintenanceRecordEntity {
    fn to() -> RelationDef {
        MaintenanceRecordRelation::Machine.def()
    }
}

/// Active model behavior for maintenance records
impl ActiveModelBehavior for MaintenanceRecordActiveModel {
    /// Hook that is triggered before insert/update
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        let now = Utc::now();
        self.updated_at = Set(now);
        
        if insert {
            self.created_at = Set(now);
            
            // Set default values
            if self.follow_up_required.is_none() {
                self.follow_up_required = Set(false);
            }
        }
        
        Ok(self)
    }
}

impl MaintenanceRecordModel {
    /// Create a new maintenance record
    pub fn new(
        machine_id: i32,
        maintenance_date: NaiveDate,
        description: String,
        maintenance_type: MaintenanceType,
        performed_by: String,
        cost: Option<f64>,
        hours_spent: Option<f32>,
        downtime_hours: Option<f32>,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let record = Self {
            id: 0, // Will be set by database
            machine_id,
            maintenance_date,
            description,
            maintenance_type,
            performed_by,
            cost,
            hours_spent,
            downtime_hours,
            parts_replaced: None,
            issues_found: None,
            follow_up_required: false,
            follow_up_date: None,
            follow_up_notes: None,
            preventive_actions: None,
            created_at: now,
            updated_at: now,
            created_by: None,
            updated_by: None,
            attachment_urls: None,
        };
        
        record.validate()?;
        Ok(record)
    }
    
    /// Flag this maintenance record as requiring follow-up
    pub fn require_follow_up(
        &mut self,
        follow_up_date: NaiveDate,
        notes: Option<String>,
    ) -> Result<(), ValidationError> {
        self.follow_up_required = true;
        self.follow_up_date = Some(follow_up_date);
        self.follow_up_notes = notes;
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }
    
    /// Add parts that were replaced during maintenance
    pub fn add_replaced_parts(
        &mut self,
        parts: Vec<String>,
    ) -> Result<(), ValidationError> {
        self.parts_replaced = Some(parts.join(", "));
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }
    
    /// Add issues found during maintenance
    pub fn add_issues_found(
        &mut self,
        issues: Vec<String>,
    ) -> Result<(), ValidationError> {
        self.issues_found = Some(issues.join(", "));
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }
    
    /// Add attachments (e.g., photos, documents)
    pub fn add_attachments(
        &mut self,
        attachment_urls: Vec<String>,
    ) -> Result<(), ValidationError> {
        let current_attachments: Vec<String> = self.attachment_urls
            .clone()
            .map(|urls| urls.split(',').map(|url| url.trim().to_string()).collect())
            .unwrap_or_default();
        
        let mut all_attachments = current_attachments;
        all_attachments.extend(attachment_urls);
        
        self.attachment_urls = Some(all_attachments.join(", "));
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }
    
    /// Add preventive actions recommended after this maintenance
    pub fn add_preventive_actions(
        &mut self,
        actions: String,
    ) -> Result<(), ValidationError> {
        self.preventive_actions = Some(actions);
        self.updated_at = Utc::now();
        
        self.validate()?;
        Ok(())
    }
    
    /// Save the maintenance record to database
    pub async fn save(
        &self,
        db: &DatabaseConnection
    ) -> Result<MaintenanceRecordModel, MachineError> {
        // Validate before saving
        self.validate()?;
        
        let model: MaintenanceRecordActiveModel = self.clone().into();
        let result = match self.id {
            0 => model.insert(db).await?,
            _ => model.update(db).await?,
        };
        
        // Update the machine's maintenance information
        if self.id == 0 {  // Only for new records
            if let Some(machine) = super::machine::Entity::find_by_id(self.machine_id).one(db).await? {
                let mut machine_model = machine;
                
                // Update last maintenance date
                machine_model.last_maintenance_date = Some(self.maintenance_date);
                
                // Update next maintenance date
                machine_model.next_maintenance_date = Some(calculate_next_maintenance_date(
                    machine_model.installation_date,
                    machine_model.maintenance_schedule_type,
                    machine_model.custom_schedule_days,
                    Some(self.maintenance_date),
                ));
                
                // Update maintenance statistics
                machine_model.maintenance_count += 1;
                
                if let Some(cost) = self.cost {
                    machine_model.total_maintenance_cost += cost;
                }
                
                if let Some(downtime) = self.downtime_hours {
                    machine_model.total_downtime_hours += downtime as f64;
                }
                
                let machine_active_model: super::machine::ActiveModel = machine_model.into();
                machine_active_model.update(db).await?;
            }
        }
        
        Ok(result)
    }
    
    /// Find maintenance records by date range
    pub async fn find_by_date_range(
        db: &DatabaseConnection,
        start_date: NaiveDate,
        end_date: NaiveDate
    ) -> Result<Vec<MaintenanceRecordModel>, DbErr> {
        MaintenanceRecordEntity::find()
            .filter(Column::MaintenanceDate.gte(start_date))
            .filter(Column::MaintenanceDate.lte(end_date))
            .order_by_desc(Column::MaintenanceDate)
            .all(db)
            .await
    }
    
    /// Find maintenance records by type
    pub async fn find_by_maintenance_type(
        db: &DatabaseConnection,
        maintenance_type: MaintenanceType
    ) -> Result<Vec<MaintenanceRecordModel>, DbErr> {
        MaintenanceRecordEntity::find()
            .filter(Column::MaintenanceType.eq(maintenance_type))
            .order_by_desc(Column::MaintenanceDate)
            .all(db)
            .await
    }
    
    /// Find maintenance records that require follow-up
    pub async fn find_requiring_follow_up(
        db: &DatabaseConnection
    ) -> Result<Vec<MaintenanceRecordModel>, DbErr> {
        MaintenanceRecordEntity::find()
            .filter(Column::FollowUpRequired.eq(true))
            .filter(Column::FollowUpDate.is_not_null())
            .all(db)
            .await
    }
    
    /// Calculate total cost of maintenance by machine ID
    pub async fn calculate_total_cost_by_machine(
        db: &DatabaseConnection,
        machine_id: i32
    ) -> Result<f64, DbErr> {
        let records = Self::find_by_machine_id(db, machine_id).await?;
        
        let total = records.iter()
            .filter_map(|record| record.cost)
            .sum();
            
        Ok(total)
    }
    
    /// Calculate average maintenance time by machine ID
    pub async fn calculate_average_hours_by_machine(
        db: &DatabaseConnection,
        machine_id: i32
    ) -> Result<f32, DbErr> {
        let records = Self::find_by_machine_id(db, machine_id).await?;
        
        let (sum, count) = records.iter()
            .filter_map(|record| record.hours_spent)
            .fold((0.0, 0), |(sum, count), hours| (sum + hours, count + 1));
            
        if count > 0 {
            Ok(sum / count as f32)
        } else {
            Ok(0.0)
        }
    }
} by machine ID
    pub async fn find_by_machine_id(
        db: &DatabaseConnection,
        machine_id: i32
    ) -> Result<Vec<MaintenanceRecordModel>, DbErr> {
        MaintenanceRecordEntity::find()
            .filter(Column::MachineId.eq(machine_id))
            .order_by_desc(Column::MaintenanceDate)
            .all(db)
            .await
    }