use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "warehouse_locations")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    
    pub warehouse_id: Uuid,
    
    pub aisle: Option<String>,
    
    pub shelf: Option<String>,
    
    pub bin: Option<String>,
    
    pub capacity: i32,
    pub volume: f64,
    pub pick_sequence: i32,
    pub created_at: DateTime<Utc>,
    
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "crate::models::inventory_item_entity::Entity")]
    InventoryItems,
}

impl ActiveModelBehavior for ActiveModel {}

impl Related<crate::models::inventory_item_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryItems.def()
    }
}