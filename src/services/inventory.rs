use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
};
use tracing::instrument;
use uuid::Uuid;

use crate::entities::{
    inventory_balance::{self, Entity as InventoryBalanceEntity},
    inventory_location::{self, Entity as InventoryLocationEntity},
    item_master::{self, Entity as ItemMasterEntity},
};
use crate::errors::ServiceError;
use crate::events::{Event, EventSender};

/// Summary of inventory for a single item master record.
#[derive(Debug, Clone)]
pub struct InventorySnapshot {
    pub inventory_item_id: i64,
    pub item_number: String,
    pub description: Option<String>,
    pub primary_uom_code: Option<String>,
    pub organization_id: i64,
    pub total_on_hand: Decimal,
    pub total_allocated: Decimal,
    pub total_available: Decimal,
    pub locations: Vec<LocationBalance>,
}

/// Inventory values at a specific location.
#[derive(Debug, Clone)]
pub struct LocationBalance {
    pub location_id: i32,
    pub location_name: Option<String>,
    pub quantity_on_hand: Decimal,
    pub quantity_allocated: Decimal,
    pub quantity_available: Decimal,
    pub updated_at: DateTime<Utc>,
    /// Version number for optimistic locking. Use this when updating to prevent lost updates.
    pub version: i32,
}

/// Command payload for adjusting inventory.
#[derive(Debug, Clone)]
pub struct AdjustInventoryCommand {
    pub inventory_item_id: Option<i64>,
    pub item_number: Option<String>,
    pub location_id: i32,
    pub quantity_delta: Decimal,
    pub reason: Option<String>,
    /// Optional version for optimistic locking. If provided, the operation will fail
    /// if the current version doesn't match, preventing lost updates.
    pub expected_version: Option<i32>,
}

/// Command payload for reserving inventory.
#[derive(Debug, Clone)]
pub struct ReserveInventoryCommand {
    pub inventory_item_id: Option<i64>,
    pub item_number: Option<String>,
    pub location_id: i32,
    pub quantity: Decimal,
    pub reference_id: Option<Uuid>,
    pub reference_type: Option<String>,
    /// Optional version for optimistic locking.
    pub expected_version: Option<i32>,
}

/// Result of a reservation operation.
#[derive(Debug, Clone)]
pub struct ReservationOutcome {
    pub reservation_id: Uuid,
    pub balance: LocationBalance,
}

impl ReservationOutcome {
    pub fn id_str(&self) -> String {
        self.reservation_id.to_string()
    }
}

/// Command payload for releasing a reservation.
#[derive(Debug, Clone)]
pub struct ReleaseReservationCommand {
    pub inventory_item_id: Option<i64>,
    pub item_number: Option<String>,
    pub location_id: i32,
    pub quantity: Decimal,
}

/// Service for managing inventory quantities rooted in the item_master table.
///
/// This service provides comprehensive inventory management capabilities including:
/// - Listing and filtering inventory across locations
/// - Adjusting inventory quantities with optimistic locking
/// - Reserving and releasing inventory for orders
/// - Batch operations for bulk inventory updates
///
/// # Thread Safety
/// All operations are thread-safe and use database transactions for consistency.
///
/// # Error Handling
/// Operations return `ServiceError` variants for different failure modes:
/// - `ValidationError` for invalid input parameters
/// - `NotFound` for missing inventory items
/// - `InsufficientStock` when reserve operations exceed available quantity
/// - `ConcurrentModification` when optimistic locking fails
#[derive(Clone)]
pub struct InventoryService {
    db_pool: Arc<DatabaseConnection>,
    event_sender: EventSender,
}

impl InventoryService {
    /// Creates a new InventoryService instance.
    ///
    /// # Arguments
    /// * `db_pool` - Database connection pool for persistence operations
    /// * `event_sender` - Channel sender for emitting inventory events
    pub fn new(db_pool: Arc<DatabaseConnection>, event_sender: EventSender) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Returns a paginated list of inventory snapshots.
    ///
    /// Uses batch loading to avoid N+1 queries when fetching location balances.
    ///
    /// # Arguments
    /// * `page` - Page number (1-indexed, must be > 0)
    /// * `limit` - Items per page (must be 1-1000)
    ///
    /// # Returns
    /// A tuple of (snapshots, total_count) for pagination
    ///
    /// # Errors
    /// Returns `ValidationError` if page or limit are out of valid ranges
    #[instrument(skip(self))]
    pub async fn list_inventory(
        &self,
        page: u64,
        limit: u64,
    ) -> Result<(Vec<InventorySnapshot>, u64), ServiceError> {
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
        let paginator = ItemMasterEntity::find()
            .order_by_asc(item_master::Column::ItemNumber)
            .paginate(db, limit);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Failed to count items: {}", e)))?;
        let models = paginator.fetch_page(page - 1).await.map_err(|e| {
            ServiceError::InternalError(format!("Failed to fetch inventory page: {}", e))
        })?;

        // Batch load all balances for fetched items (avoids N+1 queries)
        let snapshots = self.batch_snapshots_for_items(&models).await?;

        Ok((snapshots, total))
    }

