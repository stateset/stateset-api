use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "parts")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub part_number: String,
    pub unit_of_measure: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::waste_and_scrap::Entity")]
    WasteAndScrap,
}

impl Related<super::waste_and_scrap::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WasteAndScrap.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
