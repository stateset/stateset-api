use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum PurchaseOrderStatus {
    #[sea_orm(string_value = "Draft")]
    Draft,
    #[sea_orm(string_value = "Submitted")]
    Submitted,
    #[sea_orm(string_value = "Approved")]
    Approved,
    #[sea_orm(string_value = "Rejected")]
    Rejected,
    #[sea_orm(string_value = "Ordered")]
    Ordered,
    #[sea_orm(string_value = "PartiallyReceived")]
    PartiallyReceived,
    #[sea_orm(string_value = "Received")]
    Received,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "purchase_orders")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub po_number: String,
    pub supplier_id: Uuid,
    pub status: PurchaseOrderStatus,
    pub order_date: DateTime<Utc>,
    pub expected_delivery_date: Option<chrono::NaiveDate>,
    pub total_amount: Decimal,
    pub currency: String,
    pub notes: Option<String>,
    pub created_by: Uuid,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::suppliers::Entity",
        from = "Column::SupplierId",
        to = "crate::models::suppliers::Column::Id"
    )]
    Supplier,
}

impl Related<crate::models::suppliers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Supplier.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
