use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sales_invoice_lines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub invoice_line_id: i64,
    pub invoice_id: Option<i64>,
    pub line_number: Option<i32>,
    pub description: Option<String>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub quantity_invoiced: Option<Decimal>,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub unit_selling_price: Option<Decimal>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sales_invoice::Entity",
        from = "Column::InvoiceId",
        to = "super::sales_invoice::Column::InvoiceId"
    )]
    SalesInvoice,
}

impl Related<super::sales_invoice::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesInvoice.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
