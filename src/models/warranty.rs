use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "warranties")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub action_needed: bool,
    pub advanced_replacement: bool,
    #[validate(range(min = 0, message = "Amount must be non-negative"))]
    pub amount: Decimal,
    pub condition: Option<String>,
    pub condition_date: Option<DateTime<Utc>>,
    pub country: Option<String>,
    pub created_date: DateTime<Utc>,
    #[validate(email(message = "Invalid email format"))]
    pub customer_email: String,
    pub customer_id: i32,
    pub description: Option<String>,
    pub entered_by: Option<String>,
    pub issue: Option<String>,
    pub match_condition: bool,
    pub order_date: DateTime<Utc>,
    pub order_id: i32,
    pub reason_category: Option<String>,
    pub replacement_color: Option<String>,
    pub model: Option<String>,
    pub replacement_order_created: bool,
    pub reported_condition: Option<String>,
    pub requested_date: Option<DateTime<Utc>>,
    pub rma: String,
    pub scanned_serial_number: Option<String>,
    pub serial_number: Option<String>,
    pub shipstation_order_id: Option<String>,
    pub shipped_date: Option<DateTime<Utc>>,
    pub sso_id: Option<String>,
    pub status: WarrantyStatus,
    pub stripe_invoice_id: Option<String>,
    #[validate(range(min = 0, message = "Tax refunded must be non-negative"))]
    pub tax_refunded: Decimal,
    #[validate(range(min = 0, message = "Total refunded must be non-negative"))]
    pub total_refunded: Decimal,
    pub tracking_number: Option<String>,
    pub warehouse_received_date: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::warranty_line_item::Entity")]
    WarrantyLineItems,
}

impl Related<super::warranty_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WarrantyLineItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum WarrantyStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Approved")]
    Approved,
    #[sea_orm(string_value = "Rejected")]
    Rejected,
    #[sea_orm(string_value = "Processed")]
    Processed,
    #[sea_orm(string_value = "Completed")]
    Completed,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "warranty_line_items")]
pub struct WarrantyLineItem {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[validate(range(min = 0, message = "Amount must be non-negative"))]
    pub amount: Decimal,
    pub condition: Option<String>,
    pub flat_rate_shipping: bool,
    pub name: String,
    #[validate(range(min = 0, message = "Price must be non-negative"))]
    pub price: Decimal,
    pub warranty_id: i32,
    pub serial_number: Option<String>,
    pub sales_order_sku: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum WarrantyLineItemRelation {
    #[sea_orm(
        belongs_to = "super::warranty::Entity",
        from = "Column::WarrantyId",
        to = "super::warranty::Column::Id"
    )]
    Warranty,
}

impl Related<super::warranty::Entity> for WarrantyLineItem {
    fn to() -> RelationDef {
        WarrantyLineItemRelation::Warranty.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        customer_id: i32,
        customer_email: String,
        order_id: i32,
        rma: String,
    ) -> Result<Self, ValidationError> {
        let now = Utc::now();
        let warranty = Self {
            id: 0, // Assuming database will auto-increment this
            action_needed: false,
            advanced_replacement: false,
            amount: Decimal::new(0, 2),
            condition: None,
            condition_date: None,
            country: None,
            created_date: now,
            customer_email,
            customer_id,
            description: None,
            entered_by: None,
            issue: None,
            match_condition: false,
            order_date: now, // This should be set to the actual order date
            order_id,
            reason_category: None,
            replacement_color: None,
            model: None,
            replacement_order_created: false,
            reported_condition: None,
            requested_date: None,
            rma,
            scanned_serial_number: None,
            serial_number: None,
            shipstation_order_id: None,
            shipped_date: None,
            sso_id: None,
            status: WarrantyStatus::Pending,
            stripe_invoice_id: None,
            tax_refunded: Decimal::new(0, 2),
            total_refunded: Decimal::new(0, 2),
            tracking_number: None,
            warehouse_received_date: None,
        };
        warranty.validate()?;
        Ok(warranty)
    }

    pub fn update_status(&mut self, new_status: WarrantyStatus) -> Result<(), String> {
        if self.status == WarrantyStatus::Completed {
            return Err("Cannot update status of a completed warranty".into());
        }
        self.status = new_status;
        Ok(())
    }

    pub fn add_line_item(&self, line_item: WarrantyLineItem) -> Result<(), ValidationError> {
        // Here you would typically save the line item to the database
        // For this example, we'll just validate the line item
        line_item.validate()?;
        Ok(())
    }
}

impl WarrantyStatus {
    pub fn is_final(&self) -> bool {
        matches!(self, WarrantyStatus::Completed | WarrantyStatus::Rejected)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            WarrantyStatus::Pending => "Pending",
            WarrantyStatus::Approved => "Approved",
            WarrantyStatus::Rejected => "Rejected",
            WarrantyStatus::Processed => "Processed",
            WarrantyStatus::Completed => "Completed",
            WarrantyStatus::Replaced => "Replaced",
        }
    }
}

impl WarrantyLineItem {
    pub fn new(
        warranty_id: i32,
        amount: Decimal,
        name: String,
        price: Decimal,
        sales_order_sku: String,
    ) -> Result<Self, ValidationError> {
        let item = Self {
            id: 0, // Assuming database will auto-increment this
            amount,
            condition: None,
            flat_rate_shipping: false,
            name,
            price,
            warranty_id,
            serial_number: None,
            sales_order_sku,
        };
        item.validate()?;
        Ok(item)
    }
}