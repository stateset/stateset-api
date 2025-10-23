use crate::models::order::OrderStatus;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::{Validate, ValidationError};

/// Order entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "orders")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[validate(length(
        min = 1,
        max = 50,
        message = "Order number must be between 1 and 50 characters"
    ))]
    pub order_number: String,

    pub status: OrderStatus,

    #[validate(email(message = "Invalid customer email format"))]
    pub customer_email: String,

    #[sea_orm(column_type = "Uuid")]
    pub customer_id: Uuid,

    pub total_amount: f64,

    pub shipping_address: String,

    pub billing_address: String,

    pub payment_method: String,

    pub shipping_method: String,

    pub tracking_number: Option<String>,

    pub notes: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub version: i32,
}

/// Order entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "crate::models::order_note_entity::Entity")]
    OrderNotes,
}

impl Related<crate::models::order_note_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrderNotes.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new order.
    pub fn new(
        order_number: String,
        customer_email: String,
        customer_id: Uuid,
        total_amount: f64,
        shipping_address: String,
        billing_address: String,
        payment_method: String,
        shipping_method: String,
        notes: Option<String>,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();

        let order = Self {
            id: Uuid::new_v4(),
            order_number,
            status: OrderStatus::Pending,
            customer_email,
            customer_id,
            total_amount,
            shipping_address,
            billing_address,
            payment_method,
            shipping_method,
            tracking_number: None,
            notes,
            created_at: now,
            updated_at: now,
            version: 1,
        };

        // Validate the new order
        order
            .validate()
            .map_err(|_e| ValidationError::new("Order validation failed"))?;

        Ok(order)
    }

    /// Updates the order status.
    pub fn update_status(&mut self, new_status: OrderStatus) -> Result<(), ValidationError> {
        self.status = new_status;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_e| ValidationError::new("Order validation failed"))?;

        Ok(())
    }

    /// Sets the tracking number for the order.
    pub fn set_tracking_number(&mut self, tracking_number: String) -> Result<(), ValidationError> {
        self.tracking_number = Some(tracking_number);
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_e| ValidationError::new("Order validation failed"))?;

        Ok(())
    }

    /// Updates the shipping address for the order.
    pub fn update_shipping_address(&mut self, new_address: String) -> Result<(), ValidationError> {
        self.shipping_address = new_address;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_e| ValidationError::new("Order validation failed"))?;

        Ok(())
    }

    /// Updates the total amount for the order.
    pub fn update_total_amount(&mut self, new_amount: f64) -> Result<(), ValidationError> {
        self.total_amount = new_amount;
        self.updated_at = Utc::now();
        self.version += 1;

        // Revalidate after update
        self.validate()
            .map_err(|_e| ValidationError::new("Order validation failed"))?;

        Ok(())
    }
}
