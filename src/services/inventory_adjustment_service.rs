use crate::{
    db::DbPool,
    entities::{
        inventory_balance::{self, Entity as InventoryBalance},
        inventory_transaction::{self, TransactionType},
        purchase_order_lines::{self, Entity as PurchaseOrderLine},
        sales_order_line::{self, Entity as SalesOrderLine},
    },
    errors::ServiceError,
    events::{Event, EventSender},
};
use chrono::Utc;
use rust_decimal::{prelude::ToPrimitive, Decimal};
use sea_orm::{Condition, Set, TransactionTrait, *};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tracing::info;
use uuid::Uuid;

pub struct InventoryAdjustmentService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
}

impl InventoryAdjustmentService {
    pub fn new(db_pool: Arc<DbPool>, event_sender: Arc<EventSender>) -> Self {
        Self {
            db_pool,
            event_sender,
        }
    }

    /// Adjust inventory when a sales order is created or confirmed
    /// OPTIMIZED: Uses batch operations to avoid N+1 queries
    pub async fn adjust_for_sales_order(
        &self,
        order_id: i64,
        adjustment_type: SalesOrderAdjustmentType,
    ) -> Result<Vec<InventoryAdjustmentResult>, ServiceError> {
        let db = self.db_pool.as_ref();

        // Fetch all order lines in one query
        let order_lines = SalesOrderLine::find()
            .filter(sales_order_line::Column::HeaderId.eq(order_id))
            .all(db)
            .await
            .map_err(ServiceError::db_error)?;

        if order_lines.is_empty() {
            return Err(ServiceError::NotFound(format!(
                "No order lines found for order {}",
                order_id
            )));
        }

        // OPTIMIZATION: Use batch processing instead of loop
        let results = match adjustment_type {
            SalesOrderAdjustmentType::Allocate => {
                self.allocate_inventory_batch(db, &order_lines).await?
            }
            SalesOrderAdjustmentType::Ship => self.ship_inventory_batch(db, &order_lines).await?,
            SalesOrderAdjustmentType::Cancel => {
                self.deallocate_inventory_batch(db, &order_lines).await?
            }
            SalesOrderAdjustmentType::Return => {
                self.return_inventory_batch(db, &order_lines).await?
            }
        };

        // Send event for inventory adjustment
        self.event_sender
            .send(Event::InventoryAdjustedForOrder {
                order_id,
                adjustment_type: adjustment_type.to_string(),
            })
            .await
            .map_err(|e| ServiceError::EventError(format!("Failed to send event: {}", e)))?;

        Ok(results)
    }

