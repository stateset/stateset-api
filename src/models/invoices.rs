use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use sea_orm::Set;
use serde::{Deserialize, Serialize};
use validator::Validate;

// Invoice Model (updated to include relation to line items)
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "invoices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub account_name: Option<String>,
    pub order_id: Option<String>,
    pub account_id: Option<String>,
    pub account_country: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub amount_due: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub amount_paid: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub amount_remaining: Option<Decimal>,
    pub billing_reason: Option<String>,
    pub collection_method: Option<String>,
    pub created: Option<DateTime<Utc>>,
    pub currency: Option<String>,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub due_date: Option<NaiveDate>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub ending_balance: Option<Decimal>,
    pub invoice_pdf: Option<String>,
    pub number: Option<i32>,
    pub paid: Option<bool>,
    pub period_end: Option<NaiveDate>,
    pub period_start: Option<NaiveDate>,
    pub status: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub subtotal: Option<Decimal>,
    pub invoice_name: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub total: Option<Decimal>,
    pub vendor_id: Option<String>,
    pub supplier_id: Option<String>,
    pub invoice_date: Option<NaiveDate>,
    pub payment_terms: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub discount_amount: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub tax_amount: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((10, 2)))")]
    pub shipping_amount: Option<Decimal>,
    pub notes: Option<String>,
    pub is_recurring: Option<bool>,
    pub recurrence_frequency: Option<String>,
    pub last_reminder_sent: Option<DateTime<Utc>>,
    pub payment_method: Option<String>,
    pub currency_exchange_rate: Option<Decimal>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::invoice_line_item::Entity")]
    InvoiceLineItems,
    #[sea_orm(
        belongs_to = "super::account::Entity",
        from = "Column::AccountId",
        to = "super::account::Column::Id"
    )]
    Account,
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id"
    )]
    Order,
}

impl Related<super::invoice_line_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InvoiceLineItems.def()
    }
}

impl Related<super::account::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Account.def()
    }
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    // ... (previous methods remain the same)

    pub async fn add_line_item(
        &self,
        line_item: super::invoice_line_item::Model,
        db: &DatabaseConnection,
    ) -> Result<(), DbErr> {
        let line_item = super::invoice_line_item::ActiveModel {
            id: Set(line_item.id),
            invoice_id: Set(self.id.clone()),
            description: Set(line_item.description),
            quantity: Set(line_item.quantity),
            unit_price: Set(line_item.unit_price),
            amount: Set(line_item.amount),
            product_id: Set(line_item.product_id),
            sku: Set(line_item.sku),
            tax_rate: Set(line_item.tax_rate),
            tax_amount: Set(line_item.tax_amount),
            discount_amount: Set(line_item.discount_amount),
            discount_type: Set(line_item.discount_type),
            notes: Set(line_item.notes),
        };
        super::invoice_line_item::Entity::insert(line_item)
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn calculate_total(&mut self, db: &DatabaseConnection) -> Result<(), DbErr> {
        let line_items = super::invoice_line_item::Entity::find()
            .filter(super::invoice_line_item::Column::InvoiceId.eq(self.id.clone()))
            .all(db)
            .await?;

        let subtotal: Decimal = line_items.iter().map(|item| item.amount).sum();
        let tax_amount: Decimal = line_items.iter().filter_map(|item| item.tax_amount).sum();
        let discount_amount: Decimal = line_items
            .iter()
            .filter_map(|item| item.discount_amount)
            .sum();

        self.subtotal = Some(subtotal);
        self.tax_amount = Some(tax_amount);
        self.discount_amount = Some(discount_amount);
        self.total = Some(
            subtotal + tax_amount - discount_amount + self.shipping_amount.unwrap_or(Decimal::ZERO),
        );
        self.amount_due = self.total;
        self.amount_remaining = self.total;

        Ok(())
    }
}
