# Manufacturing & Production API Guide

## Table of Contents

1. [Overview](#overview)
2. [Quick Start](#quick-start)
3. [Manufacturing Work Orders](#manufacturing-work-orders)
4. [Bill of Materials (BOM)](#bill-of-materials-bom)
5. [Component Reservation System](#component-reservation-system)
6. [Metrics & Observability](#metrics--observability)
7. [Error Handling](#error-handling)
8. [Best Practices](#best-practices)

---

## Overview

The Manufacturing & Production API provides comprehensive functionality for managing production work orders, bill of materials (BOM), component reservations, and finished goods production.

### Key Features

- ✅ **Production Work Orders** - Create, start, complete, hold, resume, cancel
- ✅ **Bill of Materials** - Multi-level BOM explosion with circular reference detection
- ✅ **Component Reservation** - Race-condition-free inventory allocation
- ✅ **Automatic Inventory Sync** - Component consumption and finished goods receipt
- ✅ **Real-time Events** - Complete lifecycle event coverage
- ✅ **Production Metrics** - Cycle time, yield percentage, throughput
- ✅ **Input Validation** - Comprehensive validation with clear error messages

### Architecture

```
┌─────────────────────────────────────────────────────┐
│                   API Layer                         │
│              (Handlers / gRPC)                      │
└──────────────────┬──────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────┐
│               Service Layer                          │
│  ┌────────────────┐  ┌─────────────────────────┐  │
│  │ Manufacturing  │  │     BomService          │  │
│  │    Service     │◄─┤  (BOM Management)      │  │
│  │                │  └─────────────────────────┘  │
│  └────────┬───────┘                                │
│           │                                        │
│           ▼                                        │
│  ┌────────────────────────┐                       │
│  │  InventorySyncService  │                       │
│  │  (Reservations & Sync) │                       │
│  └────────────────────────┘                       │
└─────────────────────────────────────────────────────┘
```

---

## Quick Start

### 1. Service Initialization

```rust
use stateset_api::services::{
    manufacturing::ManufacturingService,
    bom::BomService,
    inventory_sync::InventorySyncService,
};
use std::sync::Arc;

// Initialize services
let inventory_sync = Arc::new(InventorySyncService::new(db.clone(), event_sender));
let bom_service = Arc::new(BomService::new(db.clone(), inventory_sync.clone()));
let manufacturing_service = ManufacturingService::new(
    db.clone(),
    inventory_sync.clone(),
    bom_service.clone(),
    Some(event_sender),
);
```

### 2. Complete Production Workflow

```rust
use rust_decimal::Decimal;
use chrono::NaiveDate;

// Step 1: Create BOM
let bom = bom_service.create_bom(
    "BOM-WIDGET-001".to_string(),
    item_id,
    organization_id,
    Some("1.0".to_string()),
).await?;

// Step 2: Add components to BOM
bom_service.add_bom_component(
    bom.bom_id,
    component_item_id,
    Decimal::from(5), // 5 units per assembly
    "EA".to_string(),
    Some(10),
).await?;

// Step 3: Create work order (with automatic component reservation)
let work_order = manufacturing_service.create_work_order(
    "WO-2024-001".to_string(),
    item_id,
    organization_id,
    Decimal::from(100), // Build 100 units
    NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
    NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
    location_id,
).await?;

// Step 4: Start production (consumes components)
let updated_wo = manufacturing_service.start_work_order(
    work_order.work_order_id,
    location_id,
).await?;

// Step 5: Complete production (adds finished goods)
let completed_wo = manufacturing_service.complete_work_order(
    work_order.work_order_id,
    Decimal::from(100), // Completed quantity
    location_id,
).await?;
```

---

## Manufacturing Work Orders

### Create Work Order

Creates a new production work order and reserves components if available.

**Method:** `ManufacturingService::create_work_order()`

**Parameters:**
- `work_order_number: String` - Unique work order number (e.g., "WO-2024-001")
- `item_id: i64` - Item to produce (must have active BOM)
- `organization_id: i64` - Organization ID
- `quantity_to_build: Decimal` - Quantity to produce
- `scheduled_start_date: NaiveDate` - Planned start date
- `scheduled_completion_date: NaiveDate` - Planned completion date
- `location_id: i32` - Production location

**Returns:** `Result<manufacturing_work_orders::Model, ServiceError>`

**Validation:**
- ✅ Work order number cannot be empty
- ✅ Quantity must be positive
- ✅ Completion date must be after start date
- ✅ All IDs must be positive
- ✅ Item must exist and have active BOM
- ✅ Component availability checked

**Status Logic:**
- Components available → Status: `READY`, components reserved
- Components insufficient → Status: `PENDING_MATERIALS`, no reservations

**Events Emitted:**
- `WorkOrderCreated` - Always emitted
- `WorkOrderScheduled` - Always emitted
- `WorkOrderMaterialsReserved` - If components available
- `ComponentShortageDetected` - For each shortage (if insufficient)

**Example:**

```rust
let work_order = manufacturing_service.create_work_order(
    "WO-2024-001".to_string(),
    12345, // item_id
    1, // organization_id
    Decimal::from(50), // quantity
    NaiveDate::from_ymd_opt(2024, 1, 10).unwrap(),
    NaiveDate::from_ymd_opt(2024, 1, 20).unwrap(),
    100, // location_id
).await?;

println!("Work Order {} created with status: {:?}",
    work_order.work_order_number,
    work_order.status_code
);
```

**Error Handling:**

```rust
match manufacturing_service.create_work_order(...).await {
    Ok(wo) => println!("Success: {}", wo.work_order_id),
    Err(ServiceError::InvalidInput(msg)) => {
        eprintln!("Validation error: {}", msg);
    },
    Err(ServiceError::NotFound(msg)) => {
        eprintln!("Item or BOM not found: {}", msg);
    },
    Err(e) => eprintln!("Error: {}", e),
}
```

---

### Start Work Order

Starts production and consumes reserved components from inventory.

**Method:** `ManufacturingService::start_work_order()`

**Parameters:**
- `work_order_id: i64` - Work order to start
- `location_id: i32` - Location for component consumption

**Returns:** `Result<manufacturing_work_orders::Model, ServiceError>`

**Validation:**
- ✅ Work order must exist
- ✅ Status must be `READY`
- ✅ Cannot start already started work orders

**Inventory Changes:**
- Reserved components are consumed
- `on_hand` decreased
- `allocated` decreased
- `available` unchanged (already reflected in allocated)

**Events Emitted:**
- `WorkOrderStarted`

**Metrics Recorded:**
- `manufacturing.work_orders.started` (counter)
- `manufacturing.components.consumed` (histogram)

**Example:**

```rust
// Start production
let wo = manufacturing_service.start_work_order(
    work_order_id,
    location_id,
).await?;

assert_eq!(wo.status_code, Some("IN_PROGRESS".to_string()));
assert!(wo.actual_start_date.is_some());
```

---

### Complete Work Order

Completes production and adds finished goods to inventory.

**Method:** `ManufacturingService::complete_work_order()`

**Parameters:**
- `work_order_id: i64` - Work order to complete
- `completed_quantity: Decimal` - Quantity produced
- `location_id: i32` - Location for finished goods

**Returns:** `Result<manufacturing_work_orders::Model, ServiceError>`

**Validation:**
- ✅ Work order must exist
- ✅ Status must be `IN_PROGRESS` or `PARTIALLY_COMPLETED`
- ✅ Completed quantity must be positive

**Status Logic:**
- `quantity_completed >= quantity_to_build` → Status: `COMPLETED`
- `quantity_completed < quantity_to_build` → Status: `PARTIALLY_COMPLETED`

**Inventory Changes:**
- Finished goods added to inventory
- `on_hand` increased by completed quantity

**Events Emitted:**
- `WorkOrderCompleted`

**Metrics Recorded:**
- `manufacturing.work_orders.completed` or `partially_completed` (counter)
- `manufacturing.work_orders.cycle_time_days` (histogram)
- `manufacturing.work_orders.yield_percentage` (histogram)
- `manufacturing.finished_goods.produced` (histogram)

**Example:**

```rust
// Complete full quantity
let wo = manufacturing_service.complete_work_order(
    work_order_id,
    Decimal::from(100), // Full quantity
    location_id,
).await?;

assert_eq!(wo.status_code, Some("COMPLETED".to_string()));
assert!(wo.actual_completion_date.is_some());

// Partial completion
let wo = manufacturing_service.complete_work_order(
    work_order_id,
    Decimal::from(50), // Partial
    location_id,
).await?;

assert_eq!(wo.status_code, Some("PARTIALLY_COMPLETED".to_string()));
assert_eq!(wo.quantity_completed, Some(Decimal::from(50)));
```

---

### Cancel Work Order

Cancels a work order and releases component reservations.

**Method:** `ManufacturingService::cancel_work_order()`

**Parameters:**
- `work_order_id: i64` - Work order to cancel
- `location_id: i32` - Location for reservation release

**Returns:** `Result<(), ServiceError>`

**Validation:**
- ✅ Work order must exist
- ✅ Cannot cancel if production started (`actual_start_date` is set)

**Inventory Changes:**
- Reserved components released back to available

**Events Emitted:**
- `WorkOrderMaterialsReleased`

**Metrics Recorded:**
- `manufacturing.work_orders.cancelled` (counter)

**Example:**

```rust
// Cancel work order before starting
manufacturing_service.cancel_work_order(
    work_order_id,
    location_id,
).await?;

// Try to cancel after start - will fail
match manufacturing_service.cancel_work_order(started_wo_id, location_id).await {
    Err(ServiceError::InvalidOperation(msg)) => {
        assert!(msg.contains("already started"));
    },
    _ => panic!("Should have failed"),
}
```

---

### Hold Work Order

Temporarily pauses a work order.

**Method:** `ManufacturingService::hold_work_order()`

**Parameters:**
- `work_order_id: i64` - Work order to hold
- `reason: Option<String>` - Reason for hold

**Returns:** `Result<manufacturing_work_orders::Model, ServiceError>`

**Validation:**
- ✅ Status must be `READY` or `IN_PROGRESS`

**Events Emitted:**
- `WorkOrderOnHold`

**Example:**

```rust
let wo = manufacturing_service.hold_work_order(
    work_order_id,
    Some("Waiting for quality inspection".to_string()),
).await?;

assert_eq!(wo.status_code, Some("ON_HOLD".to_string()));
```

---

### Resume Work Order

Resumes a held work order.

**Method:** `ManufacturingService::resume_work_order()`

**Parameters:**
- `work_order_id: i64` - Work order to resume

**Returns:** `Result<manufacturing_work_orders::Model, ServiceError>`

**Validation:**
- ✅ Status must be `ON_HOLD`

**Status Logic:**
- Never started (`actual_start_date` is None) → Resume to `READY`
- Was in progress (`actual_start_date` is Some) → Resume to `IN_PROGRESS`

**Events Emitted:**
- `WorkOrderResumed`

**Example:**

```rust
let wo = manufacturing_service.resume_work_order(work_order_id).await?;

// Status depends on whether it was started
if wo.actual_start_date.is_some() {
    assert_eq!(wo.status_code, Some("IN_PROGRESS".to_string()));
} else {
    assert_eq!(wo.status_code, Some("READY".to_string()));
}
```

---

## Bill of Materials (BOM)

### Create BOM

Creates a BOM header for a product.

**Method:** `BomService::create_bom()`

**Parameters:**
- `bom_name: String` - BOM name (e.g., "BOM-WIDGET-001")
- `item_id: i64` - Product item ID
- `organization_id: i64` - Organization ID
- `revision: Option<String>` - BOM revision (e.g., "1.0")

**Returns:** `Result<bom_header::Model, ServiceError>`

**Validation:**
- ✅ BOM name cannot be empty
- ✅ All IDs must be positive

**Example:**

```rust
let bom = bom_service.create_bom(
    "BOM-WIDGET-001".to_string(),
    item_id,
    1,
    Some("1.0".to_string()),
).await?;

println!("BOM created: ID={}, Name={}", bom.bom_id, bom.bom_name);
```

---

### Add BOM Component

Adds a component to a BOM.

**Method:** `BomService::add_bom_component()`

**Parameters:**
- `bom_id: i64` - BOM to add component to
- `component_item_id: i64` - Component item ID
- `quantity_per_assembly: Decimal` - Quantity needed per unit
- `uom_code: String` - Unit of measure (e.g., "EA", "LB", "FT")
- `operation_seq_num: Option<i32>` - Operation sequence

**Returns:** `Result<bom_line::Model, ServiceError>`

**Validation:**
- ✅ All IDs must be positive
- ✅ Quantity must be positive
- ✅ UOM code cannot be empty
- ✅ BOM must exist
- ✅ Component item must exist

**Example:**

```rust
// Add components to BOM
bom_service.add_bom_component(
    bom.bom_id,
    screw_item_id,
    Decimal::from(4), // 4 screws per unit
    "EA".to_string(),
    Some(10),
).await?;

bom_service.add_bom_component(
    bom.bom_id,
    plastic_item_id,
    Decimal::new(250, 3), // 0.250 lbs
    "LB".to_string(),
    Some(20),
).await?;
```

---

### Multi-Level BOM Explosion

Recursively explodes a multi-level BOM to get all components.

**Method:** `BomService::explode_bom()`

**Parameters:**
- `item_id: i64` - Top-level item
- `quantity: Decimal` - Production quantity
- `level: i32` - Starting level (usually 0)

**Returns:** `Result<Vec<ExplodedComponent>, ServiceError>`

**Features:**
- ✅ Recursive explosion through sub-assemblies
- ✅ Circular reference detection
- ✅ Level tracking for hierarchy

**Example:**

```rust
// Explode BOM for 100 units
let components = bom_service.explode_bom(
    assembly_item_id,
    Decimal::from(100),
    0,
).await?;

for comp in components {
    println!("Level {}: Item {} needs {} units",
        comp.level,
        comp.item_id,
        comp.quantity
    );
}

// Example output:
// Level 0: Item 200 needs 200 units  (sub-assembly, qty=2 each)
// Level 1: Item 300 needs 600 units  (component, qty=3 per sub-assembly)
// Level 0: Item 400 needs 500 units  (direct component, qty=5 each)
```

---

## Component Reservation System

### Reserve Components

Reserves components for a work order (called automatically by `create_work_order`).

**Method:** `BomService::reserve_components_for_work_order()`

**Parameters:**
- `bom_id: i64` - BOM for the work order
- `production_quantity: Decimal` - Quantity to build
- `location_id: i32` - Location
- `work_order_id: i64` - Work order ID

**Returns:** `Result<Vec<ComponentReservation>, ServiceError>`

**Inventory Changes:**
- `allocated` increased
- `available` decreased (available = on_hand - allocated)
- `on_hand` unchanged

**Metrics Recorded:**
- `manufacturing.bom.components_reserved` (counter)
- `manufacturing.bom.reservation_quantity` (histogram)

---

### Release Component Reservations

Releases reserved components (called automatically by `cancel_work_order`).

**Method:** `BomService::release_component_reservations()`

**Inventory Changes:**
- `allocated` decreased
- `available` increased
- `on_hand` unchanged

**Metrics Recorded:**
- `manufacturing.bom.reservations_released` (counter)

---

### Consume Reserved Components

Consumes reserved components (called automatically by `start_work_order`).

**Method:** `BomService::consume_reserved_components()`

**Inventory Changes (two-phase):**
1. Release reservation: `allocated` decreased
2. Consume inventory: `on_hand` decreased

**Metrics Recorded:**
- `manufacturing.bom.components_consumed` (counter)
- `manufacturing.bom.consumption_quantity` (histogram)

---

## Metrics & Observability

### Available Metrics

**Counters:**
```
manufacturing.work_orders.created
manufacturing.work_orders.started
manufacturing.work_orders.completed
manufacturing.work_orders.partially_completed
manufacturing.work_orders.cancelled
manufacturing.work_orders.on_hold
manufacturing.work_orders.resumed
manufacturing.work_orders.ready
manufacturing.work_orders.pending_materials
manufacturing.bom.components_reserved
manufacturing.bom.components_consumed
manufacturing.bom.reservations_released
```

**Histograms:**
```
manufacturing.work_orders.quantity (production volumes)
manufacturing.work_orders.cycle_time_days (lead times)
manufacturing.work_orders.yield_percentage (quality)
manufacturing.components.consumed (material usage)
manufacturing.finished_goods.produced (output)
manufacturing.bom.reservation_quantity
manufacturing.bom.consumption_quantity
```

### Monitoring Dashboard Example

```text
Production KPIs:
├─ Work Orders Created: 150 (today)
├─ Completion Rate: 95%
├─ Average Cycle Time: 8.5 days
├─ Average Yield: 98.2%
├─ On Hold: 5 orders
└─ Pending Materials: 12 orders

Inventory Impact:
├─ Components Reserved: 45,000 units
├─ Components Consumed: 42,000 units
└─ Finished Goods Produced: 8,500 units
```

---

## Error Handling

### Error Types

```rust
pub enum ServiceError {
    // Validation errors
    InvalidInput(String),           // Bad input data
    InvalidOperation(String),       // Operation not allowed

    // Resource errors
    NotFound(String),               // Resource doesn't exist
    InsufficientStock(String),      // Not enough inventory
    Conflict(String),               // Concurrent modification

    // System errors
    DatabaseError(DbErr),           // Database error
    InternalError(String),          // Internal error
}
```

### Common Error Scenarios

**1. Invalid Input**
```rust
// Negative quantity
Err(ServiceError::InvalidInput("Quantity to build must be positive, got: -10"))

// Empty string
Err(ServiceError::InvalidInput("Work order number cannot be empty"))

// Invalid dates
Err(ServiceError::InvalidInput("Scheduled completion date (2024-01-01) cannot be before scheduled start date (2024-01-15)"))
```

**2. Resource Not Found**
```rust
Err(ServiceError::NotFound("Item 12345 not found"))
Err(ServiceError::NotFound("No active BOM found for item 12345"))
Err(ServiceError::NotFound("Work order 999 not found"))
```

**3. Invalid Operation**
```rust
// Wrong status
Err(ServiceError::InvalidOperation("Work order 123 is not ready to start. Current status: PENDING_MATERIALS"))

// Already started
Err(ServiceError::InvalidOperation("Cannot cancel work order that has already started"))

// Circular BOM
Err(ServiceError::InvalidOperation("Circular BOM reference detected: item 100 references itself in the BOM structure"))
```

**4. Insufficient Stock**
```rust
Err(ServiceError::InsufficientStock("Insufficient components for production. Shortages: [...]"))
```

---

## Best Practices

### 1. Always Check Component Availability

```rust
// The create_work_order method automatically checks availability
let wo = manufacturing_service.create_work_order(...).await?;

// Check status
match wo.status_code.as_deref() {
    Some("READY") => {
        // Components reserved, can start immediately
        println!("Ready to start production");
    },
    Some("PENDING_MATERIALS") => {
        // Need to add inventory first
        println!("Waiting for components");

        // Get shortage details from events
        // ComponentShortageDetected events were emitted
    },
    _ => {},
}
```

### 2. Handle Partial Completions

```rust
// Report production incrementally
let wo = manufacturing_service.complete_work_order(
    work_order_id,
    Decimal::from(30), // First batch
    location_id,
).await?;

// Continue later
let wo = manufacturing_service.complete_work_order(
    work_order_id,
    Decimal::from(70), // Second batch
    location_id,
).await?;

// Now fully completed
assert_eq!(wo.status_code, Some("COMPLETED".to_string()));
assert_eq!(wo.quantity_completed, Some(Decimal::from(100)));
```

### 3. Use Hold for Quality Checks

```rust
// Complete production
let wo = manufacturing_service.complete_work_order(...).await?;

// Hold for quality inspection
manufacturing_service.hold_work_order(
    work_order_id,
    Some("Quality inspection required".to_string()),
).await?;

// After inspection passes
manufacturing_service.resume_work_order(work_order_id).await?;
```

### 4. Monitor Metrics

```rust
// Set up alerts based on metrics
if metrics.get("manufacturing.work_orders.pending_materials") > 10 {
    alert("High number of work orders pending materials");
}

if metrics.get_histogram_avg("manufacturing.work_orders.yield_percentage") < 95.0 {
    alert("Yield percentage below target");
}
```

### 5. Proper Error Handling

```rust
async fn safe_create_work_order(...) -> Result<(), AppError> {
    match manufacturing_service.create_work_order(...).await {
        Ok(wo) => {
            log::info!("Work order created: {}", wo.work_order_id);
            Ok(())
        },
        Err(ServiceError::InvalidInput(msg)) => {
            log::warn!("Validation failed: {}", msg);
            Err(AppError::ValidationFailed(msg))
        },
        Err(ServiceError::NotFound(msg)) => {
            log::error!("Resource not found: {}", msg);
            Err(AppError::NotFound(msg))
        },
        Err(e) => {
            log::error!("Unexpected error: {}", e);
            Err(AppError::Internal(e.to_string()))
        },
    }
}
```

---

## Support

For issues or questions:
- Review test files: `tests/bom_service_test.rs`, `tests/manufacturing_service_test.rs`
- Check integration tests: `tests/work_order_lifecycle_integration_test.rs`
- See command documentation: `src/commands/workorders/README.md`