    /// Returns a filtered paginated list of inventory snapshots.
    /// Uses batch loading to avoid N+1 queries.
    #[instrument(skip(self))]
    pub async fn list_inventory_filtered(
        &self,
        page: u64,
        limit: u64,
        product_filter: Option<&str>,
        location_filter: Option<i32>,
        low_stock_threshold: Option<Decimal>,
    ) -> Result<(Vec<InventorySnapshot>, u64), ServiceError> {
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

        // Build query with filters
        let mut query = ItemMasterEntity::find();

        // Filter by product ID or item number
        if let Some(product_id_or_number) = product_filter {
            if let Ok(id) = product_id_or_number.parse::<i64>() {
                query = query.filter(item_master::Column::InventoryItemId.eq(id));
            } else {
                query =
                    query.filter(item_master::Column::ItemNumber.contains(product_id_or_number));
            }
        }

        query = query.order_by_asc(item_master::Column::ItemNumber);

        let paginator = query.paginate(db, limit);

        let total = paginator
            .num_items()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Failed to count items: {}", e)))?;
        let models = paginator.fetch_page(page - 1).await.map_err(|e| {
            ServiceError::InternalError(format!("Failed to fetch inventory page: {}", e))
        })?;

        // Batch load all balances for fetched items (avoids N+1 queries)
        let all_snapshots = self.batch_snapshots_for_items(&models).await?;

        // Apply post-fetch filters (location and low stock)
        let mut snapshots = Vec::new();
        for mut snapshot in all_snapshots {
            // Apply location filter if specified
            if let Some(loc_id) = location_filter {
                snapshot.locations.retain(|loc| loc.location_id == loc_id);
                if snapshot.locations.is_empty() {
                    continue; // Skip items not in this location
                }
                // Recalculate totals for filtered location
                snapshot.total_on_hand =
                    snapshot.locations.iter().map(|l| l.quantity_on_hand).sum();
                snapshot.total_allocated = snapshot
                    .locations
                    .iter()
                    .map(|l| l.quantity_allocated)
                    .sum();
                snapshot.total_available = snapshot
                    .locations
                    .iter()
                    .map(|l| l.quantity_available)
                    .sum();
            }

            // Apply low stock filter if specified
            if let Some(threshold) = low_stock_threshold {
                if snapshot.total_available >= threshold {
                    continue; // Skip items above threshold
                }
            }

            snapshots.push(snapshot);
        }

        Ok((snapshots, total))
    }

    /// Ensures an item_master record exists for the provided item number, creating or updating it.
    #[instrument(skip(self, description, primary_uom_code))]
    pub async fn ensure_item(
        &self,
        item_number: &str,
        organization_id: i64,
        description: Option<String>,
        primary_uom_code: Option<String>,
    ) -> Result<item_master::Model, ServiceError> {
        let db = &*self.db_pool;
        let trimmed_number = item_number.trim();
        if trimmed_number.is_empty() {
            return Err(ServiceError::ValidationError(
                "item_number cannot be empty".to_string(),
            ));
        }

        let existing = ItemMasterEntity::find()
            .filter(item_master::Column::ItemNumber.eq(trimmed_number))
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        match existing {
            Some(model) => {
                let mut active: item_master::ActiveModel = model.clone().into();
                let mut dirty = false;

                if let Some(desc) = description.clone() {
                    if model.description.as_deref() != Some(desc.as_str()) {
                        active.description = Set(Some(desc));
                        dirty = true;
                    }
                }
                if let Some(uom) = primary_uom_code.clone() {
                    if model.primary_uom_code.as_deref() != Some(uom.as_str()) {
                        active.primary_uom_code = Set(Some(uom));
                        dirty = true;
                    }
                }
                if model.organization_id != organization_id {
                    active.organization_id = Set(organization_id);
                    dirty = true;
                }

                if dirty {
                    active.updated_at = Set(Utc::now());
                    let updated = active.update(db).await.map_err(ServiceError::db_error)?;
                    Ok(updated)
                } else {
                    Ok(model)
                }
            }
            None => {
                let now = Utc::now();
                let new_item = item_master::ActiveModel {
                    organization_id: Set(organization_id),
                    item_number: Set(trimmed_number.to_string()),
                    description: Set(description),
                    primary_uom_code: Set(primary_uom_code),
                    item_type: Set(Some("STANDARD".to_string())),
                    status_code: Set(Some("ACTIVE".to_string())),
                    lead_time_weeks: Set(None),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                }
                .insert(db)
                .await
                .map_err(ServiceError::db_error)?;
                Ok(new_item)
            }
        }
    }

