use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sales_order_headers")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub header_id: i64,
    pub order_number: String,
    pub order_type_id: Option<i64>,
    pub sold_to_org_id: Option<i64>,
    pub ordered_date: Option<NaiveDate>,
    pub status_code: Option<String>,
    pub location_id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::sales_order_lines::Entity")]
    SalesOrderLines,
    #[sea_orm(has_many = "super::order_fulfillments::Entity")]
    OrderFulfillments,
}

impl Related<super::sales_order_lines::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SalesOrderLines.def()
    }
}

impl Related<super::order_fulfillments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::OrderFulfillments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {} 