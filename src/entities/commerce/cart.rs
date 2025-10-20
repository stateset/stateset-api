use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shopping cart entity
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "carts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    #[sea_orm(nullable)]
    pub session_id: Option<String>,
    #[sea_orm(nullable)]
    pub customer_id: Option<Uuid>,
    pub currency: String,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub subtotal: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub tax_total: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub shipping_total: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub discount_total: Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub total: Decimal,
    #[sea_orm(column_type = "Json", nullable)]
    pub metadata: Option<Json>,
    pub status: CartStatus,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::cart_item::Entity")]
    CartItems,
    #[sea_orm(has_many = "super::cart_coupon::Entity")]
    AppliedCoupons,
    #[sea_orm(
        belongs_to = "super::customer::Entity",
        from = "Column::CustomerId",
        to = "super::customer::Column::Id"
    )]
    Customer,
}

impl Related<super::cart_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CartItems.def()
    }
}

impl Related<super::customer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Customer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Cart status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum CartStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "converting")]
    Converting,
    #[sea_orm(string_value = "converted")]
    Converted,
    #[sea_orm(string_value = "abandoned")]
    Abandoned,
    #[sea_orm(string_value = "expired")]
    Expired,
}
