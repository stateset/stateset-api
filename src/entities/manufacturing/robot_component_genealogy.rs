use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Traceability matrix linking components to robots
/// Records which components are installed in which robots and when
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "robot_component_genealogy")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub robot_serial_id: Uuid,
    pub component_serial_id: Uuid,
    pub position: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub installed_by: Option<Uuid>,
    pub removed_at: Option<DateTime<Utc>>,
    pub removed_by: Option<Uuid>,
    pub removal_reason: Option<String>,
    pub created_at: DateTime<Utc>,
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

            if let ActiveValue::NotSet = self.installed_at {
                self.installed_at = ActiveValue::Set(now);
            }

            if let ActiveValue::NotSet = self.created_at {
                self.created_at = ActiveValue::Set(now);
            }
        }

        Ok(self)
    }
}

impl Model {
    /// Check if component is currently installed
    pub fn is_currently_installed(&self) -> bool {
        self.removed_at.is_none()
    }

    /// Remove component from robot
    pub fn remove(&mut self, removed_by: Uuid, reason: String) {
        self.removed_at = Some(Utc::now());
        self.removed_by = Some(removed_by);
        self.removal_reason = Some(reason);
    }

    /// Get installed duration in days
    pub fn installed_duration_days(&self) -> i64 {
        let end = self.removed_at.unwrap_or_else(Utc::now);
        (end - self.installed_at).num_days()
    }
}
