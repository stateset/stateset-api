use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Product variant entity for handling variations
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_variants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub product_id: Uuid,
    pub sku: String,
    pub name: String,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub price: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))", nullable)]
    pub compare_at_price: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))", nullable)]
    pub cost: Option<Decimal>,
    #[sea_orm(nullable)]
    pub weight: Option<f64>,
    #[sea_orm(column_type = "Json", nullable)]
    pub dimensions: Option<Json>,
    #[sea_orm(column_type = "Json")]
    pub options: Json, // HashMap<String, String> serialized
    pub inventory_tracking: bool,
    pub position: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::super::product::Entity",
        from = "Column::ProductId",
        to = "super::super::product::Column::Id"
    )]
    Product,
    #[sea_orm(has_many = "super::variant_image::Entity")]
    VariantImages,
    // #[sea_orm(has_many = "crate::entities::inventory_transaction::Entity")]
    // InventoryTransactions, // Commented out - Related trait not implemented yet
}

impl Related<super::super::product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Product.def()
    }
}

impl Related<super::variant_image::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::VariantImages.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Dimensions structure for product variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dimensions {
    pub length: f64,
    pub width: f64,
    pub height: f64,
    pub unit: DimensionUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DimensionUnit {
    Cm,
    In,
    Mm,
    M,
    Ft,
}

/// Weight structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weight {
    pub value: f64,
    pub unit: WeightUnit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WeightUnit {
    Kg,
    Lb,
    Oz,
    G,
} 