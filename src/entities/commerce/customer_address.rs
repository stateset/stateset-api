use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Customer address entity
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "customer_addresses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub customer_id: Uuid,
    #[sea_orm(nullable)]
    pub name: Option<String>,
    #[sea_orm(nullable)]
    pub company: Option<String>,
    pub address_line_1: String,
    #[sea_orm(nullable)]
    pub address_line_2: Option<String>,
    pub city: String,
    pub province: String,
    pub country_code: String,
    pub postal_code: String,
    #[sea_orm(nullable)]
    pub phone: Option<String>,
    pub is_default_shipping: bool,
    pub is_default_billing: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::customer::Entity",
        from = "Column::CustomerId",
        to = "super::customer::Column::Id"
    )]
    Customer,
}

impl Related<super::customer::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Customer.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}