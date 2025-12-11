use chrono::{DateTime, Datelike, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum IssueType {
    #[sea_orm(string_value = "dimensional")]
    Dimensional,
    #[sea_orm(string_value = "functional")]
    Functional,
    #[sea_orm(string_value = "cosmetic")]
    Cosmetic,
    #[sea_orm(string_value = "documentation")]
    Documentation,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum Severity {
    #[sea_orm(string_value = "critical")]
    Critical,
    #[sea_orm(string_value = "major")]
    Major,
    #[sea_orm(string_value = "minor")]
    Minor,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum NcrStatus {
    #[sea_orm(string_value = "open")]
    Open,
    #[sea_orm(string_value = "investigating")]
    Investigating,
    #[sea_orm(string_value = "action_required")]
    ActionRequired,
    #[sea_orm(string_value = "resolved")]
    Resolved,
    #[sea_orm(string_value = "closed")]
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum Disposition {
    #[sea_orm(string_value = "scrap")]
    Scrap,
    #[sea_orm(string_value = "rework")]
    Rework,
    #[sea_orm(string_value = "use_as_is")]
    UseAsIs,
    #[sea_orm(string_value = "return_to_supplier")]
    ReturnToSupplier,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "non_conformance_reports")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub ncr_number: String,
    pub robot_serial_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub component_serial_id: Option<Uuid>,
    pub reported_by: Uuid,
    pub reported_at: DateTime<Utc>,
    pub issue_type: IssueType,
    pub severity: Severity,
    pub description: String,
    pub root_cause: Option<String>,
    pub corrective_action: Option<String>,
    pub preventive_action: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub status: NcrStatus,
    pub resolution_date: Option<DateTime<Utc>>,
    pub disposition: Option<Disposition>,
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
    #[sea_orm(
        belongs_to = "super::component_serial_number::Entity",
        from = "Column::ComponentSerialId",
        to = "super::component_serial_number::Column::Id"
    )]
    Component,
}

impl Related<super::robot_serial_number::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Robot.def()
    }
}

impl Related<super::component_serial_number::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Component.def()
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

            if let ActiveValue::NotSet = self.reported_at {
                self.reported_at = ActiveValue::Set(now);
            }

            if let ActiveValue::NotSet = self.status {
                self.status = ActiveValue::Set(NcrStatus::Open);
            }

            if let ActiveValue::NotSet = self.created_at {
                self.created_at = ActiveValue::Set(now);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Generate NCR number
    /// Format: NCR-{YEAR}{MONTH}-{SEQUENCE}
    pub fn generate_ncr_number(sequence: u32) -> String {
        let now = Utc::now();
        format!(
            "NCR-{:04}{:02}-{:05}",
            now.date_naive().year(),
            now.date_naive().month(),
            sequence
        )
    }

    /// Check if NCR is still open
    pub fn is_open(&self) -> bool {
        !matches!(self.status, NcrStatus::Resolved | NcrStatus::Closed)
    }

    /// Check if NCR is critical
    pub fn is_critical(&self) -> bool {
        matches!(self.severity, Severity::Critical)
    }

    /// Close NCR
    pub fn close(&mut self) {
        self.status = NcrStatus::Closed;
        if self.resolution_date.is_none() {
            self.resolution_date = Some(Utc::now());
        }
    }
}
