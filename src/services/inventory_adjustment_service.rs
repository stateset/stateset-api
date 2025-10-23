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
use rust_decimal::Decimal;
use sea_orm::{Set, TransactionTrait, *};
use std::sync::Arc;
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
    pub async fn adjust_for_sales_order(
        &self,
        order_id: i64,
        adjustment_type: SalesOrderAdjustmentType,
    ) -> Result<Vec<InventoryAdjustmentResult>, ServiceError> {
        let db = self.db_pool.as_ref();

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

        let mut results = Vec::new();

        for line in order_lines {
            let result = match adjustment_type {
                SalesOrderAdjustmentType::Allocate => {
                    self.allocate_inventory_for_order_line(db, &line).await?
                }
                SalesOrderAdjustmentType::Ship => {
                    self.ship_inventory_for_order_line(db, &line).await?
                }
                SalesOrderAdjustmentType::Cancel => {
                    self.deallocate_inventory_for_order_line(db, &line).await?
                }
                SalesOrderAdjustmentType::Return => {
                    self.return_inventory_for_order_line(db, &line).await?
                }
            };
            results.push(result);
        }

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
                // Find inventory balance for the item
                let inventory = InventoryBalance::find()
                    .filter(
                        inventory_balance::Column::InventoryItemId.eq(order_line.inventory_item_id),
                    )
                    .filter(inventory_balance::Column::LocationId.eq(order_line.location_id))
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {:?}",
                            order_line.inventory_item_id
                        ))
                    })?;

                // Check available quantity
                let required_qty = order_line.ordered_quantity.unwrap_or(Decimal::ZERO);
                if inventory.quantity_available < required_qty {
                    return Err(ServiceError::InvalidOperation(format!(
                        "Insufficient inventory. Available: {}, Required: {}",
                        inventory.quantity_available, required_qty
                    )));
                }

                // Update inventory balance
                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_allocated =
                    Set(inventory.quantity_allocated + required_qty);
                active_inventory.quantity_available =
                    Set(inventory.quantity_available - required_qty);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                // Create transaction record
                let transaction = inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(Uuid::new_v4()), // Would map from inventory_item_id in real implementation
                    location_id: Set(Uuid::new_v4()), // Would map from location_id
                    r#type: Set(TransactionType::Allocate.as_str().to_string()),
                    quantity: Set(required_qty.round().to_string().parse::<i32>().unwrap_or(0)),
                    previous_quantity: Set(inventory
                        .quantity_on_hand
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    new_quantity: Set(inventory
                        .quantity_on_hand
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    reference_id: Set(Some(Uuid::new_v4())), // Would be order ID
                    reference_type: Set(Some("SALES_ORDER".to_string())),
                    reason: Set(Some("Order allocation".to_string())),
                    notes: Set(Some(format!(
                        "Allocated for order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::new_v4()), // Would be actual user
                    created_at: Set(Utc::now()),
                };

                transaction
                    .insert(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                info!(
                    "Allocated {} units of item {:?} for order line {}",
                    required_qty, order_line.inventory_item_id, order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id: order_line.inventory_item_id.unwrap_or(0),
                    location_id: order_line.location_id.unwrap_or(0),
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
                let inventory = InventoryBalance::find()
                    .filter(
                        inventory_balance::Column::InventoryItemId.eq(order_line.inventory_item_id),
                    )
                    .filter(inventory_balance::Column::LocationId.eq(order_line.location_id))
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {:?}",
                            order_line.inventory_item_id
                        ))
                    })?;

                let ship_qty = order_line.ordered_quantity.unwrap_or(Decimal::ZERO);

                // Update inventory balance
                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_on_hand = Set(inventory.quantity_on_hand - ship_qty);
                active_inventory.quantity_allocated = Set(inventory.quantity_allocated - ship_qty);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                // Create transaction record
                let transaction = inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(Uuid::new_v4()),
                    location_id: Set(Uuid::new_v4()),
                    r#type: Set(TransactionType::Ship.as_str().to_string()),
                    quantity: Set(ship_qty.round().to_string().parse::<i32>().unwrap_or(0)),
                    previous_quantity: Set((inventory.quantity_on_hand + ship_qty)
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    new_quantity: Set(inventory
                        .quantity_on_hand
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    reference_id: Set(Some(Uuid::new_v4())),
                    reference_type: Set(Some("SALES_ORDER_SHIPMENT".to_string())),
                    reason: Set(Some("Order shipment".to_string())),
                    notes: Set(Some(format!(
                        "Shipped for order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::new_v4()),
                    created_at: Set(Utc::now()),
                };

                transaction
                    .insert(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                info!(
                    "Shipped {} units of item {} for order line {}",
                    ship_qty,
                    order_line.inventory_item_id.unwrap_or(0),
                    order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id: order_line.inventory_item_id.unwrap_or(0),
                    location_id: order_line.location_id.unwrap_or(0),
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
                let inventory = InventoryBalance::find()
                    .filter(
                        inventory_balance::Column::InventoryItemId.eq(order_line.inventory_item_id),
                    )
                    .filter(inventory_balance::Column::LocationId.eq(order_line.location_id))
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {:?}",
                            order_line.inventory_item_id
                        ))
                    })?;

                let deallocate_qty = order_line
                    .ordered_quantity
                    .unwrap_or_else(|| Decimal::from(0));

                // Update inventory balance
                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_allocated =
                    Set(inventory.quantity_allocated - deallocate_qty);
                active_inventory.quantity_available =
                    Set(inventory.quantity_available + deallocate_qty);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                // Create transaction record
                let transaction = inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(Uuid::new_v4()),
                    location_id: Set(Uuid::new_v4()),
                    r#type: Set(TransactionType::Deallocate.as_str().to_string()),
                    quantity: Set(deallocate_qty
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    previous_quantity: Set(inventory
                        .quantity_on_hand
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    new_quantity: Set(inventory
                        .quantity_on_hand
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    reference_id: Set(Some(Uuid::new_v4())),
                    reference_type: Set(Some("SALES_ORDER_CANCEL".to_string())),
                    reason: Set(Some("Order cancellation".to_string())),
                    notes: Set(Some(format!(
                        "Deallocated for cancelled order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::new_v4()),
                    created_at: Set(Utc::now()),
                };

                transaction
                    .insert(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                info!(
                    "Deallocated {} units of item {} for cancelled order line {}",
                    deallocate_qty,
                    order_line.inventory_item_id.unwrap_or(0),
                    order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id: order_line.inventory_item_id.unwrap_or(0),
                    location_id: order_line.location_id.unwrap_or(0),
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
                let inventory = InventoryBalance::find()
                    .filter(
                        inventory_balance::Column::InventoryItemId.eq(order_line.inventory_item_id),
                    )
                    .filter(inventory_balance::Column::LocationId.eq(order_line.location_id))
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?
                    .ok_or_else(|| {
                        ServiceError::NotFound(format!(
                            "Inventory not found for item {:?}",
                            order_line.inventory_item_id
                        ))
                    })?;

                let return_qty = order_line.ordered_quantity.unwrap_or(Decimal::ZERO);

                // Update inventory balance
                let mut active_inventory: inventory_balance::ActiveModel = inventory.clone().into();
                active_inventory.quantity_on_hand = Set(inventory.quantity_on_hand + return_qty);
                active_inventory.quantity_available =
                    Set(inventory.quantity_available + return_qty);
                active_inventory.updated_at = Set(Utc::now().into());

                let updated_inventory = active_inventory
                    .update(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                // Create transaction record
                let transaction = inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(Uuid::new_v4()),
                    location_id: Set(Uuid::new_v4()),
                    r#type: Set(TransactionType::Return.as_str().to_string()),
                    quantity: Set(return_qty.round().to_string().parse::<i32>().unwrap_or(0)),
                    previous_quantity: Set((inventory.quantity_on_hand - return_qty)
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    new_quantity: Set(inventory
                        .quantity_on_hand
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    reference_id: Set(Some(Uuid::new_v4())),
                    reference_type: Set(Some("SALES_ORDER_RETURN".to_string())),
                    reason: Set(Some("Order return".to_string())),
                    notes: Set(Some(format!(
                        "Returned from order line {}",
                        order_line.line_id
                    ))),
                    created_by: Set(Uuid::new_v4()),
                    created_at: Set(Utc::now()),
                };

                transaction
                    .insert(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                info!(
                    "Returned {} units of item {} from order line {}",
                    return_qty,
                    order_line.inventory_item_id.unwrap_or(0),
                    order_line.line_id
                );

                Ok(InventoryAdjustmentResult {
                    item_id: order_line.inventory_item_id.unwrap_or(0),
                    location_id: order_line.location_id.unwrap_or(0),
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
                    .one(txn)
                    .await
                    .map_err(ServiceError::db_error)?;

                let (updated_inventory, was_created) = match inventory {
                    Some(inv) => {
                        // Update existing inventory
                        let mut active_inventory: inventory_balance::ActiveModel =
                            inv.clone().into();
                        active_inventory.quantity_on_hand =
                            Set(inv.quantity_on_hand + quantity_received);
                        active_inventory.quantity_available =
                            Set(inv.quantity_available + quantity_received);
                        active_inventory.updated_at = Set(Utc::now().into());

                        let updated = active_inventory
                            .update(txn)
                            .await
                            .map_err(ServiceError::db_error)?;
                        (updated, false)
                    }
                    None => {
                        // Create new inventory balance
                        let new_inventory = inventory_balance::ActiveModel {
                            inventory_item_id: Set(po_line.item_id.unwrap_or(0)),
                            location_id: Set(location_id),
                            quantity_on_hand: Set(quantity_received),
                            quantity_allocated: Set(Decimal::ZERO),
                            quantity_available: Set(quantity_received),
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

                // Create transaction record
                let transaction = inventory_transaction::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    product_id: Set(Uuid::new_v4()),
                    location_id: Set(Uuid::new_v4()),
                    r#type: Set(TransactionType::Receive.as_str().to_string()),
                    quantity: Set(quantity_received
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    previous_quantity: Set(if was_created {
                        0
                    } else {
                        (updated_inventory.quantity_on_hand - quantity_received)
                            .round()
                            .to_string()
                            .parse::<i32>()
                            .unwrap_or(0)
                    }),
                    new_quantity: Set(updated_inventory
                        .quantity_on_hand
                        .round()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(0)),
                    reference_id: Set(Some(Uuid::new_v4())),
                    reference_type: Set(Some("PURCHASE_ORDER_RECEIPT".to_string())),
                    reason: Set(Some("PO receipt".to_string())),
                    notes: Set(Some(format!(
                        "Received from PO line {}",
                        po_line.po_line_id
                    ))),
                    created_by: Set(Uuid::new_v4()),
                    created_at: Set(Utc::now()),
                };

                transaction
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
