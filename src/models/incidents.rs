use chrono::NaiveDate;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "incidents")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Uuid")]
    pub work_order_id: Uuid,
    pub incident_type: String,
    pub description: String,
    pub reported_by: String,
    pub incident_date: NaiveDate,
    pub status: String,
    pub corrective_actions: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::work_order::Entity",
        from = "Column::WorkOrderId",
        to = "super::work_order::Column::Id"
    )]
    WorkOrder,
}

impl Related<super::work_order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkOrder.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        work_order_id: Uuid,
        incident_type: String,
        description: String,
        reported_by: String,
        incident_date: NaiveDate,
    ) -> Self {
        Self {
            id: 0, // This will be set by the database
            work_order_id,
            incident_type,
            description,
            reported_by,
            incident_date,
            status: "Open".to_string(),
            corrective_actions: None,
        }
    }

    pub fn update_status(&mut self, new_status: String) {
        self.status = new_status;
    }

    pub fn add_corrective_action(&mut self, action: String) {
        match &mut self.corrective_actions {
            Some(actions) => {
                actions.push_str("\n");
                actions.push_str(&action);
            }
            None => self.corrective_actions = Some(action),
        }
    }

    pub fn is_open(&self) -> bool {
        self.status == "Open"
    }

    /// Attempts to close the incident.
    /// Returns `Ok(())` if successful, or `Err` with an explanation if the incident cannot be closed.
    pub fn close(&mut self) -> Result<(), &'static str> {
        if self.corrective_actions.is_some() {
            self.status = "Closed".to_string();
            Ok(())
        } else {
            Err("Cannot close incident without corrective actions")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum IncidentType {
    #[sea_orm(string_value = "Safety")]
    Safety,
    #[sea_orm(string_value = "Quality")]
    Quality,
    #[sea_orm(string_value = "Maintenance")]
    Maintenance,
    #[sea_orm(string_value = "Environmental")]
    Environmental,
    #[sea_orm(string_value = "Security")]
    Security,
    #[sea_orm(string_value = "Other")]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum IncidentStatus {
    #[sea_orm(string_value = "Open")]
    Open,
    #[sea_orm(string_value = "In Progress")]
    InProgress,
    #[sea_orm(string_value = "Closed")]
    Closed,
}
