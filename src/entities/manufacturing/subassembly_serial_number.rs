use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(Some(50))")]
pub enum SubassemblyStatus {
    #[sea_orm(string_value = "in_production")]
    InProduction,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "in_stock")]
    InStock,
    #[sea_orm(string_value = "installed")]
    Installed,
    #[sea_orm(string_value = "scrapped")]
    Scrapped,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "subassembly_serial_numbers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub serial_number: String,
    pub subassembly_type: String,
    pub bom_id: Option<Uuid>,
    pub work_order_id: Option<Uuid>,
    pub product_id: Option<Uuid>,
    pub parent_robot_serial_id: Option<Uuid>,
    pub status: SubassemblyStatus,
    pub completed_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::robot_serial_number::Entity",
        from = "Column::ParentRobotSerialId",
        to = "super::robot_serial_number::Column::Id"
    )]
    ParentRobot,
    #[sea_orm(
        belongs_to = "super::bom::Entity",
        from = "Column::BomId",
        to = "super::bom::Column::Id"
    )]
    Bom,
}

impl Related<super::robot_serial_number::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ParentRobot.def()
    }
}

impl Related<super::bom::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bom.def()
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
                self.status = ActiveValue::Set(SubassemblyStatus::InProduction);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Generate subassembly serial number
    /// Format: {TYPE}-{YEAR}{MONTH}-{SEQUENCE}
    /// Example: ARM-202412-00123
    pub fn generate_serial_number(subassembly_type: &str, sequence: u32) -> String {
        let now = Utc::now();
        let type_code = subassembly_type.to_uppercase().chars().take(4).collect::<String>();
        format!("{}-{:04}{:02}-{:05}", type_code, now.year(), now.month(), sequence)
    }

    /// Check if subassembly is available for installation
    pub fn is_available(&self) -> bool {
        matches!(
            self.status,
            SubassemblyStatus::Completed | SubassemblyStatus::InStock
        )
    }

    /// Install subassembly into robot
    pub fn install(&mut self, robot_serial_id: Uuid) {
        self.parent_robot_serial_id = Some(robot_serial_id);
        self.status = SubassemblyStatus::Installed;
    }

    /// Complete subassembly
    pub fn complete(&mut self) {
        self.status = SubassemblyStatus::Completed;
        self.completed_date = Some(Utc::now());
    }
}
