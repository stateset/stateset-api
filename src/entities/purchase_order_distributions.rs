use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "purchase_order_distributions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub po_distribution_id: i64,
    pub po_line_id: Option<i64>,
    pub quantity_ordered: Option<Decimal>,
    pub destination_type: Option<String>,
    pub charge_account_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::purchase_order_lines::Entity",
        from = "Column::PoLineId",
        to = "super::purchase_order_lines::Column::PoLineId"
    )]
    PurchaseOrderLine,
}

impl Related<super::purchase_order_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PurchaseOrderLine.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 