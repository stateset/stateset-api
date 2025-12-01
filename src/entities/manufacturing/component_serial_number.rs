use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ComponentStatus {
    #[sea_orm(string_value = "in_stock")]
    InStock,
    #[sea_orm(string_value = "allocated")]
    Allocated,
    #[sea_orm(string_value = "installed")]
    Installed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "returned")]
    Returned,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "component_serial_numbers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub serial_number: String,
    pub component_type: String,
    pub component_sku: String,
    pub supplier_id: Option<Uuid>,
    pub supplier_lot_number: Option<String>,
    pub manufacture_date: Option<NaiveDate>,
    pub receive_date: Option<NaiveDate>,
    pub status: ComponentStatus,
    pub location: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::robot_component_genealogy::Entity")]
    Genealogy,
}

impl Related<super::robot_component_genealogy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Genealogy.def()
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
                self.status = ActiveValue::Set(ComponentStatus::InStock);
            }
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}

impl Model {
    /// Check if component is available for installation
    pub fn is_available(&self) -> bool {
        matches!(self.status, ComponentStatus::InStock | ComponentStatus::Allocated)
    }

    /// Mark component as installed
    pub fn install(&mut self) {
        self.status = ComponentStatus::Installed;
    }

    /// Mark component as failed
    pub fn mark_failed(&mut self) {
        self.status = ComponentStatus::Failed;
    }

    /// Get component age in days since manufacture
    pub fn age_in_days(&self) -> Option<i64> {
        self.manufacture_date.map(|mfg_date| {
            let today = Utc::now().date_naive();
            (today - mfg_date).num_days()
        })
    }
}
