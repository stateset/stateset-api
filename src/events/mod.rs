use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use tokio::sync::{mpsc, broadcast};
use std::sync::Arc;
use tracing::{info, error, warn};
use futures::future::{join_all, BoxFuture};
use uuid::Uuid;

/// Event sender wrapper for the application
pub struct EventSender {
    sender: mpsc::Sender<Event>,
}

impl EventSender {
    /// Creates a new EventSender
    pub fn new(sender: mpsc::Sender<Event>) -> Self {
        Self { sender }
    }
    
    /// Sends an event asynchronously
    pub async fn send(&self, event: Event) -> Result<(), String> {
        self.sender.send(event)
            .await
            .map_err(|e| format!("Failed to send event: {}", e))
    }
}

// Define the various events that can occur in the system.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Event {
    // BOM Events
    BOMDeleted(i32),
    BOMUpdated(i32),
    BOMCreated(i32),
    
    // Order Events
    OrderCreated(Uuid),
    OrderUpdated(Uuid),
    OrderCancelled(Uuid),
    OrderDeleted(Uuid),
    OrderCompleted(Uuid),
    OrderRefunded(Uuid),
    OrdersMerged(Uuid),
    OrderTagged(Uuid),
    OrderSplit(Uuid),
    OrderExchanged(Uuid),
    OrderReleasedFromHold(Uuid),
    OrderOnHold(Uuid),
    OrderShipped(Uuid),
    OrderItemAdded(Uuid),
    OrderArchived(i32),
    ShippingAddressUpdated(Uuid),
    
    // Return Events
    ReturnCreated(Uuid),
    ReturnProcessed(Uuid),
    ReturnInitiated(Uuid),
    ReturnCancelled(Uuid),
    ReturnClosed(Uuid),
    ReturnDeleted(Uuid),
    ReturnCompleted(Uuid),
    ReturnRefunded(Uuid),
    ReturnApproved(Uuid),
    ReturnRejected(Uuid),
    ReturnReopened(Uuid),
    
    // Warranty Events
    WarrantyCreated(Uuid),
    WarrantyClaimed(Uuid),
    WarrantyClaimApproved(Uuid),
    WarrantyClaimRejected(Uuid),
    
    // Shipment Events
    ShipmentCreated(Uuid),
    ShipmentCancelled(Uuid),
    ShipmentUpdated(Uuid),
    ShipmentTracked(Uuid),
    
    // ASN Events
    ASNCreated(Uuid),
    ASNUpdated(Uuid),
    ASNCancelled(Uuid),
    ASNInTransit(Uuid),
    ASNDelivered(Uuid),
    ASNItemAdded(Uuid, Uuid),
    ASNItemRemoved(Uuid, Uuid),
    ASNOnHold(Uuid),
    ASNReleasedFromHold(Uuid),
    
    // Inventory Events
    InventoryAdjusted { 
        warehouse_id: String,
        product_id: Uuid, 
        adjustment_quantity: i32,
        new_quantity: i32,
        reason_code: String,
        transaction_id: Uuid,
        reference_number: Option<String>, 
    },
    InventoryAllocated { 
        reference_id: Uuid,
        reference_type: String,
        warehouse_id: String,
        allocations: Vec<crate::commands::inventory::allocate_inventory_command::AllocationResult>,
        fully_allocated: bool,
    },
    PartialAllocationWarning {
        reference_id: Uuid,
        reference_type: String,
        warehouse_id: String,
    },
    InventoryDeallocated { product_id: Uuid, quantity: i32 },
    // Enhanced Inventory events to match command implementations
    InventoryReserved {
        reference_id: Uuid,
        reference_type: String,
        warehouse_id: String,
        reservations: Vec<crate::commands::inventory::reserve_inventory_command::ReservationResult>,
        fully_reserved: bool,
        expiration_date: chrono::DateTime<chrono::Utc>,
    },
    PartialReservationWarning {
        reference_id: Uuid,
        reference_type: String,
        warehouse_id: String,
        reservations: Vec<crate::commands::inventory::reserve_inventory_command::ReservationResult>,
        expiration_date: chrono::DateTime<chrono::Utc>,
    },
    InventoryReleased { product_id: Uuid, quantity: i32 },
    InventoryReceived { product_id: Uuid, quantity: i32 },
    InventoryTransferred { product_id: Uuid, from_warehouse: Uuid, to_warehouse: Uuid, quantity: i32 },
    InventoryCycleCountCompleted { product_id: Uuid, warehouse_id: Uuid },
    InventoryLevelSet(Uuid, Uuid),
    SafetyStockAlertCreated { product_id: Uuid, warehouse_id: Uuid },
    
    // Purchase Order Events
    PurchaseOrderCreated(Uuid),
    PurchaseOrderUpdated(Uuid),
    PurchaseOrderApproved(Uuid),
    PurchaseOrderRejected(Uuid),
    PurchaseOrderCancelled(Uuid),
    PurchaseOrderReceived(Uuid),
    
    // Work Order Events
    WorkOrderCreated(Uuid),
    WorkOrderStarted(Uuid),
    WorkOrderUnassigned(Uuid),
    WorkOrderAssigned(Uuid),
    WorkOrderCancelled(Uuid),
    WorkOrderUpdated(Uuid),
    WorkOrderCompleted(Uuid),
    WorkOrderIssued(Uuid),
    WorkOrderPicked(Uuid),
    WorkOrderYielded(Uuid),
    WorkOrderScheduled(Uuid),
    WorkOrderNoteAdded(Uuid, i32),
    
    // Generic event with data
    with_data(String),
}

