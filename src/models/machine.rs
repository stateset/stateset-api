use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::NaiveDate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "machines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub model: String,
    pub serial_number: String,
    pub manufacturer: String,
    pub installation_date: NaiveDate,
    pub maintenance_schedule: String,
}

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

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        name: String,
        model: String,
        serial_number: String,
        manufacturer: String,
        installation_date: NaiveDate,
        maintenance_schedule: String,
    ) -> Self {
        Self {
            id: 0, // This will be set by the database
            name,
            model,
            serial_number,
            manufacturer,
            installation_date,
            maintenance_schedule,
        }
    }

    pub fn is_due_for_maintenance(&self, current_date: NaiveDate) -> bool {
        // This is a simple implementation. You might want to make this more sophisticated
        // based on your specific maintenance schedule format.
        let days_since_installation = current_date.signed_duration_since(self.installation_date).num_days();
        match self.maintenance_schedule.as_str() {
            "weekly" => days_since_installation % 7 == 0,
            "monthly" => days_since_installation % 30 == 0,
            "quarterly" => days_since_installation % 90 == 0,
            "annually" => days_since_installation % 365 == 0,
            _ => false,
        }
    }

    pub fn update_maintenance_schedule(&mut self, new_schedule: String) {
        self.maintenance_schedule = new_schedule;
    }

    pub fn machine_age(&self, current_date: NaiveDate) -> i64 {
        current_date.signed_duration_since(self.installation_date).num_days()
    }
}

// You might want to create a separate file for this model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "maintenance_records")]
pub struct MaintenanceRecord {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub machine_id: i32,
    pub maintenance_date: NaiveDate,
    pub description: String,
    pub performed_by: String,
    pub cost: Option<f64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum MaintenanceRecordRelation {
    #[sea_orm(
        belongs_to = "super::machine::Entity",
        from = "Column::MachineId",
        to = "super::machine::Column::Id"
    )]
    Machine,
}

impl Related<super::machine::Entity> for MaintenanceRecord {
    fn to() -> RelationDef {
        MaintenanceRecordRelation::Machine.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl MaintenanceRecord {
    pub fn new(
        machine_id: i32,
        maintenance_date: NaiveDate,
        description: String,
        performed_by: String,
        cost: Option<f64>,
    ) -> Self {
        Self {
            id: 0, // This will be set by the database
            machine_id,
            maintenance_date,
            description,
            performed_by,
            cost,
        }
    }
}