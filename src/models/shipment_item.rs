use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, Validate)]
#[sea_orm(table_name = "shipment_items")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    pub shipment_id: Uuid,

    pub product_id: Uuid,

    pub quantity: i32,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::shipment::Entity",
        from = "Column::ShipmentId",
        to = "super::shipment::Column::Id",
        on_delete = "Cascade"
    )]
    Shipment,
}

impl Related<super::shipment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Shipment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