    /// OPTIMIZED: Batch allocate inventory for multiple order lines
    /// Eliminates N+1 queries by fetching all inventory in one query
    async fn allocate_inventory_batch(
        &self,
        db: &DatabaseConnection,
        order_lines: &[sales_order_line::Model],
    ) -> Result<Vec<InventoryAdjustmentResult>, ServiceError> {
        // Clone order_lines to move into the transaction closure
        let order_lines = order_lines.to_vec();

        // Use a single transaction for all operations
        let result = db
            .transaction::<_, Vec<InventoryAdjustmentResult>, ServiceError>(|txn| {
                Box::pin(async move {
                    // Collect all (item_id, location_id) pairs from order lines.
                    let inventory_keys: Vec<(i64, i32)> = order_lines
                        .iter()
                        .filter_map(|line| match (line.inventory_item_id, line.location_id) {
                            (Some(item_id), Some(location_id)) => Some((item_id, location_id)),
                            _ => None,
                        })
                        .collect();

                    if inventory_keys.is_empty() {
                        return Err(ServiceError::ValidationError(
                            "Order lines missing inventory_item_id or location_id".to_string(),
                        ));
                    }

                    // Deduplicate keys and build OR condition (SeaORM filters are AND by default).
                    let mut unique_set: HashSet<(i64, i32)> = HashSet::new();
                    let unique_keys: Vec<(i64, i32)> = inventory_keys
                        .into_iter()
                        .filter(|k| unique_set.insert(*k))
                        .collect();

                    let mut condition = Condition::any();
                    for (item_id, location_id) in &unique_keys {
                        condition = condition.add(
                            inventory_balance::Column::InventoryItemId
                                .eq(*item_id)
                                .and(inventory_balance::Column::LocationId.eq(*location_id)),
                        );
                    }

                    // Fetch all needed inventory balances in ONE query and lock them.
                    let inventories = InventoryBalance::find()
                        .filter(condition)
                        .lock_exclusive()
                        .all(txn)
                        .await
                        .map_err(ServiceError::db_error)?;

                    let mut inventory_map: HashMap<(i64, i32), inventory_balance::Model> =
                        HashMap::with_capacity(inventories.len());
                    for inv in inventories {
                        inventory_map.insert((inv.inventory_item_id, inv.location_id), inv);
                    }

                    let mut results = Vec::new();
                    let mut transactions = Vec::new();
                    let mut modified_keys: HashSet<(i64, i32)> = HashSet::new();
                    let mut version_bumps: HashMap<(i64, i32), i32> = HashMap::new();

                    // Process all order lines, updating in-memory balances so multiple lines
                    // for the same (item, location) are handled correctly.
                    for order_line in order_lines {
                        let (item_id, location_id) =
                            match (order_line.inventory_item_id, order_line.location_id) {
                                (Some(item), Some(loc)) => (item, loc),
                                _ => continue,
                            };

                        let required_qty = order_line.ordered_quantity.unwrap_or(Decimal::ZERO);
                        if required_qty <= Decimal::ZERO {
                            continue;
                        }

                        let key = (item_id, location_id);
                        let inventory = inventory_map.get_mut(&key).ok_or_else(|| {
                            ServiceError::NotFound(format!(
                                "Inventory not found for item {} at location {}",
                                item_id, location_id
                            ))
                        })?;

                        if inventory.quantity_available < required_qty {
                            return Err(ServiceError::InvalidOperation(format!(
                                "Insufficient inventory for item {} at location {}. Available: {}, Required: {}",
                                item_id, location_id, inventory.quantity_available, required_qty
                            )));
                        }

                        let previous_on_hand = inventory.quantity_on_hand;
                        inventory.quantity_allocated += required_qty;
                        inventory.quantity_available =
                            inventory.quantity_on_hand - inventory.quantity_allocated;
                        modified_keys.insert(key);
                        *version_bumps.entry(key).or_insert(0) += 1;

                        transactions.push(inventory_transaction::ActiveModel {
                            id: Set(Uuid::new_v4()),
                            product_id: Set(uuid_from_i64(item_id)),
                            location_id: Set(uuid_from_i32(location_id)),
                            r#type: Set(TransactionType::Allocate.as_str().to_string()),
                            quantity: Set(decimal_to_i32(required_qty)?),
                            previous_quantity: Set(decimal_to_i32(previous_on_hand)?),
                            new_quantity: Set(decimal_to_i32(previous_on_hand)?), // allocate doesn't change on-hand
                            reference_id: Set(order_line.header_id.map(uuid_from_i64)),
                            reference_type: Set(Some("SALES_ORDER".to_string())),
                            reason: Set(Some("Order allocation".to_string())),
                            notes: Set(Some(format!(
                                "Allocated for order line {}",
                                order_line.line_id
                            ))),
                            created_by: Set(Uuid::nil()),
                            created_at: Set(Utc::now()),
                        });

                        results.push(InventoryAdjustmentResult {
                            item_id,
                            location_id,
                            adjustment_type: TransactionType::Allocate,
                            quantity_adjusted: required_qty,
                            new_on_hand: inventory.quantity_on_hand,
                            new_available: inventory.quantity_available,
                            new_allocated: inventory.quantity_allocated,
                        });
                    }

                    // Persist final balances once per modified key.
                    let now = Utc::now();
                    for key in modified_keys {
                        if let Some(inv) = inventory_map.get(&key) {
                            let mut active: inventory_balance::ActiveModel = inv.clone().into();
                            if let Some(bump) = version_bumps.get(&key) {
                                active.version = Set(inv.version + *bump);
                            }
                            active.updated_at = Set(now.into());
                            active.update(txn).await.map_err(ServiceError::db_error)?;
                        }
                    }

                    if !transactions.is_empty() {
                        inventory_transaction::Entity::insert_many(transactions)
                            .exec(txn)
                            .await
                            .map_err(ServiceError::db_error)?;
                    }

                    Ok(results)
                })
            })
            .await;

        result.map_err(|e| match e {
            sea_orm::TransactionError::Connection(err) => ServiceError::DatabaseError(err),
            sea_orm::TransactionError::Transaction(service_err) => service_err,
        })
    }

