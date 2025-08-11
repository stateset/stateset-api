use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// Customer entity - enhanced for eCommerce
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "customers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    #[sea_orm(nullable)]
    pub phone: Option<String>,
    pub accepts_marketing: bool,
    #[sea_orm(nullable)]
    pub customer_group_id: Option<Uuid>,
    #[sea_orm(nullable)]
    pub default_shipping_address_id: Option<Uuid>,
    #[sea_orm(nullable)]
    pub default_billing_address_id: Option<Uuid>,
    #[sea_orm(column_type = "Json")]
    pub tags: Json, // Vec<String> serialized
    #[sea_orm(column_type = "Json", nullable)]
    pub metadata: Option<Json>,
    pub email_verified: bool,
    #[sea_orm(nullable)]
    pub email_verified_at: Option<DateTime<Utc>>,
    pub status: CustomerStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::customer_address::Entity")]
    Addresses,
    // #[sea_orm(has_many = "crate::entities::order::Entity")]
    // Orders, // Commented out - Related trait not implemented yet
    #[sea_orm(has_many = "super::cart::Entity")]
    Carts,
    #[sea_orm(has_many = "super::wishlist::Entity")]
    Wishlists,
    #[sea_orm(
        belongs_to = "super::customer_group::Entity",
        from = "Column::CustomerGroupId",
        to = "super::customer_group::Column::Id"
    )]
    CustomerGroup,
}

impl Related<super::customer_address::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Addresses.def()
    }
}

// impl Related<crate::entities::order::Entity> for Entity {
//     fn to() -> RelationDef {
//         Relation::Orders.def()
//     }
// } // Commented out - Orders relation disabled

impl Related<super::cart::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Carts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

/// Customer status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(20))")]
pub enum CustomerStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "inactive")]
    Inactive,
    #[sea_orm(string_value = "suspended")]
    Suspended,
    #[sea_orm(string_value = "deleted")]
    Deleted,
} 