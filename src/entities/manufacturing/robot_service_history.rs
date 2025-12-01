use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ServiceType {
    #[sea_orm(string_value = "preventive_maintenance")]
    PreventiveMaintenance,
    #[sea_orm(string_value = "repair")]
    Repair,
    #[sea_orm(string_value = "calibration")]
    Calibration,
    #[sea_orm(string_value = "software_update")]
    SoftwareUpdate,
    #[sea_orm(string_value = "inspection")]
    Inspection,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ServiceStatus {
    #[sea_orm(string_value = "scheduled")]
    Scheduled,
    #[sea_orm(string_value = "in_progress")]
    InProgress,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "robot_service_history")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub robot_serial_id: Uuid,
    pub service_ticket_number: String,
    pub service_type: ServiceType,
    pub service_date: NaiveDate,
    pub technician_id: Option<Uuid>,
    pub description: Option<String>,
    pub work_performed: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub parts_replaced: Option<JsonValue>,
    pub labor_hours: Option<Decimal>,
    pub service_cost: Option<Decimal>,
    pub next_service_due: Option<NaiveDate>,
    pub status: ServiceStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::robot_serial_number::Entity",
        from = "Column::RobotSerialId",
        to = "super::robot_serial_number::Column::Id"
    )]
    Robot,
}

impl Related<super::robot_serial_number::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Robot.def()
    }
}

#[async_trait::async_trait]
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
                self.status = ActiveValue::Set(ServiceStatus::Scheduled);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Generate service ticket number
    /// Format: SVC-{YEAR}{MONTH}-{SEQUENCE}
    pub fn generate_ticket_number(sequence: u32) -> String {
        let now = Utc::now();
        format!("SVC-{:04}{:02}-{:05}", now.date_naive().year(), now.date_naive().month(), sequence)
    }

    /// Check if service is overdue
    pub fn is_overdue(&self) -> bool {
        if matches!(self.status, ServiceStatus::Scheduled) {
            let today = Utc::now().date_naive();
            self.service_date < today
        } else {
            false
        }
    }

    /// Complete service
    pub fn complete(&mut self) {
        self.status = ServiceStatus::Completed;
    }
}
