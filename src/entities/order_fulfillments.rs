use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "order_fulfillments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub fulfillment_id: i64,
    pub sales_order_header_id: Option<i64>,
    pub sales_order_line_id: Option<i64>,
    pub shipped_date: Option<NaiveDate>,
    pub released_status: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sales_order_headers::Entity",
        from = "Column::SalesOrderHeaderId",
        to = "super::sales_order_headers::Column::HeaderId"
    )]
    SalesOrderHeader,
    #[sea_orm(
        belongs_to = "super::sales_order_lines::Entity",
        from = "Column::SalesOrderLineId",
        to = "super::sales_order_lines::Column::LineId"
    )]
    SalesOrderLine,
}

impl Related<super::sales_order_headers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderHeader.def()
    }
}

impl Related<super::sales_order_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderLine.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 