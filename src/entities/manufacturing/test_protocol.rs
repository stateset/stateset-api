use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(50))")]
pub enum TestType {
    #[sea_orm(string_value = "mechanical")]
    Mechanical,
    #[sea_orm(string_value = "electrical")]
    Electrical,
    #[sea_orm(string_value = "software")]
    Software,
    #[sea_orm(string_value = "integration")]
    Integration,
    #[sea_orm(string_value = "safety")]
    Safety,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(50))")]
pub enum ProtocolStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "obsolete")]
    Obsolete,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "test_protocols")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub protocol_number: String,
    pub name: String,
    pub description: Option<String>,
    pub test_type: TestType,
    #[sea_orm(column_type = "JsonBinary")]
    pub applicable_models: Option<Vec<String>>,
    #[sea_orm(column_type = "JsonBinary")]
    pub test_equipment_required: Option<Vec<String>>,
    pub estimated_duration_minutes: Option<i32>,
    #[sea_orm(column_type = "JsonBinary")]
    pub pass_criteria: Option<JsonValue>,
    #[sea_orm(column_type = "JsonBinary")]
    pub procedure_steps: Option<JsonValue>,
    pub revision: Option<String>,
    pub status: ProtocolStatus,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::test_result::Entity")]
    TestResults,
}

impl Related<super::test_result::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::TestResults.def()
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
                self.status = ActiveValue::Set(ProtocolStatus::Draft);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Check if protocol is active and can be used
    pub fn is_active(&self) -> bool {
        matches!(self.status, ProtocolStatus::Active)
    }

    /// Check if protocol applies to a specific robot model
    pub fn applies_to_model(&self, model: &str) -> bool {
        if let Some(ref models) = self.applicable_models {
            models.iter().any(|m| m == model)
        } else {
            true // If no models specified, applies to all
        }
    }
}
