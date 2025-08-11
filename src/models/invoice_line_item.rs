use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

// Invoice Line Item Model
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "invoice_line_items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub invoice_id: String,
    pub description: String,
    pub quantity: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub unit_price: Decimal,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub amount: Decimal,
    pub product_id: Option<String>,
    pub sku: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub tax_rate: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub tax_amount: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub discount_amount: Option<Decimal>,
    pub discount_type: Option<String>,
    pub notes: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::invoices::Entity",
        from = "Column::InvoiceId",
        to = "super::invoices::Column::Id"
    )]
    Invoice,
}

impl Related<super::invoices::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invoice.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(
        invoice_id: String,
        description: String,
        quantity: Decimal,
        unit_price: Decimal,
    ) -> Self {
        let amount = quantity * unit_price;
        Self {
            id: Uuid::new_v4().to_string(),
            invoice_id,
            description,
            quantity,
            unit_price,
            amount,
            product_id: None,
            sku: None,
            tax_rate: None,
            tax_amount: None,
            discount_amount: None,
            discount_type: None,
            notes: None,
        }
    }

    pub fn apply_tax(&mut self, tax_rate: Decimal) {
        self.tax_rate = Some(tax_rate);
        self.tax_amount = Some(self.amount * tax_rate / Decimal::ONE_HUNDRED);
    }

    pub fn apply_discount(&mut self, discount_amount: Decimal, discount_type: String) {
        self.discount_amount = Some(discount_amount);
        if discount_type == "percentage" {
            self.amount -= self.amount * discount_amount / Decimal::ONE_HUNDRED;
        } else {
            self.amount -= discount_amount;
        }
        self.discount_type = Some(discount_type);
    }
}
