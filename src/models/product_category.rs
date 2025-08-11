use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "product_categories")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    
    pub parent_id: Option<Uuid>,
    
    pub description: Option<String>,
    
    pub created_at: DateTime<Utc>,
    
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    // TODO: Add this relation when product_entity has a category_id field
    // #[sea_orm(
    //     belongs_to = "super::product_entity::Entity",
    //     from = "Column::Id",
    //     to = "super::product_entity::Column::CategoryId"
    // )]
    // Product,
}

// TODO: Uncomment when the relation is added
// impl Related<super::product_entity::Entity> for Entity {
//     fn to() -> RelationDef {
//         Relation::Product.def()
//     }
// }

impl ActiveModelBehavior for ActiveModel {}

// Legacy Diesel-style struct for compatibility (if needed)
#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct ProductCategory {
    pub id: i32,
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub parent_id: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProductCategoryAssociation {
    pub product_id: i32,
    pub category_id: i32,
}
