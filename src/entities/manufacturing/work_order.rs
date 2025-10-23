use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, ConnectionTrait};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "manufacturing_work_orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub work_order_number: String,
    pub product_id: Uuid,
    pub bom_id: Option<Uuid>,
    pub work_center_id: Option<String>,
    pub assigned_to: Option<Uuid>,
    pub status: String,
    pub priority: String,
    pub quantity_to_build: Option<Decimal>,
    pub quantity_completed: Option<Decimal>,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub actual_start: Option<DateTime<Utc>>,
    pub actual_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bom::Entity",
        from = "Column::BomId",
        to = "super::bom::Column::Id"
    )]
    Bom,
    #[sea_orm(has_many = "super::work_order_material::Entity")]
    Materials,
    #[sea_orm(has_many = "super::work_order_task::Entity")]
    Tasks,
    #[sea_orm(has_many = "super::work_order_note::Entity")]
    Notes,
}

impl Related<super::bom::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bom.def()
    }
}

impl Related<super::work_order_material::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Materials.def()
    }
}

impl Related<super::work_order_task::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tasks.def()
    }
}

impl Related<super::work_order_note::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Notes.def()
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

        if let ActiveValue::NotSet = self.status {
            self.status = ActiveValue::Set("planned".to_string());
        }

        if let ActiveValue::NotSet = self.priority {
            self.priority = ActiveValue::Set("normal".to_string());
        }

        self.updated_at = ActiveValue::Set(now);

        Ok(self)
    }
}
