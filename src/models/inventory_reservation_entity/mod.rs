use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Reservation Status enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ReservationStatus {
    #[sea_orm(string_value = "Pending")]
    Pending,

    #[sea_orm(string_value = "Reserved")]
    Reserved,

    #[sea_orm(string_value = "PartiallyReserved")]
    PartiallyReserved,

    #[sea_orm(string_value = "Released")]
    Released,

    #[sea_orm(string_value = "Expired")]
    Expired,

    #[sea_orm(string_value = "Cancelled")]
    Cancelled,
}

/// Reservation Type enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum ReservationType {
    #[sea_orm(string_value = "Order")]
    Order,

    #[sea_orm(string_value = "WorkOrder")]
    WorkOrder,

    #[sea_orm(string_value = "Transfer")]
    Transfer,

    #[sea_orm(string_value = "CustomerHold")]
    CustomerHold,

    #[sea_orm(string_value = "QualityHold")]
    QualityHold,
}

// Implement Display for ReservationType
impl fmt::Display for ReservationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReservationType::Order => write!(f, "Order"),
            ReservationType::WorkOrder => write!(f, "WorkOrder"),
            ReservationType::Transfer => write!(f, "Transfer"),
            ReservationType::CustomerHold => write!(f, "CustomerHold"),
            ReservationType::QualityHold => write!(f, "QualityHold"),
        }
    }
}

impl fmt::Display for ReservationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReservationStatus::Reserved => write!(f, "reserved"),
            ReservationStatus::PartiallyReserved => write!(f, "partially_reserved"),
            ReservationStatus::Released => write!(f, "released"),
            ReservationStatus::Expired => write!(f, "expired"),
            ReservationStatus::Pending => write!(f, "pending"),
            ReservationStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Inventory Reservation entity model.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_reservations")]
pub struct Model {
    #[sea_orm(primary_key, column_type = "Uuid")]
    pub id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub inventory_level_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub product_id: Uuid,

    #[sea_orm(column_type = "Uuid")]
    pub warehouse_id: Uuid,

    pub reservation_type: ReservationType,

    #[sea_orm(column_type = "Uuid")]
    pub reference_id: Uuid,

    pub quantity_reserved: i32,

    pub quantity_released: i32,

    pub status: ReservationStatus,

    pub lot_numbers: Option<String>,

    pub notes: Option<String>,

    pub created_by: Option<String>,

    pub created_at: DateTime<Utc>,

    pub updated_at: DateTime<Utc>,

    pub expires_at: Option<DateTime<Utc>>,
}

/// Inventory Reservation entity relations
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "crate::models::inventory_level_entity::Entity",
        from = "Column::InventoryLevelId",
        to = "crate::models::inventory_level_entity::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    InventoryLevel,
}

impl Related<crate::models::inventory_level_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InventoryLevel.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Creates a new Inventory Reservation.
    pub fn new(
        inventory_level_id: Uuid,
        product_id: Uuid,
        warehouse_id: Uuid,
        reservation_type: ReservationType,
        reference_id: Uuid,
        quantity_reserved: i32,
        lot_numbers: Option<String>,
        notes: Option<String>,
        created_by: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            inventory_level_id,
            product_id,
            warehouse_id,
            reservation_type,
            reference_id,
            quantity_reserved,
            quantity_released: 0,
            status: ReservationStatus::Reserved,
            lot_numbers,
            notes,
            created_by,
            created_at: now,
            updated_at: now,
            expires_at,
        }
    }

    /// Releases a quantity from the reservation.
    pub fn release(&mut self, quantity: i32) -> i32 {
        let quantity_to_release = quantity.min(self.quantity_reserved - self.quantity_released);
        self.quantity_released += quantity_to_release;

        // Update status based on released quantity
        if self.quantity_released >= self.quantity_reserved {
            self.status = ReservationStatus::Released;
        } else if self.quantity_released > 0 {
            self.status = ReservationStatus::PartiallyReserved;
        }

        self.updated_at = Utc::now();

        quantity_to_release
    }

    /// Cancels the reservation.
    pub fn cancel(&mut self) {
        self.status = ReservationStatus::Cancelled;
        self.updated_at = Utc::now();
    }

    /// Checks if the reservation is expired.
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }

    /// Marks the reservation as expired.
    pub fn mark_expired(&mut self) {
        self.status = ReservationStatus::Expired;
        self.updated_at = Utc::now();
    }

    /// Extends the expiration date.
    pub fn extend_expiration(&mut self, new_expiration: DateTime<Utc>) {
        self.expires_at = Some(new_expiration);
        self.updated_at = Utc::now();
    }

    /// Updates the reservation quantity.
    pub fn update_quantity(&mut self, new_quantity: i32) {
        self.quantity_reserved = new_quantity;

        // Update status based on new quantity and released quantity
        if self.quantity_released >= new_quantity {
            self.status = ReservationStatus::Released;
        } else if self.quantity_released > 0 {
            self.status = ReservationStatus::PartiallyReserved;
        } else {
            self.status = ReservationStatus::Reserved;
        }

        self.updated_at = Utc::now();
    }
}
