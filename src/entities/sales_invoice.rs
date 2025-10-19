use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sales_invoices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub invoice_id: i64,
    pub trx_number: String,
    pub bill_to_customer_id: Option<i64>,
    pub trx_date: Option<Date>,
    pub trx_type: Option<String>,
    pub status: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::sales_invoice_line::Entity")]
    SalesInvoiceLines,
}

impl Related<super::sales_invoice_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesInvoiceLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
