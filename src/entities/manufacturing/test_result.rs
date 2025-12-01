use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum TestStatus {
    #[sea_orm(string_value = "pass")]
    Pass,
    #[sea_orm(string_value = "fail")]
    Fail,
    #[sea_orm(string_value = "conditional_pass")]
    ConditionalPass,
    #[sea_orm(string_value = "retest_required")]
    RetestRequired,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "test_results")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub test_protocol_id: Uuid,
    pub robot_serial_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub tested_by: Uuid,
    pub test_date: DateTime<Utc>,
    pub status: TestStatus,
    #[sea_orm(column_type = "JsonBinary")]
    pub measurements: Option<JsonValue>,
    #[sea_orm(column_type = "JsonBinary")]
    pub test_equipment_ids: Option<Vec<Uuid>>,
    #[sea_orm(column_type = "JsonBinary")]
    pub calibration_due_dates: Option<Vec<NaiveDate>>,
    pub notes: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub attachments: Option<JsonValue>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::test_protocol::Entity",
        from = "Column::TestProtocolId",
        to = "super::test_protocol::Column::Id"
    )]
    TestProtocol,
    #[sea_orm(
        belongs_to = "super::robot_serial_number::Entity",
        from = "Column::RobotSerialId",
        to = "super::robot_serial_number::Column::Id"
    )]
    Robot,
}

impl Related<super::test_protocol::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TestProtocol.def()
    }
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

            if let ActiveValue::NotSet = self.test_date {
                self.test_date = ActiveValue::Set(now);
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
    /// Check if test passed
    pub fn passed(&self) -> bool {
        matches!(self.status, TestStatus::Pass | TestStatus::ConditionalPass)
    }

    /// Check if retest is needed
    pub fn needs_retest(&self) -> bool {
        matches!(self.status, TestStatus::Fail | TestStatus::RetestRequired)
    }
}
