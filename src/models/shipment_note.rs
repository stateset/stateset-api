use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "shipment_notes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub shipment_id: Uuid,
    pub note: String,
    pub created_by: Option<Uuid>,
    pub is_internal: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub type NewShipmentNote = Model;

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::shipment::Entity",
        from = "Column::ShipmentId",
        to = "crate::models::shipment::Column::Id"
    )]
    Shipment,
}

impl Related<crate::models::shipment::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Shipment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