    /// Fetches a snapshot for a single inventory item id.
    #[instrument(skip(self))]
    pub async fn get_snapshot_by_id(
        &self,
        inventory_item_id: i64,
    ) -> Result<Option<InventorySnapshot>, ServiceError> {
        let db = &*self.db_pool;
        let item = ItemMasterEntity::find_by_id(inventory_item_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;
        match item {
            Some(model) => Ok(Some(self.snapshot_for_item(&model).await?)),
            None => Ok(None),
        }
    }

    /// Fetches a snapshot by item number.
    #[instrument(skip(self, item_number))]
    pub async fn get_snapshot_by_item_number(
        &self,
        item_number: &str,
    ) -> Result<Option<InventorySnapshot>, ServiceError> {
        let db = &*self.db_pool;
        let item = ItemMasterEntity::find()
            .filter(item_master::Column::ItemNumber.eq(item_number))
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;
        match item {
            Some(model) => Ok(Some(self.snapshot_for_item(&model).await?)),
            None => Ok(None),
        }
    }

    /// Fetches location-specific balance for provided item/location.
    #[instrument(skip(self))]
    pub async fn get_location_balance(
        &self,
        inventory_item_id: i64,
        location_id: i32,
    ) -> Result<Option<LocationBalance>, ServiceError> {
        let db = &*self.db_pool;
        let balance = InventoryBalanceEntity::find()
            .filter(inventory_balance::Column::InventoryItemId.eq(inventory_item_id))
            .filter(inventory_balance::Column::LocationId.eq(location_id))
            .find_also_related(InventoryLocationEntity)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(balance.map(|(model, location)| Self::map_balance(model, location)))
    }

    /// Adjusts inventory by delta amount for an item/location.
    #[instrument(skip(self, command))]
    pub async fn adjust_inventory(
        &self,
        command: AdjustInventoryCommand,
    ) -> Result<LocationBalance, ServiceError> {
        let db = &*self.db_pool;
        let item = self
            .resolve_item(
                db,
                command.inventory_item_id,
                command.item_number.as_deref(),
            )
            .await?;
        let delta = command.quantity_delta;
        if delta.is_zero() {
            return Err(ServiceError::ValidationError(
                "Adjustment quantity must be non-zero".to_string(),
            ));
        }
        let location_id = command.location_id;
        let reason = command
            .reason
            .unwrap_or_else(|| "MANUAL_ADJUSTMENT".to_string());

        let expected_version = command.expected_version;
        let (old_on_hand, updated_balance) = db
            .transaction::<_, (Decimal, inventory_balance::Model), ServiceError>(|txn| {
                Box::pin(async move {
                    let now = Utc::now();
                    // Use FOR UPDATE to prevent concurrent modifications (pessimistic locking)
                    let maybe_balance = InventoryBalanceEntity::find()
                        .filter(
                            inventory_balance::Column::InventoryItemId.eq(item.inventory_item_id),
                        )
                        .filter(inventory_balance::Column::LocationId.eq(location_id))
                        .lock_exclusive()
                        .one(txn)
                        .await
                        .map_err(ServiceError::db_error)?;

                    if let Some(existing) = maybe_balance {
                        // Optimistic locking: check version if provided
                        if let Some(expected) = expected_version {
                            if existing.version != expected {
                                return Err(ServiceError::Conflict(format!(
                                    "Inventory was modified by another request. Expected version {}, found {}. Please refresh and retry.",
                                    expected, existing.version
                                )));
                            }
                        }

                        let new_on_hand = existing.quantity_on_hand + delta;
                        // Always recompute available from on_hand - allocated to enforce invariants,
                        // even if legacy data drifted.
                        let new_available = new_on_hand - existing.quantity_allocated;
                        if new_on_hand < Decimal::ZERO {
                            return Err(ServiceError::ValidationError(
                                "Adjustment would result in negative on-hand quantity".to_string(),
                            ));
                        }
                        if new_available < Decimal::ZERO {
                            return Err(ServiceError::ValidationError(
                                "Adjustment would result in negative available quantity"
                                    .to_string(),
                            ));
                        }
                        let mut active: inventory_balance::ActiveModel = existing.clone().into();
                        active.quantity_on_hand = Set(new_on_hand);
                        active.quantity_available = Set(new_available);
                        // Increment version for optimistic locking
                        active.version = Set(existing.version + 1);
                        active.updated_at = Set(now.into());
                        let updated = active.update(txn).await.map_err(ServiceError::db_error)?;
                        Ok((existing.quantity_on_hand, updated))
                    } else {
                        if delta < Decimal::ZERO {
                            return Err(ServiceError::ValidationError(
                                "Cannot seed inventory with a negative adjustment".to_string(),
                            ));
                        }
                        let created = inventory_balance::ActiveModel {
                            inventory_item_id: Set(item.inventory_item_id),
                            location_id: Set(location_id),
                            quantity_on_hand: Set(delta),
                            quantity_allocated: Set(Decimal::ZERO),
                            quantity_available: Set(delta), // available = on_hand - allocated
                            // Initialize version to 1 for new records
                            version: Set(1),
                            created_at: Set(now.into()),
                            updated_at: Set(now.into()),
                            ..Default::default()
                        }
                        .insert(txn)
                        .await
                        .map_err(ServiceError::db_error)?;
                        Ok((Decimal::ZERO, created))
                    }
                })
            })
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        let location_model = InventoryLocationEntity::find_by_id(location_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;
        let balance = Self::map_balance(updated_balance.clone(), location_model.clone());

        self.emit_adjustment_event(
            item.inventory_item_id,
            location_id,
            old_on_hand,
            updated_balance.quantity_on_hand,
            reason,
        )
        .await?;

        Ok(balance)
    }

    /// Reserves inventory for a request, decrementing available and incrementing allocated.
    #[instrument(skip(self, command))]
    pub async fn reserve_inventory(
        &self,
        command: ReserveInventoryCommand,
    ) -> Result<ReservationOutcome, ServiceError> {
        let db = &*self.db_pool;
        let item = self
            .resolve_item(
                db,
                command.inventory_item_id,
                command.item_number.as_deref(),
            )
            .await?;
        if command.quantity <= Decimal::ZERO {
            return Err(ServiceError::ValidationError(
                "Reservation quantity must be positive".to_string(),
            ));
        }
        let location_id = command.location_id;
        let quantity = command.quantity;
        let expected_version = command.expected_version;

        let (updated_balance, reservation_id) = db
            .transaction::<_, (inventory_balance::Model, Uuid), ServiceError>(|txn| {
                let quantity = quantity;
                let reference_id = command.reference_id.unwrap_or_else(Uuid::new_v4);
                let reference_type = command
                    .reference_type
                    .clone()
                    .unwrap_or_else(|| "RESERVATION".to_string());
                Box::pin(async move {
                    let now = Utc::now();
                    // Use FOR UPDATE to prevent concurrent reservations (pessimistic locking)
                    let existing = InventoryBalanceEntity::find()
                        .filter(
                            inventory_balance::Column::InventoryItemId.eq(item.inventory_item_id),
                        )
                        .filter(inventory_balance::Column::LocationId.eq(location_id))
                        .lock_exclusive()
                        .one(txn)
                        .await
                        .map_err(ServiceError::db_error)?
                        .ok_or_else(|| {
                            ServiceError::NotFound(format!(
                                "No inventory for item {} at location {}",
                                item.inventory_item_id, location_id
                            ))
                        })?;

                    // Optimistic locking: check version if provided
                    if let Some(expected) = expected_version {
                        if existing.version != expected {
                            return Err(ServiceError::Conflict(format!(
                                "Inventory was modified by another request. Expected version {}, found {}. Please refresh and retry.",
                                expected, existing.version
                            )));
                        }
                    }

                    if existing.quantity_available < quantity {
                        return Err(ServiceError::ValidationError(format!(
                            "Insufficient available quantity for item {} at location {}. \
                             Requested: {}, Available: {}, On-hand: {}, Allocated: {}",
                            item.item_number, location_id, quantity, existing.quantity_available,
                            existing.quantity_on_hand, existing.quantity_allocated
                        )));
                    }

                    let mut active: inventory_balance::ActiveModel = existing.clone().into();
                    let new_allocated = existing.quantity_allocated + quantity;
                    active.quantity_allocated = Set(new_allocated);
                    active.quantity_available = Set(existing.quantity_on_hand - new_allocated);
                    // Increment version for optimistic locking
                    active.version = Set(existing.version + 1);
                    active.updated_at = Set(now.into());
                    let updated = active.update(txn).await.map_err(ServiceError::db_error)?;

                    let reservation_id = reference_id;
                    crate::events::outbox::enqueue(
                        txn,
                        "inventory",
                        None,
                        "InventoryReserved",
                        &serde_json::json!({
                            "inventory_item_id": item.inventory_item_id,
                            "location_id": location_id,
                            "reference_type": reference_type,
                            "reference_id": reservation_id,
                            "quantity": quantity,
                        }),
                    )
                    .await
                    .ok();

                    Ok((updated, reservation_id))
                })
            })
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        let location_model = InventoryLocationEntity::find_by_id(location_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;
        let balance = Self::map_balance(updated_balance.clone(), location_model.clone());

        self.emit_reservation_event(
            item.inventory_item_id,
            location_id,
            quantity,
            reservation_id,
            command.reference_type,
        )
        .await?;

        Ok(ReservationOutcome {
            reservation_id,
            balance,
        })
    }

    /// Convenience helper matching the legacy signature.
    pub async fn reserve_inventory_simple(
        &self,
        inventory_item_id: i64,
        location_id: i32,
        quantity: Decimal,
    ) -> Result<String, ServiceError> {
        let outcome = self
            .reserve_inventory(ReserveInventoryCommand {
                inventory_item_id: Some(inventory_item_id),
                item_number: None,
                location_id,
                quantity,
                reference_id: None,
                reference_type: None,
                expected_version: None,
            })
            .await?;
        Ok(outcome.id_str())
    }

    /// Releases a reservation, moving quantity from allocated back to available.
    #[instrument(skip(self, command))]
    pub async fn release_reservation(
        &self,
        command: ReleaseReservationCommand,
    ) -> Result<LocationBalance, ServiceError> {
        let db = &*self.db_pool;
        let item = self
            .resolve_item(
                db,
                command.inventory_item_id,
                command.item_number.as_deref(),
            )
            .await?;
        if command.quantity <= Decimal::ZERO {
            return Err(ServiceError::ValidationError(
                "Release quantity must be positive".to_string(),
            ));
        }
        let quantity = command.quantity;
        let location_id = command.location_id;

        let updated_balance = db
            .transaction::<_, inventory_balance::Model, ServiceError>(|txn| {
                let quantity = quantity;
                Box::pin(async move {
                    let now = Utc::now();
                    // Use FOR UPDATE to prevent concurrent releases (pessimistic locking)
                    let existing = InventoryBalanceEntity::find()
                        .filter(
                            inventory_balance::Column::InventoryItemId.eq(item.inventory_item_id),
                        )
                        .filter(inventory_balance::Column::LocationId.eq(location_id))
                        .lock_exclusive()
                        .one(txn)
                        .await
                        .map_err(ServiceError::db_error)?
                        .ok_or_else(|| {
                            ServiceError::NotFound(format!(
                                "No inventory for item {} at location {}",
                                item.inventory_item_id, location_id
                            ))
                        })?;

                    if existing.quantity_allocated < quantity {
                        return Err(ServiceError::ValidationError(format!(
                            "Cannot release {} units for item {} at location {} when only {} is allocated. \
                             Available: {}, On-hand: {}",
                            quantity, item.item_number, location_id, existing.quantity_allocated,
                            existing.quantity_available, existing.quantity_on_hand
                        )));
                    }

                    let mut active: inventory_balance::ActiveModel = existing.clone().into();
                    let new_allocated = existing.quantity_allocated - quantity;
                    active.quantity_allocated = Set(new_allocated);
                    active.quantity_available = Set(existing.quantity_on_hand - new_allocated);
                    // Increment version for optimistic locking
                    active.version = Set(existing.version + 1);
                    active.updated_at = Set(now.into());
                    let updated = active.update(txn).await.map_err(ServiceError::db_error)?;

                    crate::events::outbox::enqueue(
                        txn,
                        "inventory",
                        None,
                        "InventoryReleased",
                        &serde_json::json!({
                            "inventory_item_id": item.inventory_item_id,
                            "location_id": location_id,
                            "quantity": quantity,
                        }),
                    )
                    .await
                    .ok();

                    Ok(updated)
                })
            })
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        let location_model = InventoryLocationEntity::find_by_id(location_id)
            .one(db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(Self::map_balance(updated_balance, location_model))
    }

    /// Checks if enough available inventory exists.
    pub async fn is_in_stock(
        &self,
        inventory_item_id: i64,
        location_id: i32,
        quantity: Decimal,
    ) -> Result<bool, ServiceError> {
        let balance = self
            .get_location_balance(inventory_item_id, location_id)
            .await?;
        Ok(balance
            .map(|b| b.quantity_available >= quantity)
            .unwrap_or(false))
    }

    /// Transfers inventory between two locations atomically.
    #[instrument(skip(self))]
    pub async fn transfer_inventory(
        &self,
        inventory_item_id: i64,
        from_location: i32,
        to_location: i32,
        quantity: Decimal,
    ) -> Result<(), ServiceError> {
        if quantity <= Decimal::ZERO {
            return Err(ServiceError::ValidationError(
                "Transfer quantity must be positive".to_string(),
            ));
        }

        let db = &*self.db_pool;
        db.transaction::<_, (), ServiceError>(|txn| {
            Box::pin(async move {
                let now = Utc::now();

                // Use FOR UPDATE to lock both source and destination (pessimistic locking)
                let source = InventoryBalanceEntity::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(inventory_item_id))
                    .filter(inventory_balance::Column::LocationId.eq(from_location))
                    .lock_exclusive()
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "No inventory for item {} at location {}",
                            inventory_item_id, from_location
                        ))
                    })?;
                if source.quantity_available < quantity {
                    return Err(ServiceError::ValidationError(format!(
                        "Insufficient stock at source location {} for item {}. \
                         Requested: {}, Available: {}, On-hand: {}, Allocated: {}",
                        from_location,
                        inventory_item_id,
                        quantity,
                        source.quantity_available,
                        source.quantity_on_hand,
                        source.quantity_allocated
                    )));
                }

                let mut source_active: inventory_balance::ActiveModel = source.clone().into();
                let new_source_on_hand = source.quantity_on_hand - quantity;
                source_active.quantity_on_hand = Set(new_source_on_hand);
                source_active.quantity_available =
                    Set(new_source_on_hand - source.quantity_allocated);
                // Increment version for optimistic locking
                source_active.version = Set(source.version + 1);
                source_active.updated_at = Set(now.into());
                source_active
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                // Lock destination as well for atomicity
                let dest = InventoryBalanceEntity::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(inventory_item_id))
                    .filter(inventory_balance::Column::LocationId.eq(to_location))
                    .lock_exclusive()
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                if let Some(existing) = dest {
                    let mut active: inventory_balance::ActiveModel = existing.clone().into();
                    let new_on_hand = existing.quantity_on_hand + quantity;
                    active.quantity_on_hand = Set(new_on_hand);
                    active.quantity_available = Set(new_on_hand - existing.quantity_allocated);
                    // Increment version for optimistic locking
                    active.version = Set(existing.version + 1);
                    active.updated_at = Set(now.into());
                    active.update(txn).await.map_err(ServiceError::db_error)?;
                } else {
                    inventory_balance::ActiveModel {
                        inventory_item_id: Set(inventory_item_id),
                        location_id: Set(to_location),
                        quantity_on_hand: Set(quantity),
                        quantity_allocated: Set(Decimal::ZERO),
                        quantity_available: Set(quantity), // available = on_hand - allocated
                        // Initialize version to 1 for new records
                        version: Set(1),
                        created_at: Set(now.into()),
                        updated_at: Set(now.into()),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(ServiceError::db_error)?;
                }

                Ok(())
            })
        })
        .await
        .map_err(|e| ServiceError::InternalError(e.to_string()))
    }

    /// Batch loads inventory snapshots for multiple items in a single query.
    /// This avoids N+1 queries when fetching inventory for list operations.
    async fn batch_snapshots_for_items(
        &self,
        items: &[item_master::Model],
    ) -> Result<Vec<InventorySnapshot>, ServiceError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        let db = &*self.db_pool;

        // Collect all inventory_item_ids
        let item_ids: Vec<i64> = items.iter().map(|i| i.inventory_item_id).collect();

        // Single query to fetch all balances for all items with their locations
        let all_balances = InventoryBalanceEntity::find()
            .filter(inventory_balance::Column::InventoryItemId.is_in(item_ids.clone()))
            .find_also_related(InventoryLocationEntity)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        // Group balances by inventory_item_id using a HashMap for O(1) lookup
        let mut balances_by_item: std::collections::HashMap<
            i64,
            Vec<(inventory_balance::Model, Option<inventory_location::Model>)>,
        > = std::collections::HashMap::with_capacity(items.len());

        for (balance, location) in all_balances {
            balances_by_item
                .entry(balance.inventory_item_id)
                .or_default()
                .push((balance, location));
        }

        // Build snapshots for each item
        let mut snapshots = Vec::with_capacity(items.len());
        for item in items {
            let item_balances = balances_by_item
                .remove(&item.inventory_item_id)
                .unwrap_or_default();

            let mut total_on_hand = Decimal::ZERO;
            let mut total_allocated = Decimal::ZERO;
            let mut total_available = Decimal::ZERO;
            let mut location_balances = Vec::with_capacity(item_balances.len());

            for (balance, location) in item_balances {
                total_on_hand += balance.quantity_on_hand;
                total_allocated += balance.quantity_allocated;
                total_available += balance.quantity_available;
                location_balances.push(Self::map_balance(balance, location));
            }

            snapshots.push(InventorySnapshot {
                inventory_item_id: item.inventory_item_id,
                item_number: item.item_number.clone(),
                description: item.description.clone(),
                primary_uom_code: item.primary_uom_code.clone(),
                organization_id: item.organization_id,
                total_on_hand,
                total_allocated,
                total_available,
                locations: location_balances,
            });
        }

        Ok(snapshots)
    }

    async fn snapshot_for_item(
        &self,
        item: &item_master::Model,
    ) -> Result<InventorySnapshot, ServiceError> {
        let db = &*self.db_pool;
        let balances = InventoryBalanceEntity::find()
            .filter(inventory_balance::Column::InventoryItemId.eq(item.inventory_item_id))
            .find_also_related(InventoryLocationEntity)
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        let mut total_on_hand = Decimal::ZERO;
        let mut total_allocated = Decimal::ZERO;
        let mut total_available = Decimal::ZERO;
        let mut location_balances = Vec::with_capacity(balances.len());

        for (balance, location) in balances {
            total_on_hand += balance.quantity_on_hand;
            total_allocated += balance.quantity_allocated;
            total_available += balance.quantity_available;
            location_balances.push(Self::map_balance(balance, location));
        }

        Ok(InventorySnapshot {
            inventory_item_id: item.inventory_item_id,
            item_number: item.item_number.clone(),
            description: item.description.clone(),
            primary_uom_code: item.primary_uom_code.clone(),
            organization_id: item.organization_id,
            total_on_hand,
            total_allocated,
            total_available,
            locations: location_balances,
        })
    }

    fn map_balance(
        balance: inventory_balance::Model,
        location: Option<inventory_location::Model>,
    ) -> LocationBalance {
        LocationBalance {
            location_id: balance.location_id,
            location_name: location.map(|l| l.location_name),
            quantity_on_hand: balance.quantity_on_hand,
            quantity_allocated: balance.quantity_allocated,
            quantity_available: balance.quantity_available,
            updated_at: balance.updated_at.with_timezone(&Utc),
            version: balance.version,
        }
    }

    async fn resolve_item(
        &self,
        db: &DatabaseConnection,
        inventory_item_id: Option<i64>,
        item_number: Option<&str>,
    ) -> Result<item_master::Model, ServiceError> {
        if let Some(id) = inventory_item_id {
            return ItemMasterEntity::find_by_id(id)
                .one(db)
                .await
                .map_err(ServiceError::db_error)?
                .ok_or_else(|| ServiceError::NotFound(format!("Inventory item {} not found", id)));
        }

        if let Some(number) = item_number.map(|s| s.trim()).filter(|s| !s.is_empty()) {
            return ItemMasterEntity::find()
                .filter(item_master::Column::ItemNumber.eq(number))
                .one(db)
                .await
                .map_err(ServiceError::db_error)?
                .ok_or_else(|| {
                    ServiceError::NotFound(format!("Inventory item {} not found", number))
                });
        }

        Err(ServiceError::ValidationError(
            "Either inventory_item_id or item_number must be provided".to_string(),
        ))
    }

    async fn emit_adjustment_event(
        &self,
        inventory_item_id: i64,
        location_id: i32,
        old_quantity: Decimal,
        new_quantity: Decimal,
        reason: String,
    ) -> Result<(), ServiceError> {
        let old_qty = decimal_to_i32(old_quantity)?;
        let new_qty = decimal_to_i32(new_quantity)?;
        let event = Event::InventoryAdjusted {
            warehouse_id: uuid_from_i32(location_id),
            product_id: uuid_from_i64(inventory_item_id),
            old_quantity: old_qty,
            new_quantity: new_qty,
            reason_code: reason,
            transaction_id: Uuid::new_v4(),
            reference_number: None,
        };
        self.event_sender
            .send(event)
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))
    }

    async fn emit_reservation_event(
        &self,
        inventory_item_id: i64,
        location_id: i32,
        quantity: Decimal,
        reference_id: Uuid,
        reference_type: Option<String>,
    ) -> Result<(), ServiceError> {
        let qty = decimal_to_i32(quantity)?;
        let event = Event::InventoryReserved {
            warehouse_id: uuid_from_i32(location_id),
            product_id: uuid_from_i64(inventory_item_id),
            quantity: qty,
            reference_id,
            reference_type: reference_type.unwrap_or_else(|| "RESERVATION".to_string()),
            partial: false,
        };
        self.event_sender
            .send(event)
            .await
            .map_err(|e| ServiceError::EventError(e.to_string()))
    }
}