// Define a trait for handling events. Handlers implementing this trait will process events asynchronously.
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: Event) -> Result<(), String>;
}

// Function to process incoming events and distribute them to registered event handlers.
pub async fn process_events(
    mut rx: mpsc::Receiver<Event>,
) {
    info!("Starting event processing loop");
    
    while let Some(event) = rx.recv().await {
        info!("Received event: {:?}", event);
        
        // Process events based on type
        match event {
            Event::OrderCreated(order_id) => {
                // Handle order created event
                if let Err(e) = handle_order_created(order_id).await {
                    error!("Failed to handle order created event: order_id={}, error={}", order_id, e);
                }
            },
            // Inventory events with enhanced handlers
            Event::InventoryAdjusted { warehouse_id, product_id, adjustment_quantity, new_quantity, reason_code, transaction_id, reference_number } => {
                if let Err(e) = handle_inventory_adjustment(
                    &warehouse_id, 
                    product_id, 
                    adjustment_quantity, 
                    new_quantity, 
                    &reason_code, 
                    transaction_id, 
                    reference_number.as_deref()
                ).await {
                    error!("Failed to handle inventory adjustment: product_id={}, error={}", product_id, e);
                }
            },
            Event::InventoryAllocated { reference_id, reference_type, warehouse_id, allocations, fully_allocated } => {
                if let Err(e) = handle_inventory_allocated(
                    reference_id, 
                    &reference_type, 
                    &warehouse_id, 
                    &allocations, 
                    fully_allocated
                ).await {
                    error!("Failed to handle inventory allocation: reference_id={}, error={}", reference_id, e);
                }
            },
            Event::PartialAllocationWarning { reference_id, reference_type, warehouse_id } => {
                if let Err(e) = handle_partial_allocation_warning(
                    reference_id, 
                    &reference_type, 
                    &warehouse_id
                ).await {
                    error!("Failed to handle partial allocation warning: reference_id={}, error={}", reference_id, e);
                }
            },
            Event::InventoryReserved { reference_id, reference_type, warehouse_id, reservations, fully_reserved, .. } => {
                // Handle inventory reservation event
                if let Err(e) = handle_inventory_reserved(reference_id, &reference_type, &warehouse_id, &reservations, fully_reserved).await {
                    error!("Failed to handle inventory reserved event: reference_id={}, error={}", reference_id, e);
                }
            },
            Event::PartialReservationWarning { reference_id, reference_type, warehouse_id, reservations, .. } => {
                // Handle partial reservation warning
                if let Err(e) = handle_partial_reservation_warning(reference_id, &reference_type, &warehouse_id, &reservations).await {
                    error!("Failed to handle partial reservation warning: reference_id={}, error={}", reference_id, e);
                }
            },
            // Add more event handlers as needed
            _ => {
                info!("No specific handler for event: {:?}", event);
            }
        }
    }

    warn!("Event processing loop has ended");
}

// Handler functions for specific events
async fn handle_order_created(order_id: Uuid) -> Result<(), String> {
    // Example implementation: When an order is created, we may need to allocate inventory
    // or notify the warehouse system
    info!("Processing order created event for order {}", order_id);
    
    // This would be implemented with actual business logic
    Ok(())
}

async fn handle_inventory_adjusted(
    product_id: Uuid, 
    adjustment: i32
) -> Result<(), String> {
    // This is a legacy handler that's being kept for backward compatibility
    // The new event structure should use handle_inventory_adjustment instead
    info!("Processing inventory adjustment of {} for product {}", adjustment, product_id);
    
    // This would be implemented with actual business logic
    Ok(())
}

