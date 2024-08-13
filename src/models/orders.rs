use serde::{Serialize, Deserialize};
use validator::Validate;
use diesel::prelude::*;
use diesel::sql_types::Text;
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "orders"]
pub struct Order {
    pub id: i32,
    pub customer_id: i32,
    pub status: OrderStatus,
    #[validate(range(min = 0.0, message = "Total amount must be non-negative"))]
    pub total_amount: f64,
    #[validate(length(min = 1, message = "Shipping address cannot be empty"))]
    pub shipping_address: String,
    #[validate(length(min = 1, message = "Billing address cannot be empty"))]
    pub billing_address: String,
    #[validate]
    pub payment_method: PaymentMethod,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "Text"]
pub enum OrderStatus {
    Pending,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
}

impl OrderStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, OrderStatus::Delivered | OrderStatus::Cancelled)
    }
}

#[derive(Debug, Serialize, Deserialize, Associations, Queryable, Insertable)]
#[belongs_to(Order)]
#[table_name = "order_items"]
pub struct OrderItem {
    pub id: i32,
    pub order_id: i32,
    pub product_id: i32,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    #[validate(range(min = 0.0, message = "Unit price must be non-negative"))]
    pub unit_price: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PaymentMethod {
    CreditCard,
    PayPal,
    BankTransfer,
}

impl PaymentMethod {
    pub fn validate(&self) -> Result<(), String> {
        match self {
            PaymentMethod::CreditCard | PaymentMethod::PayPal | PaymentMethod::BankTransfer => Ok(()),
            _ => Err("Unsupported payment method".into()),
        }
    }
}
