use std::sync::Arc;

use chrono::{DateTime, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
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
}

/// Command payload for adjusting inventory.
#[derive(Debug, Clone)]
pub struct AdjustInventoryCommand {
    pub inventory_item_id: Option<i64>,
    pub item_number: Option<String>,
    pub location_id: i32,
    pub quantity_delta: Decimal,
    pub reason: Option<String>,
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
#[derive(Clone)]
pub struct InventoryService {
    db_pool: Arc<DatabaseConnection>,
    event_sender: EventSender,
}

impl InventoryService {
    pub fn new(db_pool: Arc<DatabaseConnection>, event_sender: EventSender) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Returns a paginated list of inventory snapshots.
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

        let mut snapshots = Vec::with_capacity(models.len());
        for item in models {
            snapshots.push(self.snapshot_for_item(&item).await?);
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

        let (old_on_hand, updated_balance) = db
            .transaction::<_, (Decimal, inventory_balance::Model), ServiceError>(|txn| {
                let reason_clone = reason.clone();
                Box::pin(async move {
                    let now = Utc::now();
                    let maybe_balance = InventoryBalanceEntity::find()
                        .filter(
                            inventory_balance::Column::InventoryItemId.eq(item.inventory_item_id),
                        )
                        .filter(inventory_balance::Column::LocationId.eq(location_id))
                        .one(txn)
                        .await
                        .map_err(ServiceError::db_error)?;

                    if let Some(existing) = maybe_balance {
                        let new_on_hand = existing.quantity_on_hand + delta;
                        let new_available = existing.quantity_available + delta;
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
                            quantity_available: Set(delta),
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
                    let existing = InventoryBalanceEntity::find()
                        .filter(
                            inventory_balance::Column::InventoryItemId.eq(item.inventory_item_id),
                        )
                        .filter(inventory_balance::Column::LocationId.eq(location_id))
                        .one(txn)
                        .await
                        .map_err(ServiceError::db_error)?
                        .ok_or_else(|| {
                            ServiceError::NotFound(format!(
                                "No inventory for item {} at location {}",
                                item.inventory_item_id, location_id
                            ))
                        })?;

                    if existing.quantity_available < quantity {
                        return Err(ServiceError::ValidationError(format!(
                            "Insufficient available quantity. Requested: {}, Available: {}",
                            quantity, existing.quantity_available
                        )));
                    }

                    let mut active: inventory_balance::ActiveModel = existing.clone().into();
                    active.quantity_allocated = Set(existing.quantity_allocated + quantity);
                    active.quantity_available = Set(existing.quantity_available - quantity);
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
                    let existing = InventoryBalanceEntity::find()
                        .filter(
                            inventory_balance::Column::InventoryItemId.eq(item.inventory_item_id),
                        )
                        .filter(inventory_balance::Column::LocationId.eq(location_id))
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
                            "Cannot release {} when only {} is allocated",
                            quantity, existing.quantity_allocated
                        )));
                    }

                    let mut active: inventory_balance::ActiveModel = existing.clone().into();
                    active.quantity_allocated = Set(existing.quantity_allocated - quantity);
                    active.quantity_available = Set(existing.quantity_available + quantity);
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

                let source = InventoryBalanceEntity::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(inventory_item_id))
                    .filter(inventory_balance::Column::LocationId.eq(from_location))
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
                        "Insufficient stock at source. Requested: {}, Available: {}",
                        quantity, source.quantity_available
                    )));
                }

                let mut source_active: inventory_balance::ActiveModel = source.clone().into();
                source_active.quantity_on_hand = Set(source.quantity_on_hand - quantity);
                source_active.quantity_available = Set(source.quantity_available - quantity);
                source_active.updated_at = Set(now.into());
                source_active
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                let dest = InventoryBalanceEntity::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(inventory_item_id))
                    .filter(inventory_balance::Column::LocationId.eq(to_location))
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                if let Some(existing) = dest {
                    let mut active: inventory_balance::ActiveModel = existing.clone().into();
                    active.quantity_on_hand = Set(existing.quantity_on_hand + quantity);
                    active.quantity_available = Set(existing.quantity_available + quantity);
                    active.updated_at = Set(now.into());
                    active.update(txn).await.map_err(ServiceError::db_error)?;
                } else {
                    inventory_balance::ActiveModel {
                        inventory_item_id: Set(inventory_item_id),
                        location_id: Set(to_location),
                        quantity_on_hand: Set(quantity),
                        quantity_allocated: Set(Decimal::ZERO),
                        quantity_available: Set(quantity),
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
