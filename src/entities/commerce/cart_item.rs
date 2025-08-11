use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Cart item entity
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cart_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub cart_id: Uuid,
    pub variant_id: Uuid,
    pub quantity: i32,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub unit_price: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub line_total: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub discount_amount: Decimal,
    #[sea_orm(column_type = "Json", nullable)]
    pub metadata: Option<Json>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::cart::Entity",
        from = "Column::CartId",
        to = "super::cart::Column::Id"
    )]
    Cart,
    #[sea_orm(
        belongs_to = "super::product_variant::Entity",
        from = "Column::VariantId",
        to = "super::product_variant::Column::Id"
    )]
    ProductVariant,
}

impl Related<super::cart::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cart.def()
    }
}

impl Related<super::product_variant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProductVariant.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 