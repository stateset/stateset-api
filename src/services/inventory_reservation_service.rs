//! Inventory Reservation Service
//!
//! Provides functionality for managing inventory reservations, including
//! cleanup of expired reservations and querying reservation status.

use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, instrument, warn};
use uuid::Uuid;

use crate::entities::inventory_reservation::{
    self, Entity as InventoryReservationEntity, ReservationStatus,
};
use crate::errors::ServiceError;

/// Result of cleaning up expired reservations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    /// Number of reservations marked as expired.
    pub expired_count: u64,
    /// Number of reservations that were already expired (skipped).
    pub already_expired_count: u64,
    /// Timestamp when cleanup was performed.
    pub cleaned_at: DateTime<Utc>,
}

/// Summary of a reservation for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservationSummary {
    pub id: Uuid,
    pub product_id: Uuid,
    pub location_id: Uuid,
    pub quantity: i32,
    pub status: String,
    pub reference_id: Uuid,
    pub reference_type: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub is_expired: bool,
}

impl From<inventory_reservation::Model> for ReservationSummary {
    fn from(model: inventory_reservation::Model) -> Self {
        let is_expired = model
            .expires_at
            .map(|exp| exp < Utc::now())
            .unwrap_or(false);
        Self {
            id: model.id,
            product_id: model.product_id,
            location_id: model.location_id,
            quantity: model.quantity,
            status: model.status,
            reference_id: model.reference_id,
            reference_type: model.reference_type,
            expires_at: model.expires_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
            is_expired,
        }
    }
}

/// Service for managing inventory reservations.
#[derive(Clone)]
pub struct InventoryReservationService {
    db_pool: Arc<DatabaseConnection>,
}

impl InventoryReservationService {
    pub fn new(db_pool: Arc<DatabaseConnection>) -> Self {
        Self { db_pool }
    }

    /// Marks all expired reservations as "expired" status.
    ///
    /// This should be called periodically (e.g., via a cron job or background task)
    /// to clean up reservations that have passed their expiration time.
    ///
    /// # Returns
    /// A `CleanupResult` containing counts of processed reservations.
    #[instrument(skip(self))]
    pub async fn cleanup_expired_reservations(&self) -> Result<CleanupResult, ServiceError> {
        let db = &*self.db_pool;
        let now = Utc::now();

        // Find all reservations that:
        // 1. Have an expires_at timestamp in the past
        // 2. Are not already in a terminal state (cancelled, released, expired)
        let expired_reservations = InventoryReservationEntity::find()
            .filter(inventory_reservation::Column::ExpiresAt.lt(now))
            .filter(inventory_reservation::Column::Status.ne(ReservationStatus::Cancelled.as_str()))
            .filter(inventory_reservation::Column::Status.ne(ReservationStatus::Released.as_str()))
            .filter(inventory_reservation::Column::Status.ne(ReservationStatus::Expired.as_str()))
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        let mut expired_count = 0u64;
        let already_expired_count = 0u64;

        for reservation in expired_reservations {
            let mut active: inventory_reservation::ActiveModel = reservation.clone().into();
            active.status = Set(ReservationStatus::Expired.as_str().to_string());
            active.updated_at = Set(Some(now));

            match active.update(db).await {
                Ok(_) => {
                    expired_count += 1;
                    info!(
                        reservation_id = %reservation.id,
                        product_id = %reservation.product_id,
                        "Marked reservation as expired"
                    );
                }
                Err(e) => {
                    warn!(
                        reservation_id = %reservation.id,
                        error = %e,
                        "Failed to mark reservation as expired"
                    );
                }
            }
        }

        info!(
            expired_count = expired_count,
            "Completed expired reservation cleanup"
        );

        Ok(CleanupResult {
            expired_count,
            already_expired_count,
            cleaned_at: now,
        })
    }