    /// OPTIMIZED: Batch ship inventory (placeholder for similar pattern)
    async fn ship_inventory_batch(
        &self,
        db: &DatabaseConnection,
        order_lines: &[sales_order_line::Model],
    ) -> Result<Vec<InventoryAdjustmentResult>, ServiceError> {
        // Similar batch implementation for shipping
        // For now, fall back to individual processing
        let mut results = Vec::new();
        for line in order_lines {
            results.push(self.ship_inventory_for_order_line(db, line).await?);
        }
        Ok(results)
    }

    /// OPTIMIZED: Batch deallocate inventory (placeholder for similar pattern)
    async fn deallocate_inventory_batch(
        &self,
        db: &DatabaseConnection,
        order_lines: &[sales_order_line::Model],
    ) -> Result<Vec<InventoryAdjustmentResult>, ServiceError> {
        // Similar batch implementation for deallocation
        // For now, fall back to individual processing
        let mut results = Vec::new();
        for line in order_lines {
            results.push(self.deallocate_inventory_for_order_line(db, line).await?);
        }
        Ok(results)
    }

    /// OPTIMIZED: Batch return inventory (placeholder for similar pattern)
    async fn return_inventory_batch(
        &self,
        db: &DatabaseConnection,
        order_lines: &[sales_order_line::Model],
    ) -> Result<Vec<InventoryAdjustmentResult>, ServiceError> {
        // Similar batch implementation for returns
        // For now, fall back to individual processing
        let mut results = Vec::new();
        for line in order_lines {
            results.push(self.return_inventory_for_order_line(db, line).await?);
        }
        Ok(results)
    }

    /// Adjust inventory when a purchase order is received
    pub async fn adjust_for_purchase_order_receipt(
        &self,
        po_id: i64,
        receipt_lines: Vec<PurchaseOrderReceiptLine>,
    ) -> Result<Vec<InventoryAdjustmentResult>, ServiceError> {
        let db = self.db_pool.as_ref();
        let mut results = Vec::new();

        for receipt_line in receipt_lines {
            let po_line = PurchaseOrderLine::find()
                .filter(purchase_order_lines::Column::PoLineId.eq(receipt_line.po_line_id))
                .one(db)
                .await
                .map_err(ServiceError::db_error)?
                .ok_or_else(|| {
                    ServiceError::NotFound(format!("PO line {} not found", receipt_line.po_line_id))
                })?;

            let result = self
                .receive_inventory_for_po_line(
                    db,
                    &po_line,
                    receipt_line.quantity_received,
                    receipt_line.location_id,
                )
                .await?;

            results.push(result);
        }

        // Send event for PO receipt
        self.event_sender
            .send(Event::InventoryReceivedFromPO { po_id })
            .await
            .map_err(|e| ServiceError::EventError(format!("Failed to send event: {}", e)))?;

        Ok(results)
    }

