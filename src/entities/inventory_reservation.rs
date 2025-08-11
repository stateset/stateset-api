use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelBehavior, ActiveValue, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use async_trait::async_trait;

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

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C: ConnectionTrait>(self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut active_model = self;
        let now = Utc::now();
        
        if insert {
            active_model.created_at = Set(now);
            
            // Generate UUID if not set
            if let ActiveValue::NotSet = active_model.id {
                active_model.id = Set(Uuid::new_v4());
            }
        }
        
        // Always update the updated_at timestamp
        active_model.updated_at = Set(Some(now));
        
        Ok(active_model)
    }
}
