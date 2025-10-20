use chrono::NaiveDate;
use rust_decimal::Decimal;
use sea_orm::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub sku: String,
    pub warehouse: String,
    pub available: i32,
    pub allocated_quantity: Option<i32>,
    pub reserved_quantity: Option<i32>,
    pub unit_cost: Option<Decimal>,
    pub last_movement_date: Option<DateTime>,
    pub arrival_date: NaiveDate,
    pub updated_at: DateTime,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
