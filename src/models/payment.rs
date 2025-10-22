use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "payments")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    pub order_id: Uuid,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub amount: Decimal,
    #[sea_orm(column_type = "Text")]
    pub currency: String,
    #[sea_orm(column_type = "Text")]
    pub payment_method: String,
    pub payment_method_id: Option<String>,
    pub status: String,
    pub description: Option<String>,
    pub transaction_id: Option<String>,
    #[sea_orm(column_type = "Json")]
    pub gateway_response: Option<serde_json::Value>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub refunded_amount: Decimal,
    pub refund_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub processed_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id",
        on_delete = "Cascade"
    )]
    Order,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {}
