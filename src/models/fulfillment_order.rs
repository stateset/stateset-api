use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "Text")]
pub enum FulfillmentOrderStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,
    #[sea_orm(string_value = "Picking")]
    Picking,
    #[sea_orm(string_value = "Packed")]
    Packed,
    #[sea_orm(string_value = "Shipped")]
    Shipped,
    #[sea_orm(string_value = "Delivered")]
    Delivered,
    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "fulfillment_orders")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,
    #[sea_orm(column_type = "Uuid")]
    pub order_id: Uuid,
    pub status: FulfillmentOrderStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::order::Entity",
        from = "Column::OrderId",
        to = "super::order::Column::Id",
        on_delete = "Cascade"
    )]
    Order,
}

impl Related<super::order::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Order.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn new(order_id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            order_id,
            status: FulfillmentOrderStatus::Pending,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn update_status(&mut self, status: FulfillmentOrderStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }
}
