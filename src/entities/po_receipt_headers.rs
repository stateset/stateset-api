use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "po_receipt_headers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub shipment_header_id: i64,
    pub receipt_num: String,
    pub vendor_id: Option<i64>,
    pub shipment_num: Option<String>,
    pub receipt_source: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::po_receipt_lines::Entity")]
    PoReceiptLines,
}

impl Related<super::po_receipt_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PoReceiptLines.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
