# Work Order Commands

## Overview

This directory contains CQRS-style commands for the **modern UUID-based work order system** used for maintenance, repairs, and general asset management.

⚠️ **Important:** These commands are for `work_order_entity::Model` (UUID-based), which is separate from the **manufacturing work order system** (`manufacturing_work_orders::Model`, i64-based) used for production operations.

## Two Work Order Systems

### 1. Manufacturing Work Orders (Production)
- **Entity:** `manufacturing_work_orders` (i64 primary key)
- **Location:** `src/entities/manufacturing_work_orders.rs`
- **Service:** `ManufacturingService` in `src/services/manufacturing.rs`
- **Purpose:** Production planning, BOM-driven manufacturing, component consumption
- **Operations:**
  - Create production work orders with BOM validation
  - Reserve and consume components
  - Track production quantities
  - Calculate yield and cycle times

### 2. General Work Orders (Maintenance/Assets)
- **Entity:** `work_order` (UUID primary key)
- **Location:** `src/models/work_order.rs`
- **Service:** `WorkOrderService` in `src/services/work_orders.rs`
- **Purpose:** Maintenance tasks, repairs, asset management
- **Operations:**
  - Create maintenance tasks
  - Assign to personnel
  - Track completion

## Currently Enabled Commands

### Active Commands:
- ✅ `add_note_to_work_order_command` - Add notes to work orders
- ✅ `assign_work_order_command` - Assign work orders to users
- ✅ `calculate_average_cost_command` - Calculate average production costs
- ✅ `calculate_cogs_command` - Calculate cost of goods sold
- ✅ `calculate_monthly_cogs_command` - Monthly COGS aggregation
- ✅ `calculate_weighted_average_cogs_command` - Weighted average costing
- ✅ `delete_work_order_command` - Delete work orders
- ✅ `get_work_order_command` - Retrieve work order details
- ✅ `list_work_orders` - List all work orders with filtering

### Disabled Commands (Refactored to Services):
The following commands have been refactored into service layer methods for better separation of concerns and are currently disabled:

- ⏸️ `cancel_work_order_command` → Use `WorkOrderService::cancel()`
- ⏸️ `complete_work_order_command` → Use `WorkOrderService::complete()`
- ⏸️ `create_work_order_command` → Use `WorkOrderService::create_work_order()`
- ⏸️ `issue_work_order_command` → Use `WorkOrderService` methods
- ⏸️ `pick_work_order_command` → Use `WorkOrderService` methods
- ⏸️ `schedule_work_order_command` → Use `WorkOrderService` methods
- ⏸️ `start_work_order_command` → Use `WorkOrderService` methods
- ⏸️ `unassign_work_order_command` → Use `WorkOrderService::unassign()`
- ⏸️ `update_work_order_command` → Use `WorkOrderService::update_work_order()`
- ⏸️ `yield_work_order_command` → Use `WorkOrderService` methods

## Architectural Decision

The project is transitioning from a CQRS command pattern to a service layer pattern for work order operations. The service layer provides:

1. **Better testability** - Services are easier to mock and test
2. **Clearer boundaries** - Service methods have explicit contracts
3. **Less boilerplate** - No need for Command trait implementations
4. **Better transaction management** - Services handle transactions internally

## When to Use Commands vs. Services

### Use Commands for:
- Complex operations requiring multiple steps
- Operations that need to be queued/deferred
- Operations requiring audit trails
- CQRS-style architectures

### Use Services for:
- Direct CRUD operations
- Real-time operations
- Operations requiring immediate responses
- Business logic encapsulation

## Future Work

### Option 1: Complete Migration to Services
- Remove all disabled command files
- Update any remaining references
- Simplify architecture to pure service layer

### Option 2: Re-enable Commands for CQRS
- Update disabled commands to use current service methods
- Implement proper command bus/queue
- Add command versioning and replay capabilities

### Option 3: Hybrid Approach
- Keep costing commands (complex, analytical)
- Use services for CRUD operations
- Document clear boundaries

## For Manufacturing Operations

For production-related work orders, **always use `ManufacturingService`**:

```rust
use stateset_api::services::manufacturing::ManufacturingService;

// Create production work order with component reservation
let work_order = manufacturing_service
    .create_work_order(wo_number, item_id, org_id, quantity, start_date, end_date, location)
    .await?;

// Start production and consume components
manufacturing_service
    .start_work_order(work_order_id, location_id)
    .await?;

// Complete production and add finished goods
manufacturing_service
    .complete_work_order(work_order_id, completed_qty, location_id)
    .await?;
```

## For General/Maintenance Work Orders

For maintenance and asset management work orders, use `WorkOrderService`:

```rust
use stateset_api::services::work_orders::WorkOrderService;

// Create maintenance work order
let work_order = work_order_service
    .create_work_order(work_order_data)
    .await?;

// Assign to technician
work_order_service
    .assign_work_order(work_order_id, user_id)
    .await?;
```

## References

- Manufacturing Service: `src/services/manufacturing.rs`
- Work Order Service: `src/services/work_orders.rs`
- Manufacturing Entities: `src/entities/manufacturing_work_orders.rs`
- Work Order Model: `src/models/work_order.rs`