fn decimal_to_i32(value: Decimal) -> Result<i32, ServiceError> {
    value
        .round()
        .to_i32()
        .ok_or_else(|| ServiceError::ValidationError("Quantity overflow".to_string()))
}

fn uuid_from_i64(id: i64) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[8..16].copy_from_slice(&(id as u64).to_be_bytes());
    Uuid::from_bytes(bytes)
}

fn uuid_from_i32(id: i32) -> Uuid {
    let mut bytes = [0u8; 16];
    bytes[12..16].copy_from_slice(&(id as u32).to_be_bytes());
    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use std::str::FromStr;

    /// Test InventorySnapshot structure
    #[test]
    fn test_inventory_snapshot_creation() {
        let snapshot = InventorySnapshot {
            inventory_item_id: 123,
            item_number: "ITEM-001".to_string(),
            description: Some("Test Item".to_string()),
            primary_uom_code: Some("EA".to_string()),
            organization_id: 1,
            total_on_hand: Decimal::from_str("100.00").unwrap(),
            total_allocated: Decimal::from_str("20.00").unwrap(),
            total_available: Decimal::from_str("80.00").unwrap(),
            locations: vec![],
        };

        assert_eq!(snapshot.inventory_item_id, 123);
        assert_eq!(snapshot.item_number, "ITEM-001");
        assert_eq!(snapshot.total_on_hand, Decimal::from_str("100.00").unwrap());
        assert_eq!(
            snapshot.total_allocated,
            Decimal::from_str("20.00").unwrap()
        );
        assert_eq!(
            snapshot.total_available,
            Decimal::from_str("80.00").unwrap()
        );
    }

    /// Test LocationBalance structure
    #[test]
    fn test_location_balance_creation() {
        let now = Utc::now();
        let balance = LocationBalance {
            location_id: 456,
            location_name: Some("Warehouse A".to_string()),
            quantity_on_hand: Decimal::from_str("50.00").unwrap(),
            quantity_allocated: Decimal::from_str("10.00").unwrap(),
            quantity_available: Decimal::from_str("40.00").unwrap(),
            updated_at: now,
            version: 1,
        };

        assert_eq!(balance.location_id, 456);
        assert_eq!(balance.location_name, Some("Warehouse A".to_string()));
        assert_eq!(
            balance.quantity_on_hand,
            Decimal::from_str("50.00").unwrap()
        );
        assert_eq!(
            balance.quantity_allocated,
            Decimal::from_str("10.00").unwrap()
        );
        assert_eq!(
            balance.quantity_available,
            Decimal::from_str("40.00").unwrap()
        );
        assert_eq!(balance.version, 1);
    }

    /// Test AdjustInventoryCommand structure
    #[test]
    fn test_adjust_inventory_command() {
        let command = AdjustInventoryCommand {
            inventory_item_id: Some(100),
            item_number: Some("SKU-100".to_string()),
            location_id: 5,
            quantity_delta: Decimal::from_str("25.00").unwrap(),
            reason: Some("Receiving".to_string()),
            expected_version: Some(3),
        };

        assert_eq!(command.inventory_item_id, Some(100));
        assert_eq!(command.item_number, Some("SKU-100".to_string()));
        assert_eq!(command.location_id, 5);
        assert_eq!(command.quantity_delta, Decimal::from_str("25.00").unwrap());
        assert_eq!(command.reason, Some("Receiving".to_string()));
        assert_eq!(command.expected_version, Some(3));
    }

    /// Test ReserveInventoryCommand structure
    #[test]
    fn test_reserve_inventory_command() {
        let ref_id = Uuid::new_v4();
        let command = ReserveInventoryCommand {
            inventory_item_id: Some(200),
            item_number: Some("SKU-200".to_string()),
            location_id: 10,
            quantity: Decimal::from_str("15.00").unwrap(),
            reference_id: Some(ref_id),
            reference_type: Some("ORDER".to_string()),
            expected_version: Some(5),
        };

        assert_eq!(command.inventory_item_id, Some(200));
        assert_eq!(command.quantity, Decimal::from_str("15.00").unwrap());
        assert_eq!(command.reference_id, Some(ref_id));
        assert_eq!(command.reference_type, Some("ORDER".to_string()));
        assert_eq!(command.expected_version, Some(5));
    }

    /// Test ReservationOutcome structure and id_str method
    #[test]
    fn test_reservation_outcome() {
        let res_id = Uuid::new_v4();
        let now = Utc::now();
        let balance = LocationBalance {
            location_id: 1,
            location_name: Some("Main Warehouse".to_string()),
            quantity_on_hand: Decimal::from_str("100.00").unwrap(),
            quantity_allocated: Decimal::from_str("30.00").unwrap(),
            quantity_available: Decimal::from_str("70.00").unwrap(),
            updated_at: now,
            version: 2,
        };

        let outcome = ReservationOutcome {
            reservation_id: res_id,
            balance,
        };

        assert_eq!(outcome.reservation_id, res_id);
        assert_eq!(outcome.id_str(), res_id.to_string());
        assert_eq!(
            outcome.balance.quantity_on_hand,
            Decimal::from_str("100.00").unwrap()
        );
        assert_eq!(outcome.balance.version, 2);
    }

    /// Test ReleaseReservationCommand structure
    #[test]
    fn test_release_reservation_command() {
        let command = ReleaseReservationCommand {
            inventory_item_id: Some(300),
            item_number: Some("SKU-300".to_string()),
            location_id: 15,
            quantity: Decimal::from_str("5.00").unwrap(),
        };

        assert_eq!(command.inventory_item_id, Some(300));
        assert_eq!(command.item_number, Some("SKU-300".to_string()));
        assert_eq!(command.location_id, 15);
        assert_eq!(command.quantity, Decimal::from_str("5.00").unwrap());
    }

    /// Test decimal_to_i32 conversion - positive value
    #[test]
    fn test_decimal_to_i32_positive() {
        let decimal = Decimal::from_str("42.7").unwrap();
        let result = decimal_to_i32(decimal);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 43); // Rounds to 43
    }

    /// Test decimal_to_i32 conversion - zero
    #[test]
    fn test_decimal_to_i32_zero() {
        let decimal = Decimal::ZERO;
        let result = decimal_to_i32(decimal);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    /// Test decimal_to_i32 conversion - negative value
    #[test]
    fn test_decimal_to_i32_negative() {
        let decimal = Decimal::from_str("-10.3").unwrap();
        let result = decimal_to_i32(decimal);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), -10); // Rounds to -10
    }

    /// Test uuid_from_i64 conversion
    #[test]
    fn test_uuid_from_i64() {
        let id: i64 = 123456789;
        let uuid = uuid_from_i64(id);

        // UUID should be deterministic for the same input
        assert_eq!(uuid, uuid_from_i64(id));

        // Different inputs should produce different UUIDs
        let uuid2 = uuid_from_i64(987654321);
        assert_ne!(uuid, uuid2);
    }

    /// Test uuid_from_i32 conversion
    #[test]
    fn test_uuid_from_i32() {
        let id: i32 = 42;
        let uuid = uuid_from_i32(id);

        // UUID should be deterministic for the same input
        assert_eq!(uuid, uuid_from_i32(id));

        // Different inputs should produce different UUIDs
        let uuid2 = uuid_from_i32(99);
        assert_ne!(uuid, uuid2);
    }

    /// Test uuid_from_i64 and uuid_from_i32 work independently
    #[test]
    fn test_uuid_conversion_functions_work() {
        let id_i64: i64 = 100;
        let id_i32: i32 = 200;

        let uuid_i64 = uuid_from_i64(id_i64);
        let uuid_i32 = uuid_from_i32(id_i32);

        // Both should produce valid UUIDs
        assert_ne!(uuid_i64, Uuid::nil());
        assert_ne!(uuid_i32, Uuid::nil());

        // Different input values produce different UUIDs within same function
        assert_ne!(uuid_from_i64(100), uuid_from_i64(200));
        assert_ne!(uuid_from_i32(100), uuid_from_i32(200));
    }

    /// Test inventory snapshot with multiple locations
    #[test]
    fn test_inventory_snapshot_with_locations() {
        let now = Utc::now();
        let location1 = LocationBalance {
            location_id: 1,
            location_name: Some("Warehouse A".to_string()),
            quantity_on_hand: Decimal::from_str("60.00").unwrap(),
            quantity_allocated: Decimal::from_str("10.00").unwrap(),
            quantity_available: Decimal::from_str("50.00").unwrap(),
            updated_at: now,
            version: 1,
        };

        let location2 = LocationBalance {
            location_id: 2,
            location_name: Some("Warehouse B".to_string()),
            quantity_on_hand: Decimal::from_str("40.00").unwrap(),
            quantity_allocated: Decimal::from_str("10.00").unwrap(),
            quantity_available: Decimal::from_str("30.00").unwrap(),
            updated_at: now,
            version: 1,
        };

        let snapshot = InventorySnapshot {
            inventory_item_id: 500,
            item_number: "MULTI-LOC-001".to_string(),
            description: Some("Multi-location item".to_string()),
            primary_uom_code: Some("EA".to_string()),
            organization_id: 1,
            total_on_hand: Decimal::from_str("100.00").unwrap(),
            total_allocated: Decimal::from_str("20.00").unwrap(),
            total_available: Decimal::from_str("80.00").unwrap(),
            locations: vec![location1, location2],
        };

        assert_eq!(snapshot.locations.len(), 2);
        assert_eq!(snapshot.locations[0].location_id, 1);
        assert_eq!(snapshot.locations[1].location_id, 2);
    }

    /// Test clone trait for InventorySnapshot
    #[test]
    fn test_inventory_snapshot_clone() {
        let original = InventorySnapshot {
            inventory_item_id: 999,
            item_number: "CLONE-TEST".to_string(),
            description: Some("Clone test item".to_string()),
            primary_uom_code: Some("EA".to_string()),
            organization_id: 1,
            total_on_hand: Decimal::from_str("50.00").unwrap(),
            total_allocated: Decimal::from_str("5.00").unwrap(),
            total_available: Decimal::from_str("45.00").unwrap(),
            locations: vec![],
        };

        let cloned = original.clone();
        assert_eq!(original.inventory_item_id, cloned.inventory_item_id);
        assert_eq!(original.item_number, cloned.item_number);
        assert_eq!(original.total_on_hand, cloned.total_on_hand);
    }
}
