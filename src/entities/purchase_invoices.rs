use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "purchase_invoices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub ap_invoice_id: i64,
    pub invoice_num: String,
    pub vendor_id: Option<i64>,
    pub invoice_date: Option<NaiveDate>,
    pub invoice_amount: Option<Decimal>,
    pub status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::purchase_invoice_lines::Entity")]
    PurchaseInvoiceLines,
}

impl Related<super::purchase_invoice_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseInvoiceLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 