async fn handle_inventory_adjustment(
    warehouse_id: &str,
    product_id: Uuid,
    adjustment_quantity: i32,
    new_quantity: i32,
    reason_code: &str,
    transaction_id: Uuid,
    reference_number: Option<&str>
) -> Result<(), String> {
    info!(
        "Processing inventory adjustment: product={}, warehouse={}, adjustment={}, new_total={}, reason={}",
        product_id, warehouse_id, adjustment_quantity, new_quantity, reason_code
    );
    
    // Business logic based on reason code
    match reason_code {
        "DAMAGED" => {
            warn!("Inventory damaged: {} units of product {} in warehouse {}", 
                adjustment_quantity.abs(), product_id, warehouse_id);
            // Could trigger quality inspection or insurance claim
        },
        "CYCLE_COUNT" => {
            info!("Cycle count adjustment: {} units of product {} in warehouse {}", 
                adjustment_quantity, product_id, warehouse_id);
            // Could update accuracy metrics
        },
        _ => {
            info!("Standard inventory adjustment with reason: {}", reason_code);
        }
    }
    
    // This would be implemented with actual business logic
    // For example, if inventory drops below thresholds, trigger reordering
    if new_quantity < 10 {
        warn!("Low inventory alert: product {} has only {} units remaining", product_id, new_quantity);
        // Trigger reorder or purchasing workflow
    }
    
    Ok(())
}

async fn handle_inventory_allocated(
    reference_id: Uuid,
    reference_type: &str,
    warehouse_id: &str,
    allocations: &[crate::commands::inventory::allocate_inventory_command::AllocationResult],
    fully_allocated: bool
) -> Result<(), String> {
    info!(
        "Processing inventory allocation for reference {} of type {} in warehouse {}",
        reference_id, reference_type, warehouse_id
    );
    
    // Log details about each allocation
    for (index, allocation) in allocations.iter().enumerate() {
        info!(
            "Allocation {}: product_id={}, requested={}, allocated={}",
            index,
            allocation.product_id,
            allocation.requested_quantity,
            allocation.allocated_quantity
        );
    }
    
    if fully_allocated {
        info!("All inventory was successfully allocated");
    } else {
        warn!("Only partial inventory was allocated - this may affect fulfillment");
    }
    
    // This would be implemented with actual business logic
    // For example, if allocating for an order, update order status
    if reference_type == "ORDER" {
        info!("Updating order {} with allocation status", reference_id);
        // Update order status to "ALLOCATED" or "PARTIALLY_ALLOCATED"
    }
    
    Ok(())
}

async fn handle_partial_allocation_warning(
    reference_id: Uuid,
    reference_type: &str,
    warehouse_id: &str
) -> Result<(), String> {
    warn!(
        "PARTIAL ALLOCATION WARNING: reference {} of type {} in warehouse {} could not be fully allocated",
        reference_id, reference_type, warehouse_id
    );
    
    // This would be implemented with actual business logic
    // For example, notifying customer service or purchasing department
    if reference_type == "ORDER" {
        warn!("Order {} cannot be fully allocated - notifying customer service", reference_id);
        // Send notification to customer service team
    }
    
    Ok(())
}

async fn handle_inventory_reserved(
    reference_id: Uuid,
    reference_type: &str,
    warehouse_id: &str,
    reservations: &[crate::commands::inventory::reserve_inventory_command::ReservationResult],
    fully_reserved: bool
) -> Result<(), String> {
    info!(
        "Processing inventory reservation for reference {} of type {} in warehouse {}",
        reference_id, reference_type, warehouse_id
    );
    
    // Log details about each reservation
    for (index, reservation) in reservations.iter().enumerate() {
        info!(
            "Reservation {}: product_id={}, requested={}, reserved={}, expires={}",
            index,
            reservation.product_id,
            reservation.requested_quantity,
            reservation.reserved_quantity,
            reservation.expiration_date
        );
    }
    
    if fully_reserved {
        info!("All inventory was successfully reserved");
    } else {
        warn!("Only partial inventory was reserved - this may affect subsequent processes");
    }
    
    // This would be implemented with actual business logic - e.g., updating order status
    // if the reference_type is "SALES_ORDER"
    if reference_type == "SALES_ORDER" {
        info!("Updating order {} with inventory reservation status", reference_id);
        // Additional logic would go here
    }
    
    Ok(())
}

async fn handle_partial_reservation_warning(
    reference_id: Uuid,
    reference_type: &str,
    warehouse_id: &str,
    reservations: &[crate::commands::inventory::reserve_inventory_command::ReservationResult]
) -> Result<(), String> {
    warn!(
        "PARTIAL RESERVATION WARNING: reference {} of type {} in warehouse {} could not be fully reserved",
        reference_id, reference_type, warehouse_id
    );
    
    // Log details about each reservation to identify shortfalls
    for reservation in reservations {
        let shortfall = reservation.requested_quantity - reservation.reserved_quantity;
        warn!(
            "Product {} shortfall: requested={}, reserved={}, shortfall={}",
            reservation.product_id,
            reservation.requested_quantity,
            reservation.reserved_quantity,
            shortfall
        );
    }
    
    // This would be implemented with actual business logic - e.g., notifying purchasing
    // or automatically creating purchase orders for the missing inventory
    if reference_type == "SALES_ORDER" {
        warn!("Order {} cannot be fully fulfilled - notifying purchasing department", reference_id);
        // Additional logic would go here to trigger alerts or create purchase orders
    }
    
    Ok(())
}