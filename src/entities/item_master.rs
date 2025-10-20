use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "item_master")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub inventory_item_id: i64,
    pub organization_id: i64,
    pub item_number: String,
    pub description: Option<String>,
    pub primary_uom_code: Option<String>,
    pub item_type: Option<String>,
    pub status_code: Option<String>,
    pub lead_time_weeks: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::inventory_balance::Entity")]
    InventoryBalances,
    #[sea_orm(has_many = "super::bom_header::Entity")]
    BomHeaders,
    #[sea_orm(has_many = "super::bom_line::Entity")]
    BomLinesAsComponent,
    #[sea_orm(has_many = "super::manufacturing_work_orders::Entity")]
    ManufacturingWorkOrders,
    #[sea_orm(has_many = "super::sales_order_line::Entity")]
    SalesOrderLines,
    #[sea_orm(has_many = "super::purchase_order_lines::Entity")]
    PurchaseOrderLines,
    #[sea_orm(has_many = "super::po_receipt_lines::Entity")]
    PoReceiptLines,
}

impl Related<super::inventory_balance::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryBalances.def()
    }
}

impl Related<super::bom_header::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BomHeaders.def()
    }
}

impl Related<super::bom_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BomLinesAsComponent.def()
    }
}

impl Related<super::manufacturing_work_orders::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ManufacturingWorkOrders.def()
    }
}

impl Related<super::sales_order_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderLines.def()
    }
}

impl Related<super::purchase_order_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrderLines.def()
    }
}

impl Related<super::po_receipt_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PoReceiptLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
