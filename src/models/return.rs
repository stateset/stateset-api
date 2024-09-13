use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "return_entity")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub created_date: DateTime<Utc>,
    #[validate(range(min = 0, message = "Amount must be non-negative"))]
    pub amount: Decimal,
    pub action_needed: Option<String>,
    pub condition: Option<String>,
    #[validate(email(message = "Invalid email format"))]
    pub customer_email: String,
    pub customer_id: i32,
    pub description: Option<String>,
    pub entered_by: Option<String>,
    #[validate(range(min = 0, message = "Flat rate shipping must be non-negative"))]
    pub flat_rate_shipping: Decimal,
    pub order_date: DateTime<Utc>,
    pub order_id: i32,
    pub reason_category: Option<String>,
    pub reported_condition: Option<String>,
    pub requested_date: DateTime<Utc>,
    pub rma: String,
    pub serial_number: Option<String>,
    pub shipped_date: Option<DateTime<Utc>>,
    pub status: ReturnStatus,
    #[validate(range(min = 0, message = "Tax refunded must be non-negative"))]
    pub tax_refunded: Decimal,
    #[validate(range(min = 0, message = "Total refunded must be non-negative"))]
    pub total_refunded: Decimal,
    pub tracking_number: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::return_line_item::Entity")]
    ReturnLineItems,
}

impl Related<super::return_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ReturnLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum ReturnStatus {
    #[sea_orm(string_value = "Requested")]
    Requested,
    #[sea_orm(string_value = "Approved")]
    Approved,
    #[sea_orm(string_value = "Rejected")]
    Rejected,
    #[sea_orm(string_value = "Received")]
    Received,
    #[sea_orm(string_value = "Refunded")]
    Refunded,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "return_line_items")]
pub struct ReturnLineItem {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[validate(range(min = 0, message = "Amount must be non-negative"))]
    pub amount: Decimal,
    pub condition: Option<String>,
    #[validate(range(min = 0, message = "Flat rate shipping must be non-negative"))]
    pub flat_rate_shipping: Decimal,
    pub name: String,
    #[validate(range(min = 0, message = "Price must be non-negative"))]
    pub price: Decimal,
    pub return_id: i32,
    pub serial_number: Option<String>,
    pub sku: String,
    #[validate(range(min = 0, message = "Tax refunded must be non-negative"))]
    pub tax_refunded: Decimal,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum ReturnLineItemRelation {
    #[sea_orm(
        belongs_to = "super::return_entity::Entity",
        from = "Column::ReturnId",
        to = "super::return_entity::Column::Id"
    )]
    Return,
}

impl Related<super::return_entity::Entity> for ReturnLineItem {
    fn to() -> RelationDef {
        ReturnLineItemRelation::Return.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        order_id: i32,
        customer_id: i32,
        customer_email: String,
        amount: Decimal,
        rma: String,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let return_request = Self {
            id: 0, // Assuming database will auto-increment this
            created_date: now,
            amount,
            action_needed: None,
            condition: None,
            customer_email,
            customer_id,
            description: None,
            entered_by: None,
            flat_rate_shipping: Decimal::new(0, 2),
            order_date: now, // This should be set to the actual order date
            order_id,
            reason_category: None,
            reported_condition: None,
            requested_date: now,
            rma,
            serial_number: None,
            shipped_date: None,
            status: ReturnStatus::Requested,
            tax_refunded: Decimal::new(0, 2),
            total_refunded: Decimal::new(0, 2),
            tracking_number: None,
        };
        return_request.validate()?;
        Ok(return_request)
    }

    pub fn update_status(&mut self, new_status: ReturnStatus) -> Result<(), String> {
        if self.status == ReturnStatus::Refunded {
            return Err("Cannot update status of a refunded return".into());
        }
        self.status = new_status;
        Ok(())
    }

    pub fn add_line_item(&self, line_item: ReturnLineItem) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }
}

impl ReturnStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, ReturnStatus::Refunded | ReturnStatus::Rejected)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ReturnStatus::Requested => "Requested",
            ReturnStatus::Approved => "Approved",
            ReturnStatus::Rejected => "Rejected",
            ReturnStatus::Received => "Received",
            ReturnStatus::Refunded => "Refunded",
        }
    }
}

impl ReturnLineItem {
    pub fn new(
        return_id: i32,
        amount: Decimal,
        name: String,
        price: Decimal,
        sku: String,
    ) -> Result<Self, ValidationError> {
        let item = Self {
            id: 0, // Assuming database will auto-increment this
            amount,
            condition: None,
            flat_rate_shipping: Decimal::new(0, 2),
            name,
            price,
            return_id,
            serial_number: None,
            sku,
            tax_refunded: Decimal::new(0, 2),
        };
        item.validate()?;
        Ok(item)
    }
}