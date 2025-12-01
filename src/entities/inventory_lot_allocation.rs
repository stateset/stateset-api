use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_lot_allocations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub allocation_id: i64,
    pub lot_id: i64,
    pub reservation_id: Option<Uuid>,
    pub reference_type: String,
    pub reference_id: Uuid,
    #[sea_orm(column_type = "Decimal(Some((19, 4)))")]
    pub quantity_allocated: rust_decimal::Decimal,
    pub allocated_at: DateTime<Utc>,
    pub allocated_by: Option<String>,
    pub fulfilled_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::inventory_lot::Entity",
        from = "Column::LotId",
        to = "super::inventory_lot::Column::LotId"
    )]
    InventoryLot,
    #[sea_orm(
        belongs_to = "super::inventory_reservation::Entity",
        from = "Column::ReservationId",
        to = "super::inventory_reservation::Column::ReservationId"
    )]
    InventoryReservation,
}

impl Related<super::inventory_lot::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLot.def()
    }
}

impl Related<super::inventory_reservation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryReservation.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
