use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::future::{join_all, BoxFuture};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};
use uuid::Uuid;
use rust_decimal::Decimal;
// use bigdecimal::BigDecimal;

pub mod outbox;

/// Event data payload for structured event information
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum EventData {
    /// Return events
    ReturnClosed {
        return_id: Uuid,
        timestamp: DateTime<Utc>,
        reason: Option<String>,
    },
    ReturnCompleted {
        return_id: Uuid,
        timestamp: DateTime<Utc>,
        refund_amount: Option<f64>,
    },
    
    /// Generic event data
    Generic {
        message: String,
        timestamp: DateTime<Utc>,
        metadata: serde_json::Value,
    },
}

#[derive(Debug, Clone)]
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
        self.sender
            .send(event)
            .await
            .map_err(|e| format!("Failed to send event: {}", e))
    }
}

// Define the various events that can occur in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // Order events
    OrderCreated(Uuid),
    OrderUpdated(Uuid),
    OrderCancelled(Uuid),
    OrderCompleted(Uuid),
    OrderStatusChanged {
        order_id: Uuid,
        old_status: String,
        new_status: String,
    },
    
    // Inventory events
    InventoryUpdatedLegacy { item_id: Uuid, quantity: i32 },
    InventoryAllocated { 
        item_id: Uuid, 
        quantity: i32,
        reference_id: Uuid,
        reference_type: String,
        warehouse_id: Uuid,
        allocations: Vec<Uuid>,
        fully_allocated: bool,
    },
    InventoryDeallocated { item_id: Uuid, quantity: i32 },
    InventoryAdjusted {
        warehouse_id: Uuid,
        product_id: Uuid,
        old_quantity: i32,
        new_quantity: i32,
        reason_code: String,
        transaction_id: Uuid,
        reference_number: Option<String>,
    },
    InventoryReserved {
        warehouse_id: Uuid,
        product_id: Uuid,
        quantity: i32,
        reference_id: Uuid,
        reference_type: String,
        partial: bool,
    },
    PartialAllocationWarning {
        reference_id: Uuid,
        reference_type: String,
        requested_quantity: i32,
        allocated_quantity: i32,
    },
    PartialReservationWarning {
        reference_id: Uuid,
        reference_type: String,
        requested_quantity: i32,
        reserved_quantity: i32,
    },
    
    // Return events
    ReturnCreated(Uuid),
    ReturnUpdated(Uuid),
    ReturnApproved(Uuid),
    ReturnRejected(Uuid),
    
    // Payment events
    PaymentAuthorized(Uuid),
    PaymentCaptured(Uuid),
    PaymentRefunded(Uuid),
    PaymentFailed(Uuid),
    PaymentVoided(Uuid),
    
    // Shipment events
    ShipmentCreated(Uuid),
    ShipmentUpdated(Uuid),
    ShipmentDelivered(Uuid),
    ShipmentCancelled(Uuid),
    
    // ASN events
    ASNDeleted {
        asn_id: Uuid,
        asn_number: String,
    },
    
    // Warranty events
    WarrantyCreated(Uuid),
    WarrantyClaimed(Uuid),
    WarrantyExpired(Uuid),
    
    // Work order events
    WorkOrderCreatedLegacy(Uuid),
    WorkOrderUpdated(Uuid),
    WorkOrderCompletedLegacy(Uuid),
    WorkOrderCancelled(Uuid),
    WorkOrderAverageCostCalculated {
        product_id: Uuid,
        average_cost: rust_decimal::Decimal,
    },
    
    // COGS events
    MonthlyCOGSCalculated(String, rust_decimal::Decimal),
    COGSCalculated {
        work_order_id: Uuid,
        total_cogs: rust_decimal::Decimal,
    },
    
    // BOM events
    BOMCreated(i32),
    BOMDeleted(i32),
    BOMDuplicated(i32),
    ComponentAddedToBOM { bom_id: Uuid, component_id: Uuid },
    
    // Commerce events
    ProductCreated(Uuid),
    ProductUpdated(Uuid),
    ProductDeleted(Uuid),
    VariantCreated { product_id: Uuid, variant_id: Uuid },
    VariantUpdated { product_id: Uuid, variant_id: Uuid },
    CartCreated(Uuid),
    CartUpdated(Uuid),
    CartItemAdded { cart_id: Uuid, variant_id: Uuid },
    CartItemUpdated { cart_id: Uuid, item_id: Uuid },
    CartItemRemoved { cart_id: Uuid, item_id: Uuid },
    CartCleared(Uuid),
    
    // Checkout events
    CheckoutStarted { cart_id: Uuid, session_id: Uuid },
    CheckoutCompleted { session_id: Uuid, order_id: Uuid },
    
    // Customer events
    CustomerCreated(Uuid),
    CustomerUpdated(Uuid),
    
    // ASN (Advanced Shipping Notice) events
    ASNCreated(Uuid),
    ASNUpdated(Uuid),
    ASNCancelled(Uuid),
    ASNInTransit(Uuid),
    ASNDelivered(Uuid),
    ASNOnHold(Uuid),
    ASNReleasedFromHold(Uuid),
    ASNItemAdded(Uuid),
    ASNItemsUpdated(Uuid),
    ASNCancellationNotificationRequested(Uuid),
    
    // Generic event for custom messages
    Generic {
        message: String,
        timestamp: DateTime<Utc>,
        metadata: serde_json::Value,
    },
    
    // Additional order events
    OrderItemAdded(Uuid, Uuid),
    OrderNoteAdded(Uuid, Uuid),
    OrderNoteDeleted(Uuid, Uuid),
    OrderExchanged(Uuid),
    OrderOnHold(Uuid),
    ShippingMethodUpdated(Uuid),
    OrdersMerged(Vec<Uuid>, Uuid),
    
    // Promotion events
    PromotionCreated(Uuid),
    PromotionDeactivated(Uuid),
    
    // Purchase Order events
    PurchaseOrderCreated(Uuid),
    
    // Additional return events
    ReturnRefunded(Uuid),
    ReturnDeleted(Uuid),
    ReturnReopened(Uuid),
    
    // Additional shipment events
    OrderShipped(Uuid),
    ShipmentOnHold(Uuid),
    ShipmentRescheduled(Uuid, DateTime<Utc>),
    ShipmentTracked(Uuid),
    CarrierAssignedToShipment(Uuid, String),
    
    // Weighted Average COGS event
    WeightedAverageCOGSCalculated(Uuid, Decimal),
    
    // Manufacturing and inventory sync events
    InventoryUpdated {
        item_id: i64,
        location_id: i32,
        new_quantity: Decimal,
        available_quantity: Decimal,
    },
    // Inventory adjustment events for orders
    InventoryAdjustedForOrder {
        order_id: i64,
        adjustment_type: String,
    },
    InventoryReceivedFromPO {
        po_id: i64,
    },
    WorkOrderCreated {
        work_order_id: i64,
        item_id: i64,
        quantity: Decimal,
    },
    WorkOrderStarted {
        work_order_id: i64,
        item_id: i64,
    },
    WorkOrderCompleted {
        work_order_id: i64,
        item_id: i64,
        quantity_completed: Decimal,
    },
    PurchaseOrderReceived {
        po_line_id: i64,
        item_id: i64,
        quantity: Decimal,
        location_id: i32,
    },
    PurchaseOrderReturned {
        receipt_line_id: i64,
        item_id: i64,
        quantity: Decimal,
        reason: String,
    },
    SalesOrderFulfilled {
        order_id: i64,
        line_id: i64,
        item_id: i64,
        quantity: Decimal,
    },
    SalesOrderReturned {
        fulfillment_id: i64,
        item_id: i64,
        quantity: Decimal,
        reason: String,
    },
}

