use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "purchase_invoice_lines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub ap_invoice_line_id: i64,
    pub ap_invoice_id: Option<i64>,
    pub line_type_code: Option<String>,
    pub amount: Option<rust_decimal::Decimal>,
    pub quantity: Option<rust_decimal::Decimal>,
    pub po_header_id: Option<i64>,
    pub po_line_id: Option<i64>,
    pub sku: Option<String>,
    pub po_number: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::purchase_invoices::Entity",
        from = "Column::ApInvoiceId",
        to = "super::purchase_invoices::Column::ApInvoiceId"
    )]
    PurchaseInvoices,
    #[sea_orm(
        belongs_to = "super::purchase_order_headers::Entity",
        from = "Column::PoHeaderId",
        to = "super::purchase_order_headers::Column::PoHeaderId"
    )]
    PurchaseOrderHeaders,
    #[sea_orm(
        belongs_to = "super::purchase_order_lines::Entity",
        from = "Column::PoLineId",
        to = "super::purchase_order_lines::Column::PoLineId"
    )]
    PurchaseOrderLines,
}

impl Related<super::purchase_invoices::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseInvoices.def()
    }
}

impl Related<super::purchase_order_headers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrderHeaders.def()
    }
}

impl Related<super::purchase_order_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrderLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
