use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum PromotionStatus {
    #[sea_orm(string_value = "Draft")]
    Draft,
    #[sea_orm(string_value = "Active")]
    Active,
    #[sea_orm(string_value = "Paused")]
    Paused,
    #[sea_orm(string_value = "Expired")]
    Expired,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
    #[sea_orm(string_value = "Inactive")]
    Inactive,
}

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum PromotionType {
    #[sea_orm(string_value = "Percentage")]
    Percentage,
    #[sea_orm(string_value = "FixedAmount")]
    FixedAmount,
    #[sea_orm(string_value = "BuyOneGetOne")]
    BuyOneGetOne,
    #[sea_orm(string_value = "FreeShipping")]
    FreeShipping,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "promotions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub promotion_code: String,
    pub promotion_type: PromotionType,
    pub discount_value: Decimal,
    pub min_order_amount: Option<Decimal>,
    pub max_discount_amount: Option<Decimal>,
    pub usage_limit: Option<i32>,
    pub usage_count: i32,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub status: PromotionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
