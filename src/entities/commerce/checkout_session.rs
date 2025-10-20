use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum CheckoutStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "processing")]
    Processing,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
    #[sea_orm(string_value = "failed")]
    Failed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "checkout_sessions")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub cart_id: Uuid,

    pub status: CheckoutStatus,
    pub payment_method: Option<String>,
    pub shipping_address: Option<String>,
    pub billing_address: Option<String>,
    pub total_amount: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::cart::Entity",
        from = "Column::CartId",
        to = "super::cart::Column::Id"
    )]
    Cart,
}

impl Related<super::cart::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cart.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
