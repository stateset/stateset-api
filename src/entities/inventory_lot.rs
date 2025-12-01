use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_lots")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub lot_id: i64,
    pub inventory_item_id: i64,
    pub location_id: i32,
    pub lot_number: String,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub quantity: rust_decimal::Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub original_quantity: rust_decimal::Decimal,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub unit_cost: Option<rust_decimal::Decimal>,
    pub expiration_date: Option<NaiveDate>,
    pub manufacture_date: Option<NaiveDate>,
    pub received_date: NaiveDate,
    pub supplier_lot_number: Option<String>,
    pub supplier_id: Option<i64>,
    pub po_number: Option<String>,
    pub po_line_id: Option<i64>,
    pub status: String,
    pub quality_status: Option<String>,
    pub quarantine_reason: Option<String>,
    pub quarantined_at: Option<DateTime<Utc>>,
    pub quarantined_by: Option<String>,
    pub released_at: Option<DateTime<Utc>>,
    pub released_by: Option<String>,
    pub notes: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub metadata: Option<Json>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::item_master::Entity",
        from = "Column::InventoryItemId",
        to = "super::item_master::Column::InventoryItemId"
    )]
    ItemMaster,
    #[sea_orm(
        belongs_to = "super::inventory_location::Entity",
        from = "Column::LocationId",
        to = "super::inventory_location::Column::LocationId"
    )]
    InventoryLocation,
    #[sea_orm(has_many = "super::inventory_lot_allocation::Entity")]
    InventoryLotAllocations,
}

impl Related<super::item_master::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ItemMaster.def()
    }
}

impl Related<super::inventory_location::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLocation.def()
    }
}

impl Related<super::inventory_lot_allocation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLotAllocations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

// Helper enums
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LotStatus {
    Available,
    Allocated,
    Quarantine,
    Expired,
    Consumed,
    Scrapped,
}

impl ToString for LotStatus {
    fn to_string(&self) -> String {
        match self {
            LotStatus::Available => "AVAILABLE".to_string(),
            LotStatus::Allocated => "ALLOCATED".to_string(),
            LotStatus::Quarantine => "QUARANTINE".to_string(),
            LotStatus::Expired => "EXPIRED".to_string(),
            LotStatus::Consumed => "CONSUMED".to_string(),
            LotStatus::Scrapped => "SCRAPPED".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityStatus {
    Pending,
    Passed,
    Failed,
    Conditional,
}

impl ToString for QualityStatus {
    fn to_string(&self) -> String {
        match self {
            QualityStatus::Pending => "PENDING".to_string(),
            QualityStatus::Passed => "PASSED".to_string(),
            QualityStatus::Failed => "FAILED".to_string(),
            QualityStatus::Conditional => "CONDITIONAL".to_string(),
        }
    }
}
