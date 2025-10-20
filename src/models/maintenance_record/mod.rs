use crate::models::machine::MachineError;
use crate::models::machine::MaintenanceType;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::QueryOrder;
use sea_orm::{ActiveModelBehavior, ActiveValue, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Maintenance Record entity model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "maintenance_records")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,

    pub machine_id: i32,

    pub maintenance_date: NaiveDate,

    #[validate(length(
        min = 1,
        max = 500,
        message = "Description must be between 1 and 500 characters"
    ))]
    pub description: String,

    pub maintenance_type: MaintenanceType,

    #[validate(length(
        min = 1,
        max = 100,
        message = "Performed by must be between 1 and 100 characters"
    ))]
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
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::machine::Entity",
        from = "Column::MachineId",
        to = "crate::models::machine::Column::Id",
        on_delete = "Cascade"
    )]
    Machine,
}

impl Related<crate::models::machine::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Machine.def()
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

        record
            .validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
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

        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Add parts that were replaced during maintenance
    pub fn add_replaced_parts(&mut self, parts: Vec<String>) -> Result<(), ValidationError> {
        self.parts_replaced = Some(parts.join(", "));
        self.updated_at = Utc::now();

        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Add issues found during maintenance
    pub fn add_issues_found(&mut self, issues: Vec<String>) -> Result<(), ValidationError> {
        self.issues_found = Some(issues.join(", "));
        self.updated_at = Utc::now();

        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Add attachments (e.g., photos, documents)
    pub fn add_attachments(&mut self, attachment_urls: Vec<String>) -> Result<(), ValidationError> {
        let current_attachments: Vec<String> = self
            .attachment_urls
            .clone()
            .map(|urls| urls.split(',').map(|url| url.trim().to_string()).collect())
            .unwrap_or_default();

        let mut all_attachments = current_attachments;
        all_attachments.extend(attachment_urls);

        self.attachment_urls = Some(all_attachments.join(", "));
        self.updated_at = Utc::now();

        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Add preventive actions recommended after this maintenance
    pub fn add_preventive_actions(&mut self, actions: String) -> Result<(), ValidationError> {
        self.preventive_actions = Some(actions);
        self.updated_at = Utc::now();

        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;
        Ok(())
    }

    /// Save the maintenance record to database
    pub async fn save(&self, db: &DatabaseConnection) -> Result<Model, MachineError> {
        // Validate before saving
        self.validate()
            .map_err(|_| ValidationError::new("Validation failed"))?;

        let model: ActiveModel = self.clone().into();
        let result = match self.id {
            0 => model.insert(db).await?,
            _ => model.update(db).await?,
        };

        // Update the machine's maintenance information
        if self.id == 0 {
            // Only for new records
            if let Some(machine) = crate::models::machine::Entity::find_by_id(self.machine_id)
                .one(db)
                .await?
            {
                let mut machine_model = machine;

                // Update last maintenance date
                machine_model.last_maintenance_date = Some(self.maintenance_date);

                // Update next maintenance date based on schedule
                let next_date = crate::models::machine::calculate_next_maintenance_date(
                    machine_model.installation_date,
                    machine_model.maintenance_schedule_type,
                    machine_model.custom_schedule_days,
                    Some(self.maintenance_date),
                );
                machine_model.next_maintenance_date = Some(next_date);

                // Update maintenance statistics
                machine_model.maintenance_count += 1;

                if let Some(cost) = self.cost {
                    machine_model.total_maintenance_cost += cost;
                }

                if let Some(downtime) = self.downtime_hours {
                    machine_model.total_downtime_hours += downtime as f64;
                }

                let machine_active_model: crate::models::machine::ActiveModel =
                    machine_model.into();
                machine_active_model.update(db).await?;
            }
        }

        Ok(result)
    }

    /// Find maintenance records by date range
    pub async fn find_by_date_range(
        db: &DatabaseConnection,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::MaintenanceDate.gte(start_date))
            .filter(Column::MaintenanceDate.lte(end_date))
            .order_by_desc(Column::MaintenanceDate)
            .all(db)
            .await
    }

    /// Find maintenance records by type
    pub async fn find_by_maintenance_type(
        db: &DatabaseConnection,
        maintenance_type: MaintenanceType,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::MaintenanceType.eq(maintenance_type))
            .order_by_desc(Column::MaintenanceDate)
            .all(db)
            .await
    }

    /// Find maintenance records that require follow-up
    pub async fn find_requiring_follow_up(db: &DatabaseConnection) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::FollowUpRequired.eq(true))
            .filter(Column::FollowUpDate.is_not_null())
            .all(db)
            .await
    }

    /// Calculate total cost of maintenance by machine ID
    pub async fn calculate_total_cost_by_machine(
        db: &DatabaseConnection,
        machine_id: i32,
    ) -> Result<f64, DbErr> {
        let records = Self::find_by_machine_id(db, machine_id).await?;

        let total = records.iter().filter_map(|record| record.cost).sum();

        Ok(total)
    }

    /// Calculate average maintenance time by machine ID
    pub async fn calculate_average_hours_by_machine(
        db: &DatabaseConnection,
        machine_id: i32,
    ) -> Result<f32, DbErr> {
        let records = Self::find_by_machine_id(db, machine_id).await?;

        let (sum, count) = records
            .iter()
            .filter_map(|record| record.hours_spent)
            .fold((0.0, 0), |(sum, count), hours| (sum + hours, count + 1));

        if count > 0 {
            Ok(sum / count as f32)
        } else {
            Ok(0.0)
        }
    }

    /// Find maintenance records by machine ID
    pub async fn find_by_machine_id(
        db: &DatabaseConnection,
        machine_id: i32,
    ) -> Result<Vec<Model>, DbErr> {
        Entity::find()
            .filter(Column::MachineId.eq(machine_id))
            .order_by_desc(Column::MaintenanceDate)
            .all(db)
            .await
    }
}
