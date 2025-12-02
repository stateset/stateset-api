use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,

    #[validate(length(
        min = 1,
        max = 50,
        message = "Order number must be between 1 and 50 characters"
    ))]
    pub order_number: String,

    pub customer_id: Uuid,
    pub status: String,
    pub order_date: DateTime<Utc>,
    pub total_amount: Decimal,
    pub currency: String,
    pub payment_status: String,
    pub fulfillment_status: String,
    pub payment_method: Option<String>,
    pub shipping_method: Option<String>,
    pub tracking_number: Option<String>,
    pub notes: Option<String>,
    pub shipping_address: Option<String>,
    pub billing_address: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub version: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::order_item::Entity")]
    OrderItem,
    #[sea_orm(has_many = "super::shipment::Entity")]
    Shipments,
}

impl Related<super::order_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrderItem.def()
    }
}

impl Related<super::shipment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Shipments.def()
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let active_model = self;
        // Add any custom business logic here
        Ok(active_model)
    }
}
