use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "wishlist_items")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    
    #[sea_orm(column_type = "Uuid")]
    pub wishlist_id: Uuid,
    
    #[sea_orm(column_type = "Uuid")]
    pub product_variant_id: Uuid,
    
    pub quantity: i32,
    pub added_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::wishlist::Entity",
        from = "Column::WishlistId",
        to = "super::wishlist::Column::Id"
    )]
    Wishlist,
    
    #[sea_orm(
        belongs_to = "super::product_variant::Entity",
        from = "Column::ProductVariantId",
        to = "super::product_variant::Column::Id"
    )]
    ProductVariant,
}

impl Related<super::wishlist::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Wishlist.def()
    }
}

impl Related<super::product_variant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProductVariant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}