    /// Allocate inventory for a sales order line
    async fn allocate_inventory_for_order_line(
        &self,
        db: &DatabaseConnection,
        order_line: &sales_order_line::Model,
    ) -> Result<InventoryAdjustmentResult, ServiceError> {
        let order_line = order_line.clone();
        db.transaction::<_, InventoryAdjustmentResult, ServiceError>(move |txn| {
            Box::pin(async move {
                let item_id = order_line.inventory_item_id.ok_or_else(|| {
                    ServiceError::ValidationError(
                        "Order line missing inventory_item_id".to_string(),
                    )
                })?;
                let location_id = order_line.location_id.ok_or_else(|| {
                    ServiceError::ValidationError("Order line missing location_id".to_string())
                })?;

                let inventory = InventoryBalance::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
                    .filter(inventory_balance::Column::LocationId.eq(location_id))
                    .lock_exclusive()
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {} at location {}",
                            item_id, location_id
                        ))
                    })?;

                let required_qty = order_line.ordered_quantity.unwrap_or(Decimal::ZERO);
                if required_qty <= Decimal::ZERO {
                    return Err(ServiceError::ValidationError(
                        "Ordered quantity must be positive".to_string(),
                    ));
                }

                let new_allocated = inventory.quantity_allocated + required_qty;
                let new_available = inventory.quantity_on_hand - new_allocated;
                if new_available < Decimal::ZERO {
                    return Err(ServiceError::InvalidOperation(format!(
                        "Insufficient inventory for item {} at location {}. Available: {}, Required: {}",
                        item_id, location_id, inventory.quantity_available, required_qty
                    )));
                }

                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_allocated = Set(new_allocated);
                active_inventory.quantity_available = Set(new_available);
                active_inventory.version = Set(inventory.version + 1);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(uuid_from_i64(item_id)),
                    location_id: Set(uuid_from_i32(location_id)),
                    r#type: Set(TransactionType::Allocate.as_str().to_string()),
                    quantity: Set(decimal_to_i32(required_qty)?),
                    previous_quantity: Set(decimal_to_i32(inventory.quantity_on_hand)?),
                    new_quantity: Set(decimal_to_i32(inventory.quantity_on_hand)?),
                    reference_id: Set(order_line.header_id.map(uuid_from_i64)),
                    reference_type: Set(Some("SALES_ORDER".to_string())),
                    reason: Set(Some("Order allocation".to_string())),
                    notes: Set(Some(format!(
                        "Allocated for order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::nil()),
                    created_at: Set(Utc::now()),
                }
                .insert(txn)
                .await
                .map_err(ServiceError::db_error)?;

                info!(
                    "Allocated {} units of item {} for order line {}",
                    required_qty, item_id, order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id,
                    location_id,
                    adjustment_type: TransactionType::Allocate,
                    quantity_adjusted: required_qty,
                    new_on_hand: updated_inventory.quantity_on_hand,
                    new_available: updated_inventory.quantity_available,
                    new_allocated: updated_inventory.quantity_allocated,
                })
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
    }

    /// Ship inventory for a sales order line
    async fn ship_inventory_for_order_line(
        &self,
        db: &DatabaseConnection,
        order_line: &sales_order_line::Model,
    ) -> Result<InventoryAdjustmentResult, ServiceError> {
        let order_line = order_line.clone();
        db.transaction::<_, InventoryAdjustmentResult, ServiceError>(move |txn| {
            Box::pin(async move {
                let item_id = order_line.inventory_item_id.ok_or_else(|| {
                    ServiceError::ValidationError(
                        "Order line missing inventory_item_id".to_string(),
                    )
                })?;
                let location_id = order_line.location_id.ok_or_else(|| {
                    ServiceError::ValidationError("Order line missing location_id".to_string())
                })?;

                let inventory = InventoryBalance::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
                    .filter(inventory_balance::Column::LocationId.eq(location_id))
                    .lock_exclusive()
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {} at location {}",
                            item_id, location_id
                        ))
                    })?;

                let ship_qty = order_line.ordered_quantity.unwrap_or(Decimal::ZERO);
                if ship_qty <= Decimal::ZERO {
                    return Err(ServiceError::ValidationError(
                        "Ship quantity must be positive".to_string(),
                    ));
                }
                if inventory.quantity_allocated < ship_qty {
                    return Err(ServiceError::InvalidOperation(format!(
                        "Cannot ship {} units when only {} are allocated for item {} at location {}",
                        ship_qty, inventory.quantity_allocated, item_id, location_id
                    )));
                }
                if inventory.quantity_on_hand < ship_qty {
                    return Err(ServiceError::InvalidOperation(format!(
                        "Cannot ship {} units when only {} are on-hand for item {} at location {}",
                        ship_qty, inventory.quantity_on_hand, item_id, location_id
                    )));
                }

                let new_on_hand = inventory.quantity_on_hand - ship_qty;
                let new_allocated = inventory.quantity_allocated - ship_qty;
                let new_available = new_on_hand - new_allocated;

                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_on_hand = Set(new_on_hand);
                active_inventory.quantity_allocated = Set(new_allocated);
                active_inventory.quantity_available = Set(new_available);
                active_inventory.version = Set(inventory.version + 1);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(uuid_from_i64(item_id)),
                    location_id: Set(uuid_from_i32(location_id)),
                    r#type: Set(TransactionType::Ship.as_str().to_string()),
                    quantity: Set(decimal_to_i32(ship_qty)?),
                    previous_quantity: Set(decimal_to_i32(inventory.quantity_on_hand)?),
                    new_quantity: Set(decimal_to_i32(new_on_hand)?),
                    reference_id: Set(order_line.header_id.map(uuid_from_i64)),
                    reference_type: Set(Some("SALES_ORDER_SHIPMENT".to_string())),
                    reason: Set(Some("Order shipment".to_string())),
                    notes: Set(Some(format!(
                        "Shipped for order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::nil()),
                    created_at: Set(Utc::now()),
                }
                .insert(txn)
                .await
                .map_err(ServiceError::db_error)?;

                info!(
                    "Shipped {} units of item {} for order line {}",
                    ship_qty,
                    item_id,
                    order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id,
                    location_id,
                    adjustment_type: TransactionType::Ship,
                    quantity_adjusted: ship_qty,
                    new_on_hand: updated_inventory.quantity_on_hand,
                    new_available: updated_inventory.quantity_available,
                    new_allocated: updated_inventory.quantity_allocated,
                })
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
    }

    /// Deallocate inventory for a cancelled order line
    async fn deallocate_inventory_for_order_line(
        &self,
        db: &DatabaseConnection,
        order_line: &sales_order_line::Model,
    ) -> Result<InventoryAdjustmentResult, ServiceError> {
        let order_line = order_line.clone();
        db.transaction::<_, InventoryAdjustmentResult, ServiceError>(move |txn| {
            Box::pin(async move {
                let item_id = order_line.inventory_item_id.ok_or_else(|| {
                    ServiceError::ValidationError(
                        "Order line missing inventory_item_id".to_string(),
                    )
                })?;
                let location_id = order_line.location_id.ok_or_else(|| {
                    ServiceError::ValidationError("Order line missing location_id".to_string())
                })?;

                let inventory = InventoryBalance::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
                    .filter(inventory_balance::Column::LocationId.eq(location_id))
                    .lock_exclusive()
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {} at location {}",
                            item_id, location_id
                        ))
                    })?;

                let deallocate_qty = order_line
                    .ordered_quantity
                    .unwrap_or_else(|| Decimal::from(0));
                if deallocate_qty <= Decimal::ZERO {
                    return Err(ServiceError::ValidationError(
                        "Deallocate quantity must be positive".to_string(),
                    ));
                }
                if inventory.quantity_allocated < deallocate_qty {
                    return Err(ServiceError::InvalidOperation(format!(
                        "Cannot deallocate {} units when only {} are allocated for item {} at location {}",
                        deallocate_qty, inventory.quantity_allocated, item_id, location_id
                    )));
                }

                let new_allocated = inventory.quantity_allocated - deallocate_qty;
                let new_available = inventory.quantity_on_hand - new_allocated;

                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_allocated = Set(new_allocated);
                active_inventory.quantity_available = Set(new_available);
                active_inventory.version = Set(inventory.version + 1);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(uuid_from_i64(item_id)),
                    location_id: Set(uuid_from_i32(location_id)),
                    r#type: Set(TransactionType::Deallocate.as_str().to_string()),
                    quantity: Set(decimal_to_i32(deallocate_qty)?),
                    previous_quantity: Set(decimal_to_i32(inventory.quantity_on_hand)?),
                    new_quantity: Set(decimal_to_i32(inventory.quantity_on_hand)?),
                    reference_id: Set(order_line.header_id.map(uuid_from_i64)),
                    reference_type: Set(Some("SALES_ORDER_CANCEL".to_string())),
                    reason: Set(Some("Order cancellation".to_string())),
                    notes: Set(Some(format!(
                        "Deallocated for cancelled order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::nil()),
                    created_at: Set(Utc::now()),
                }
                .insert(txn)
                .await
                .map_err(ServiceError::db_error)?;

                info!(
                    "Deallocated {} units of item {} for cancelled order line {}",
                    deallocate_qty,
                    item_id,
                    order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id,
                    location_id,
                    adjustment_type: TransactionType::Deallocate,
                    quantity_adjusted: deallocate_qty,
                    new_on_hand: updated_inventory.quantity_on_hand,
                    new_available: updated_inventory.quantity_available,
                    new_allocated: updated_inventory.quantity_allocated,
                })
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
    }

    /// Return inventory for a returned order line
    async fn return_inventory_for_order_line(
        &self,
        db: &DatabaseConnection,
        order_line: &sales_order_line::Model,
    ) -> Result<InventoryAdjustmentResult, ServiceError> {
        let order_line = order_line.clone();
        db.transaction::<_, InventoryAdjustmentResult, ServiceError>(move |txn| {
            Box::pin(async move {
                let item_id = order_line.inventory_item_id.ok_or_else(|| {
                    ServiceError::ValidationError(
                        "Order line missing inventory_item_id".to_string(),
                    )
                })?;
                let location_id = order_line.location_id.ok_or_else(|| {
                    ServiceError::ValidationError("Order line missing location_id".to_string())
                })?;

                let inventory = InventoryBalance::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(item_id))
                    .filter(inventory_balance::Column::LocationId.eq(location_id))
                    .lock_exclusive()
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {} at location {}",
                            item_id, location_id
                        ))
                    })?;

                let return_qty = order_line.ordered_quantity.unwrap_or(Decimal::ZERO);
                if return_qty <= Decimal::ZERO {
                    return Err(ServiceError::ValidationError(
                        "Return quantity must be positive".to_string(),
                    ));
                }

                let new_on_hand = inventory.quantity_on_hand + return_qty;
                let new_available = new_on_hand - inventory.quantity_allocated;

                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_on_hand = Set(new_on_hand);
                active_inventory.quantity_available = Set(new_available);
                active_inventory.version = Set(inventory.version + 1);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(uuid_from_i64(item_id)),
                    location_id: Set(uuid_from_i32(location_id)),
                    r#type: Set(TransactionType::Return.as_str().to_string()),
                    quantity: Set(decimal_to_i32(return_qty)?),
                    previous_quantity: Set(decimal_to_i32(inventory.quantity_on_hand)?),
                    new_quantity: Set(decimal_to_i32(new_on_hand)?),
                    reference_id: Set(order_line.header_id.map(uuid_from_i64)),
                    reference_type: Set(Some("SALES_ORDER_RETURN".to_string())),
                    reason: Set(Some("Order return".to_string())),
                    notes: Set(Some(format!(
                        "Returned from order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::nil()),
                    created_at: Set(Utc::now()),
                }
                .insert(txn)
                .await
                .map_err(ServiceError::db_error)?;

                info!(
                    "Returned {} units of item {} from order line {}",
                    return_qty, item_id, order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id,
                    location_id,
                    adjustment_type: TransactionType::Return,
                    quantity_adjusted: return_qty,
                    new_on_hand: updated_inventory.quantity_on_hand,
                    new_available: updated_inventory.quantity_available,
                    new_allocated: updated_inventory.quantity_allocated,
                })
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
    }

    /// Receive inventory from a purchase order line
    async fn receive_inventory_for_po_line(
        &self,
        db: &DatabaseConnection,
        po_line: &purchase_order_lines::Model,
        quantity_received: Decimal,
        location_id: i32,
    ) -> Result<InventoryAdjustmentResult, ServiceError> {
        let po_line = po_line.clone();
        db.transaction::<_, InventoryAdjustmentResult, ServiceError>(move |txn| {
            Box::pin(async move {
                // Find or create inventory balance
                let inventory = InventoryBalance::find()
                    .filter(inventory_balance::Column::InventoryItemId.eq(po_line.item_id))
                    .filter(inventory_balance::Column::LocationId.eq(location_id))
                    .lock_exclusive()
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                let (updated_inventory, was_created) = match inventory {
                    Some(inv) => {
                        if quantity_received <= Decimal::ZERO {
                            return Err(ServiceError::ValidationError(
                                "Received quantity must be positive".to_string(),
                            ));
                        }
                        // Update existing inventory
                        let mut active_inventory: inventory_balance::ActiveModel =
                            inv.clone().into();
                        let new_on_hand = inv.quantity_on_hand + quantity_received;
                        let new_available = new_on_hand - inv.quantity_allocated;
                        active_inventory.quantity_on_hand = Set(new_on_hand);
                        active_inventory.quantity_available = Set(new_available);
                        active_inventory.version = Set(inv.version + 1);
                        active_inventory.updated_at = Set(Utc::now().into());

                        let updated = active_inventory
                            .update(txn)
                            .await
                            .map_err(ServiceError::db_error)?;
                        (updated, false)
                    }
                    None => {
                        if quantity_received <= Decimal::ZERO {
                            return Err(ServiceError::ValidationError(
                                "Received quantity must be positive".to_string(),
                            ));
                        }
                        // Create new inventory balance
                        let new_inventory = inventory_balance::ActiveModel {
                            inventory_item_id: Set(po_line.item_id.unwrap_or(0)),
                            location_id: Set(location_id),
                            quantity_on_hand: Set(quantity_received),
                            quantity_allocated: Set(Decimal::ZERO),
                            quantity_available: Set(quantity_received),
                            version: Set(1),
                            created_at: Set(Utc::now().into()),
                            updated_at: Set(Utc::now().into()),
                            ..Default::default()
                        };

                        let created = new_inventory
                            .insert(txn)
                            .await
                            .map_err(ServiceError::db_error)?;
                        (created, true)
                    }
                };

                let ref_uuid = po_line
                    .po_header_id
                    .or(Some(po_line.po_line_id))
                    .map(uuid_from_i64);

                inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(uuid_from_i64(po_line.item_id.unwrap_or(0))),
                    location_id: Set(uuid_from_i32(location_id)),
                    r#type: Set(TransactionType::Receive.as_str().to_string()),
                    quantity: Set(decimal_to_i32(quantity_received)?),
                    previous_quantity: Set(if was_created {
                        0
                    } else {
                        decimal_to_i32(updated_inventory.quantity_on_hand - quantity_received)?
                    }),
                    new_quantity: Set(decimal_to_i32(updated_inventory.quantity_on_hand)?),
                    reference_id: Set(ref_uuid),
                    reference_type: Set(Some("PURCHASE_ORDER_RECEIPT".to_string())),
                    reason: Set(Some("PO receipt".to_string())),
                    notes: Set(Some(format!(
                        "Received from PO line {}",
                        po_line.po_line_id
                    ))),
                    created_by: Set(Uuid::nil()),
                    created_at: Set(Utc::now()),
                }
                .insert(txn)
                .await
                .map_err(ServiceError::db_error)?;

                info!(
                    "Received {} units of item {} from PO line {} into location {}",
                    quantity_received,
                    po_line.item_id.unwrap_or(0),
                    po_line.po_line_id,
                    location_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id: po_line.item_id.unwrap_or(0),
                    location_id,
                    adjustment_type: TransactionType::Receive,
                    quantity_adjusted: quantity_received,
                    new_on_hand: updated_inventory.quantity_on_hand,
                    new_available: updated_inventory.quantity_available,
                    new_allocated: updated_inventory.quantity_allocated,
                })
            })
        })
        .await
        .map_err(|e| match e {
            TransactionError::Connection(db_err) => ServiceError::db_error(db_err),
            TransactionError::Transaction(service_err) => service_err,
        })
    }
}

#[derive(Debug, Clone)]
pub enum SalesOrderAdjustmentType {
    Allocate,
    Ship,
    Cancel,
    Return,
}

impl SalesOrderAdjustmentType {
    pub fn to_string(&self) -> String {
        match self {
            Self::Allocate => "ALLOCATE".to_string(),
            Self::Ship => "SHIP".to_string(),
            Self::Cancel => "CANCEL".to_string(),
            Self::Return => "RETURN".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PurchaseOrderReceiptLine {
    pub po_line_id: i64,
    pub quantity_received: Decimal,
    pub location_id: i32,
}

#[derive(Debug, Clone)]
pub struct InventoryAdjustmentResult {
    pub item_id: i64,
    pub location_id: i32,
    pub adjustment_type: TransactionType,
    pub quantity_adjusted: Decimal,
    pub new_on_hand: Decimal,
    pub new_available: Decimal,
    pub new_allocated: Decimal,
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
