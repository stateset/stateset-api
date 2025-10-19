use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "manufacturing_work_orders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub work_order_id: i64,
    pub work_order_number: String,
    pub item_id: Option<i64>,
    pub organization_id: i64,
    pub scheduled_start_date: Option<NaiveDate>,
    pub scheduled_completion_date: Option<NaiveDate>,
    pub actual_start_date: Option<NaiveDate>,
    pub actual_completion_date: Option<NaiveDate>,
    pub status_code: Option<String>,
    pub quantity_to_build: Option<Decimal>,
    pub quantity_completed: Option<Decimal>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::ItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ItemMaster,
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
