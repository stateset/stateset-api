use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "po_receipt_lines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub shipment_line_id: i64,
    pub shipment_header_id: Option<i64>,
    pub item_id: Option<i64>,
    pub po_header_id: Option<i64>,
    pub po_line_id: Option<i64>,
    pub quantity_received: Option<rust_decimal::Decimal>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::po_receipt_headers::Entity",
        from = "Column::ShipmentHeaderId",
        to = "super::po_receipt_headers::Column::ShipmentHeaderId"
    )]
    PoReceiptHeaders,
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::ItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ItemMaster,
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

impl ActiveModelBehavior for ActiveModel {}

// Related implementations for other entities
impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
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

impl Related<super::po_receipt_headers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PoReceiptHeaders.def()
    }
}