    /// Gets a reservation by ID.
    #[instrument(skip(self))]
    pub async fn get_reservation(
        &self,
        reservation_id: Uuid,
    ) -> Result<Option<ReservationSummary>, ServiceError> {
        let db = &*self.db_pool;

        let reservation = InventoryReservationEntity::find_by_id(reservation_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(reservation.map(ReservationSummary::from))
    }

    /// Lists reservations with pagination and optional filters.
    #[instrument(skip(self))]
    pub async fn list_reservations(
        &self,
        page: u64,
        limit: u64,
        status_filter: Option<&str>,
        product_id_filter: Option<Uuid>,
        include_expired: bool,
    ) -> Result<(Vec<ReservationSummary>, u64), ServiceError> {
        if page == 0 {
            return Err(ServiceError::ValidationError(
                "Page number must be greater than 0".to_string(),
            ));
        }
        if limit == 0 || limit > 1000 {
            return Err(ServiceError::ValidationError(
                "Limit must be between 1 and 1000".to_string(),
            ));
        }

        let db = &*self.db_pool;

        let mut query = InventoryReservationEntity::find();

        // Apply status filter
        if let Some(status) = status_filter {
            query = query.filter(inventory_reservation::Column::Status.eq(status));
        }

        // Apply product filter
        if let Some(product_id) = product_id_filter {
            query = query.filter(inventory_reservation::Column::ProductId.eq(product_id));
        }

        // Exclude expired unless requested
        if !include_expired {
            query = query.filter(
                inventory_reservation::Column::Status.ne(ReservationStatus::Expired.as_str()),
            );
        }

        query = query.order_by_desc(inventory_reservation::Column::CreatedAt);

        let paginator = query.paginate(db, limit);
        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Failed to count reservations: {}", e)))?;

        let models = paginator.fetch_page(page - 1).await.map_err(|e| {
            ServiceError::InternalError(format!("Failed to fetch reservations page: {}", e))
        })?;

        let summaries = models.into_iter().map(ReservationSummary::from).collect();

        Ok((summaries, total))
    }

    /// Lists reservations for a specific reference (e.g., order).
    #[instrument(skip(self))]
    pub async fn list_reservations_by_reference(
        &self,
        reference_id: Uuid,
        reference_type: &str,
    ) -> Result<Vec<ReservationSummary>, ServiceError> {
        let db = &*self.db_pool;

        let reservations = InventoryReservationEntity::find()
            .filter(inventory_reservation::Column::ReferenceId.eq(reference_id))
            .filter(inventory_reservation::Column::ReferenceType.eq(reference_type))
            .order_by_desc(inventory_reservation::Column::CreatedAt)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(reservations
            .into_iter()
            .map(ReservationSummary::from)
            .collect())
    }

    /// Gets statistics about reservations.
    #[instrument(skip(self))]
    pub async fn get_reservation_stats(&self) -> Result<ReservationStats, ServiceError> {
        let db = &*self.db_pool;
        let now = Utc::now();

        let total = InventoryReservationEntity::find()
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        let active = InventoryReservationEntity::find()
            .filter(
                inventory_reservation::Column::Status
                    .is_in([
                        ReservationStatus::Pending.as_str(),
                        ReservationStatus::Confirmed.as_str(),
                        ReservationStatus::Allocated.as_str(),
                    ]),
            )
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        let expired_but_not_marked = InventoryReservationEntity::find()
            .filter(inventory_reservation::Column::ExpiresAt.lt(now))
            .filter(inventory_reservation::Column::Status.ne(ReservationStatus::Expired.as_str()))
            .filter(inventory_reservation::Column::Status.ne(ReservationStatus::Cancelled.as_str()))
            .filter(inventory_reservation::Column::Status.ne(ReservationStatus::Released.as_str()))
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        let expiring_soon = InventoryReservationEntity::find()
            .filter(inventory_reservation::Column::ExpiresAt.gt(now))
            .filter(inventory_reservation::Column::ExpiresAt.lt(now + Duration::hours(24)))
            .filter(
                inventory_reservation::Column::Status
                    .is_in([
                        ReservationStatus::Pending.as_str(),
                        ReservationStatus::Confirmed.as_str(),
                        ReservationStatus::Allocated.as_str(),
                    ]),
            )
            .count(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(ReservationStats {
            total_reservations: total,
            active_reservations: active,
            expired_not_cleaned: expired_but_not_marked,
            expiring_within_24h: expiring_soon,
            stats_at: now,
        })
    }

    /// Cancels a reservation by ID.
    #[instrument(skip(self))]
    pub async fn cancel_reservation(
        &self,
        reservation_id: Uuid,
    ) -> Result<ReservationSummary, ServiceError> {
        let db = &*self.db_pool;

        let reservation = InventoryReservationEntity::find_by_id(reservation_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Reservation {} not found", reservation_id))
            })?;

        // Check if reservation is in a cancellable state
        let current_status = ReservationStatus::from_str(&reservation.status);
        match current_status {
            Some(ReservationStatus::Cancelled) => {
                return Err(ServiceError::ValidationError(
                    "Reservation is already cancelled".to_string(),
                ));
            }
            Some(ReservationStatus::Released) => {
                return Err(ServiceError::ValidationError(
                    "Cannot cancel a released reservation".to_string(),
                ));
            }
            Some(ReservationStatus::Expired) => {
                return Err(ServiceError::ValidationError(
                    "Cannot cancel an expired reservation".to_string(),
                ));
            }
            _ => {}
        }

        let mut active: inventory_reservation::ActiveModel = reservation.into();
        active.status = Set(ReservationStatus::Cancelled.as_str().to_string());
        active.updated_at = Set(Some(Utc::now()));

        let updated = active.update(db).await.map_err(ServiceError::db_error)?;

        info!(reservation_id = %reservation_id, "Cancelled reservation");

        Ok(ReservationSummary::from(updated))
    }
}

/// Statistics about reservations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReservationStats {
    pub total_reservations: u64,
    pub active_reservations: u64,
    pub expired_not_cleaned: u64,
    pub expiring_within_24h: u64,
    pub stats_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reservation_status_conversion() {
        assert_eq!(ReservationStatus::Pending.as_str(), "pending");
        assert_eq!(ReservationStatus::Expired.as_str(), "expired");
        assert_eq!(
            ReservationStatus::from_str("pending"),
            Some(ReservationStatus::Pending)
        );
        assert_eq!(ReservationStatus::from_str("invalid"), None);
    }

    #[test]
    fn test_cleanup_result_serialization() {
        let result = CleanupResult {
            expired_count: 5,
            already_expired_count: 2,
            cleaned_at: Utc::now(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("expired_count"));
    }
}