impl Event {
    /// Create a generic event with string data
    pub fn with_data(data: String) -> Self {
        Event::Generic {
            message: data,
            timestamp: Utc::now(),
            metadata: serde_json::Value::Null,
        }
    }
}

// Define a trait for handling events. Handlers implementing this trait will process events asynchronously.
#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle_event(&self, event: Event) -> Result<(), String>;
}

// Function to process incoming events and distribute them to registered event handlers.
pub async fn process_events(mut rx: mpsc::Receiver<Event>) {
    info!("Starting event processing loop");

    while let Some(event) = rx.recv().await {
        info!("Received event: {:?}", event);

        // Process events based on type
        match event {
            Event::OrderCreated(order_id) => {
                // Handle order created event
                if let Err(e) = handle_order_created(order_id).await {
                    error!(
                        "Failed to handle order created event: order_id={}, error={}",
                        order_id, e
                    );
                }
            }
            // Inventory events with enhanced handlers
            Event::InventoryAdjusted {
                warehouse_id,
                product_id,
                old_quantity,
                new_quantity,
                reason_code,
                transaction_id,
                reference_number,
            } => {
                if let Err(e) = handle_inventory_adjustment(
                    &warehouse_id.to_string(),
                    product_id,
                    old_quantity,
                    new_quantity,
                    &reason_code,
                    transaction_id,
                    reference_number.as_deref(),
                )
                .await
                {
                    error!(
                        "Failed to handle inventory adjustment: product_id={}, error={}",
                        product_id, e
                    );
                }
            }
            Event::InventoryAllocated {
                reference_id,
                reference_type,
                warehouse_id,
                allocations,
                fully_allocated,
                item_id,
                quantity,
            } => {
                if let Err(e) = handle_inventory_allocated(
                    reference_id,
                    &reference_type,
                    &warehouse_id.to_string(),
                    &allocations.iter().map(|uuid| serde_json::json!(uuid.to_string())).collect::<Vec<_>>(),
                    fully_allocated,
                )
                .await
                {
                    error!(
                        "Failed to handle inventory allocation: reference_id={}, error={}",
                        reference_id, e
                    );
                }
            }
            Event::PartialAllocationWarning {
                reference_id,
                reference_type,
                requested_quantity,
                allocated_quantity,
            } => {
                if let Err(e) =
                    handle_partial_allocation_warning(reference_id, &reference_type, requested_quantity, allocated_quantity)
                        .await
                {
                    error!(
                        "Failed to handle partial allocation warning: reference_id={}, error={}",
                        reference_id, e
                    );
                }
            }
            Event::InventoryReserved {
                reference_id,
                reference_type,
                warehouse_id,
                product_id,
                quantity,
                partial,
            } => {
                // Handle inventory reservation event
                if let Err(e) = handle_inventory_reserved(
                    reference_id,
                    &reference_type,
                    &warehouse_id.to_string(),
                    product_id,
                    quantity,
                )
                .await
                {
                    error!(
                        "Failed to handle inventory reserved event: reference_id={}, error={}",
                        reference_id, e
                    );
                }
            }
            Event::PartialReservationWarning {
                reference_id,
                reference_type,
                requested_quantity,
                reserved_quantity,
            } => {
                // Handle partial reservation warning
                if let Err(e) = handle_partial_reservation_warning(
                    reference_id,
                    &reference_type,
                    requested_quantity,
                    reserved_quantity,
                )
                .await
                {
                    error!(
                        "Failed to handle partial reservation warning: reference_id={}, error={}",
                        reference_id, e
                    );
                }
            }
            Event::PaymentAuthorized(payment_id) => {
                info!("Payment authorized: {}", payment_id);
            }
            Event::PaymentCaptured(payment_id) => {
                info!("Payment captured: {}", payment_id);
            }
            Event::PaymentRefunded(payment_id) => {
                info!("Payment refunded: {}", payment_id);
            }
            Event::PaymentFailed(payment_id) => {
                warn!("Payment failed: {}", payment_id);
            }
            Event::PaymentVoided(payment_id) => {
                info!("Payment voided: {}", payment_id);
            }
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

async fn handle_inventory_adjusted(product_id: Uuid, adjustment: i32) -> Result<(), String> {
    // This is a legacy handler that's being kept for backward compatibility
    // The new event structure should use handle_inventory_adjustment instead
    info!(
        "Processing inventory adjustment of {} for product {}",
        adjustment, product_id
    );

    // This would be implemented with actual business logic
    Ok(())
}

async fn handle_inventory_adjustment(
    warehouse_id: &str,
    product_id: Uuid,
    old_quantity: i32,
    new_quantity: i32,
    reason_code: &str,
    transaction_id: Uuid,
    reference_number: Option<&str>,
) -> Result<(), String> {
    info!(
        "Processing inventory adjustment: product={}, warehouse={}, old_quantity={}, new_total={}, reason={}",
        product_id, warehouse_id, old_quantity, new_quantity, reason_code
    );

    // Business logic based on reason code
    match reason_code {
        "DAMAGED" => {
            warn!(
                "Inventory damaged: {} units of product {} in warehouse {}",
                (new_quantity - old_quantity).abs(),
                product_id,
                warehouse_id
            );
            // Could trigger quality inspection or insurance claim
        }
        "CYCLE_COUNT" => {
            info!(
                "Cycle count adjustment: {} units of product {} in warehouse {}",
                (new_quantity - old_quantity), product_id, warehouse_id
            );
            // Could update accuracy metrics
        }
        _ => {
            info!("Standard inventory adjustment with reason: {}", reason_code);
        }
    }

    // This would be implemented with actual business logic
    // For example, if inventory drops below thresholds, trigger reordering
    if new_quantity < 10 {
        warn!(
            "Low inventory alert: product {} has only {} units remaining",
            product_id, new_quantity
        );
        // Trigger reorder or purchasing workflow
    }

    Ok(())
}

async fn handle_inventory_allocated(
    reference_id: Uuid,
    reference_type: &str,
    warehouse_id: &str,
    allocations: &[serde_json::Value],
    fully_allocated: bool,
) -> Result<(), String> {
    info!(
        "Processing inventory allocation for reference {} of type {} in warehouse {}",
        reference_id, reference_type, warehouse_id
    );

    // Log details about each allocation
    for (index, allocation) in allocations.iter().enumerate() {
        info!(
            "Allocation {}: data={:?}",
            index,
            allocation
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
    requested_quantity: i32,
    allocated_quantity: i32,
) -> Result<(), String> {
    warn!(
        "PARTIAL ALLOCATION WARNING: reference {} of type {} could not be fully allocated. Requested: {}, Allocated: {}",
        reference_id, reference_type, requested_quantity, allocated_quantity
    );

    // This would be implemented with actual business logic
    // For example, notifying customer service or purchasing department
    if reference_type == "ORDER" {
        warn!(
            "Order {} cannot be fully allocated - notifying customer service",
            reference_id
        );
        // Send notification to customer service team
    }

    Ok(())
}

async fn handle_inventory_reserved(
    reference_id: Uuid,
    reference_type: &str,
    warehouse_id: &str,
    product_id: Uuid,
    quantity: i32,
) -> Result<(), String> {
    info!(
        "Processing inventory reservation for reference {} of type {} in warehouse {}",
        reference_id, reference_type, warehouse_id
    );

    // Log details about each reservation
    // The original code had `reservations` here, but the new event structure doesn't include it.
    // Assuming `product_id` and `quantity` are sufficient for logging.
    info!(
        "Reservation: product_id={}, quantity={}",
        product_id, quantity
    );

    // This would be implemented with actual business logic - e.g., updating order status
    // if the reference_type is "SALES_ORDER"
    if reference_type == "SALES_ORDER" {
        info!(
            "Updating order {} with inventory reservation status",
            reference_id
        );
        // Additional logic would go here
    }

    Ok(())
}

async fn handle_partial_reservation_warning(
    reference_id: Uuid,
    reference_type: &str,
    requested_quantity: i32,
    reserved_quantity: i32,
) -> Result<(), String> {
    warn!(
        "PARTIAL RESERVATION WARNING: reference {} of type {} could not be fully reserved. Requested: {}, Reserved: {}",
        reference_id, reference_type, requested_quantity, reserved_quantity
    );

    // Log details about each reservation to identify shortfalls
    // The original code had `reservations` here, but the new event structure doesn't include it.
    // Assuming `requested_quantity` and `reserved_quantity` are sufficient for logging.
    warn!(
        "Reservation shortfall: requested={}, reserved={}",
        requested_quantity, reserved_quantity
    );

    // This would be implemented with actual business logic - e.g., notifying purchasing
    // or automatically creating purchase orders for the missing inventory
    if reference_type == "SALES_ORDER" {
        warn!(
            "Order {} cannot be fully fulfilled - notifying purchasing department",
            reference_id
        );
        // Additional logic would go here to trigger alerts or create purchase orders
    }

    Ok(())
}
