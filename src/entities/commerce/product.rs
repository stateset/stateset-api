use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Product entity for the catalog system
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    #[sea_orm(column_type = "Text")]
    pub description: String,
    pub status: ProductStatus,
    pub product_type: ProductType,
    #[sea_orm(column_type = "Json")]
    pub attributes: Json,
    #[sea_orm(column_type = "Json")]
    pub seo: Json,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::product_variant::Entity")]
    ProductVariants,
    #[sea_orm(has_many = "super::product_category::Entity")]
    ProductCategories,
    #[sea_orm(has_many = "super::product_tag::Entity")]
    ProductTags,
    #[sea_orm(has_many = "super::product_image::Entity")]
    ProductImages,
}

impl Related<super::product_variant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProductVariants.def()
    }
}

impl Related<super::product_category::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ProductCategories.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Product status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum ProductStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "archived")]
    Archived,
}

/// Product type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum ProductType {
    #[sea_orm(string_value = "simple")]
    Simple,
    #[sea_orm(string_value = "variable")]
    Variable,
    #[sea_orm(string_value = "bundle")]
    Bundle,
    #[sea_orm(string_value = "digital")]
    Digital,
}

/// SEO metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeoMetadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Vec<String>,
    pub og_title: Option<String>,
    pub og_description: Option<String>,
    pub og_image: Option<String>,
}

/// Product attribute structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductAttribute {
    pub name: String,
    pub value: serde_json::Value,
    pub group: Option<String>,
    pub is_visible: bool,
    pub is_variation: bool,
} 