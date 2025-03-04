use sea_orm::entity::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use rust_decimal::Decimal;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub order_number: String,
    pub customer_id: Uuid,
    pub status: String,
    pub order_date: DateTime<Utc>,
    pub total_amount: Decimal,
    pub currency: String,
    pub payment_status: String,
    pub fulfillment_status: String,
    pub notes: Option<String>,
    pub shipping_address: Option<String>,
    pub billing_address: Option<String>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::order_item::Entity")]
    OrderItem,
}

impl Related<super::order_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrderItem.def()
    }
}

impl ActiveModelBehavior for ActiveModel {
    // Add business logic here, such as validating the total amount
    fn before_save(self, _insert: bool) -> Result<Self, DbErr> {
        // Ensure order total is not negative
        if let sea_orm::ActiveValue::Set(ref total) = self.total_amount {
            if total.is_sign_negative() {
                return Err(DbErr::Custom("Order total cannot be negative".to_string()));
            }
        }

        Ok(self)
    }

    // Note: We don't need to override after_save as the default implementation
    // already does what we need (converts to Model)
}