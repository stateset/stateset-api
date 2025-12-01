use chrono::{DateTime, Datelike, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum RobotStatus {
    #[sea_orm(string_value = "in_production")]
    InProduction,
    #[sea_orm(string_value = "testing")]
    Testing,
    #[sea_orm(string_value = "ready")]
    Ready,
    #[sea_orm(string_value = "shipped")]
    Shipped,
    #[sea_orm(string_value = "in_service")]
    InService,
    #[sea_orm(string_value = "returned")]
    Returned,
    #[sea_orm(string_value = "decommissioned")]
    Decommissioned,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum RobotType {
    #[sea_orm(string_value = "articulated_arm")]
    ArticulatedArm,
    #[sea_orm(string_value = "cobot")]
    Cobot,
    #[sea_orm(string_value = "amr")]
    Amr,
    #[sea_orm(string_value = "specialized")]
    Specialized,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "robot_serial_numbers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub serial_number: String,
    pub product_id: Uuid,
    pub work_order_id: Option<Uuid>,
    pub robot_model: String,
    pub robot_type: RobotType,
    pub manufacturing_date: Option<DateTime<Utc>>,
    pub ship_date: Option<DateTime<Utc>>,
    pub customer_id: Option<Uuid>,
    pub order_id: Option<Uuid>,
    pub status: RobotStatus,
    pub warranty_start_date: Option<DateTime<Utc>>,
    pub warranty_end_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::robot_component_genealogy::Entity")]
    ComponentGenealogy,
}

impl Related<super::robot_component_genealogy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ComponentGenealogy.def()
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
                self.status = ActiveValue::Set(RobotStatus::InProduction);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Generate a serial number for a robot
    /// Format: {MODEL}-{YEAR}{MONTH}-{SEQUENCE}
    /// Example: IR6000-202401-00123
    pub fn generate_serial_number(model: &str, sequence: u32) -> String {
        let now = Utc::now();
        format!(
            "{}-{:04}{:02}-{:05}",
            model,
            now.date_naive().year(),
            now.date_naive().month(),
            sequence
        )
    }

    /// Check if robot is under warranty
    pub fn is_under_warranty(&self) -> bool {
        if let (Some(start), Some(end)) = (self.warranty_start_date, self.warranty_end_date) {
            let now = Utc::now();
            now >= start && now <= end
        } else {
            false
        }
    }

    /// Calculate warranty remaining days
    pub fn warranty_remaining_days(&self) -> Option<i64> {
        if let Some(end) = self.warranty_end_date {
            let now = Utc::now();
            if now <= end {
                Some((end - now).num_days())
            } else {
                Some(0)
            }
        } else {
            None
        }
    }

    /// Check if robot can be shipped
    pub fn can_ship(&self) -> bool {
        matches!(self.status, RobotStatus::Ready)
    }

    /// Mark robot as shipped
    pub fn ship(&mut self) {
        self.status = RobotStatus::Shipped;
        self.ship_date = Some(Utc::now());
    }
}
