use serde::{Serialize, Deserialize};
use validator::{Validate, ValidationError};
use diesel::prelude::*;
use diesel::sql_types::Text;
use chrono::{NaiveDateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, Queryable, Insertable, AsChangeset, Validate)]
#[table_name = "orders"]
pub struct Order {
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
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[sql_type = "Text"]
pub enum OrderStatus {
    Pending,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Associations, Queryable, Insertable, Validate)]
#[belongs_to(Order)]
#[table_name = "order_items"]
pub struct OrderItem {
    pub id: i32,
    #[validate(range(min = 1, message = "Order ID must be positive"))]
    pub order_id: i32,
    #[validate(range(min = 1, message = "Product ID must be positive"))]
    pub product_id: i32,
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    #[validate(range(min = 0.0, message = "Unit price must be non-negative"))]
    pub unit_price: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentMethod {
    CreditCard,
    PayPal,
    BankTransfer,
}

impl Order {
    pub fn new(
        customer_id: i32,
        total_amount: f64,
        shipping_address: String,
        billing_address: String,
        payment_method: PaymentMethod,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now().naive_utc();
        let order = Self {
            id: 0, // Assuming database will auto-increment this
            customer_id,
            status: OrderStatus::Pending,
            total_amount,
            shipping_address,
            billing_address,
            payment_method,
            created_at: now,
            updated_at: now,
        };
        order.validate()?;
        Ok(order)
    }

    pub fn update_status(&mut self, new_status: OrderStatus) -> Result<(), String> {
        if self.status.is_final() {
            return Err("Cannot update status of a finalized order".into());
        }
        self.status = new_status;
        self.updated_at = Utc::now().naive_utc();
        Ok(())
    }
}

impl OrderStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, OrderStatus::Delivered | OrderStatus::Cancelled)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "Pending",
            OrderStatus::Processing => "Processing",
            OrderStatus::Shipped => "Shipped",
            OrderStatus::Delivered => "Delivered",
            OrderStatus::Cancelled => "Cancelled",
        }
    }
}

impl OrderItem {
    pub fn new(order_id: i32, product_id: i32, quantity: i32, unit_price: f64) -> Result<Self, ValidationError> {
        let item = Self {
            id: 0, // Assuming database will auto-increment this
            order_id,
            product_id,
            quantity,
            unit_price,
        };
        item.validate()?;
        Ok(item)
    }
}

impl PaymentMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentMethod::CreditCard => "Credit Card",
            PaymentMethod::PayPal => "PayPal",
            PaymentMethod::BankTransfer => "Bank Transfer",
        }
    }
}

fn validate_payment_method(payment_method: &PaymentMethod) -> Result<(), ValidationError> {
    match payment_method {
        PaymentMethod::CreditCard | PaymentMethod::PayPal | PaymentMethod::BankTransfer => Ok(()),
        _ => Err(ValidationError::new("Unsupported payment method")),
    }
}