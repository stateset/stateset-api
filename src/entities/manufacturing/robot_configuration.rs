use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ConfigurationType {
    #[sea_orm(string_value = "as_ordered")]
    AsOrdered,
    #[sea_orm(string_value = "as_built")]
    AsBuilt,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum MountingType {
    #[sea_orm(string_value = "floor")]
    Floor,
    #[sea_orm(string_value = "ceiling")]
    Ceiling,
    #[sea_orm(string_value = "wall")]
    Wall,
    #[sea_orm(string_value = "mobile")]
    Mobile,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "robot_configurations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub robot_serial_id: Uuid,
    pub configuration_type: ConfigurationType,
    pub robot_model: String,
    pub payload_kg: Option<Decimal>,
    pub reach_mm: Option<i32>,
    pub degrees_of_freedom: Option<i32>,
    pub end_effector_type: Option<String>,
    pub power_requirements: Option<String>,
    pub mounting_type: Option<MountingType>,
    #[sea_orm(column_type = "JsonBinary")]
    pub custom_options: Option<JsonValue>,
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
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Check if configuration matches customer order
    pub fn matches_order(&self, other: &Self) -> bool {
        self.robot_model == other.robot_model
            && self.payload_kg == other.payload_kg
            && self.reach_mm == other.reach_mm
            && self.degrees_of_freedom == other.degrees_of_freedom
            && self.end_effector_type == other.end_effector_type
    }
}
