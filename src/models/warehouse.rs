use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "warehouses")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    
    pub address: Option<String>,
    
    pub city: Option<String>,
    
    pub state: Option<String>,
    
    pub country: Option<String>,
    
    pub postal_code: Option<String>,
    
    pub created_at: DateTime<Utc>,
    
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}