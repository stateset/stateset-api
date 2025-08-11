use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "purchase_order_headers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub po_header_id: i64,
    pub po_number: String,
    pub type_code: Option<String>,
    pub vendor_id: Option<i64>,
    pub agent_id: Option<i64>,
    pub approved_flag: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::purchase_order_lines::Entity")]
    PurchaseOrderLines,
    #[sea_orm(has_many = "super::purchase_invoice_lines::Entity")]
    PurchaseInvoiceLines,
    #[sea_orm(has_many = "super::po_receipt_lines::Entity")]
    PoReceiptLines,
}

impl Related<super::purchase_order_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrderLines.def()
    }
}

impl Related<super::purchase_invoice_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseInvoiceLines.def()
    }
}

impl Related<super::po_receipt_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PoReceiptLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 