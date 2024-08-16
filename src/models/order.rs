use sea_orm::entity::prelude::*;
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use validator::{Validate};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    
    #[validate(range(min = 1, message = "Customer ID must be positive"))]
    pub customer_id: i32,
    
    pub status: OrderStatus,
    
    #[validate(range(min = 0.0, message = "Total amount must be non-negative"))]
    pub total_amount: f64,
    
    #[validate(length(min = 1, max = 255, message = "Shipping address must be between 1 and 255 characters"))]
    pub shipping_address: String,
    
    #[validate(length(min = 1, max = 255, message = "Billing address must be between 1 and 255 characters"))]
    pub billing_address: String,
    
    #[validate(custom = "validate_payment_method")]
    pub payment_method: PaymentMethod,
    
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum OrderStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    
    #[sea_orm(string_value = "Processing")]
    Processing,
    
    #[sea_orm(string_value = "Shipped")]
    Shipped,
    
    #[sea_orm(string_value = "Delivered")]
    Delivered,
    
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub enum PaymentMethod {
    CreditCard,
    Paypal,
    BankTransfer,
}