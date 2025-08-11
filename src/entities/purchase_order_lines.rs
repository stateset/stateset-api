use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "purchase_order_lines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub po_line_id: i64,
    pub po_header_id: Option<i64>,
    pub line_num: Option<i32>,
    pub item_id: Option<i64>,
    pub quantity: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub line_type_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::purchase_order_headers::Entity",
        from = "Column::PoHeaderId",
        to = "super::purchase_order_headers::Column::PoHeaderId"
    )]
    PurchaseOrderHeader,
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::ItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ItemMaster,
    #[sea_orm(has_many = "super::purchase_order_distributions::Entity")]
    PurchaseOrderDistributions,
    #[sea_orm(has_many = "super::purchase_invoice_lines::Entity")]
    PurchaseInvoiceLines,
    #[sea_orm(has_many = "super::po_receipt_lines::Entity")]
    PoReceiptLines,
}

impl Related<super::purchase_order_headers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrderHeader.def()
    }
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
    }
}

impl Related<super::purchase_order_distributions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrderDistributions.def()
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