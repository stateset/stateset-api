use sea_orm::entity::prelude::*;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use serde::{Serialize, Deserialize};

/// Status for inventory reservations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReservationStatus {
    Pending,
    Confirmed,
    Allocated,
    Cancelled,
    Released,
    Expired,
}

impl ReservationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReservationStatus::Pending => "pending",
            ReservationStatus::Confirmed => "confirmed",
            ReservationStatus::Allocated => "allocated",
            ReservationStatus::Cancelled => "cancelled",
            ReservationStatus::Released => "released",
            ReservationStatus::Expired => "expired",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(ReservationStatus::Pending),
            "confirmed" => Some(ReservationStatus::Confirmed),
            "allocated" => Some(ReservationStatus::Allocated),
            "cancelled" => Some(ReservationStatus::Cancelled),
            "released" => Some(ReservationStatus::Released),
            "expired" => Some(ReservationStatus::Expired),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "inventory_reservations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub product_id: Uuid,
    pub location_id: Uuid,
    pub quantity: i32,
    pub status: String, // Storing as string in DB, but will convert to/from enum
    pub reference_id: Uuid,
    pub reference_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
}

// No relations for inventory reservations yet

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {
    // Add business logic here
    fn before_save(mut self, insert: bool) -> Result<Self, DbErr> {
        // Set default status for new reservations
        if insert && !matches!(self.status, sea_orm::ActiveValue::Set(_)) {
            self.status = Set("pending".to_string());
        }
        
        // Validate the reservation status
        if let sea_orm::ActiveValue::Set(status_str) = &self.status {
            if ReservationStatus::from_str(status_str).is_none() {
                return Err(DbErr::Custom(format!("Invalid reservation status: {}", status_str)));
            }
        }
        
        // Validate quantity
        if let sea_orm::ActiveValue::Set(qty) = &self.quantity {
            if *qty <= 0 {
                return Err(DbErr::Custom("Reservation quantity must be positive".to_string()));
            }
        }
        
        Ok(self)
    }
}