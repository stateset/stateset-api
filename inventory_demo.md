# Inventory Adjustment System with Item Master

## Overview

We've implemented a comprehensive inventory adjustment system that integrates with the item master and handles various order types. The system automatically adjusts inventory levels based on:

1. **Sales Orders** - Allocates, ships, cancels, and returns
2. **Purchase Orders** - Receives inventory from suppliers
3. **Manual Adjustments** - Cycle counts, damages, etc.

## Key Components

### 1. Item Master (`src/entities/item_master.rs`)
The central repository for all inventory items with relationships to:
- Inventory balances
- Sales order lines
- Purchase order lines
- BOM (Bill of Materials)
- Manufacturing work orders

### 2. Inventory Adjustment Service (`src/services/inventory_adjustment_service.rs`)
Core service that handles all inventory movements:

```rust
pub struct InventoryAdjustmentService {
    db_pool: Arc<DbPool>,
    event_sender: Arc<EventSender>,
}
```

Key methods:
- `adjust_for_sales_order()` - Handles SO allocations, shipments, cancellations, returns
- `adjust_for_purchase_order_receipt()` - Handles PO receipts

### 3. Event System
Events are triggered for all inventory adjustments:
- `InventoryAdjustedForOrder` - When SO affects inventory
- `InventoryReceivedFromPO` - When PO receipt occurs
- Full transaction history maintained

## Inventory Adjustment Flow

### Sales Order Processing
```
1. Order Created → Allocate Inventory
   - Reduces available quantity
   - Increases allocated quantity
   - On-hand remains same

2. Order Shipped → Ship Inventory
   - Reduces on-hand quantity
   - Reduces allocated quantity
   - Available remains same

3. Order Cancelled → Deallocate Inventory
   - Increases available quantity
   - Reduces allocated quantity
   - On-hand remains same

4. Order Returned → Return Inventory
   - Increases on-hand quantity
   - Increases available quantity
   - Allocated remains same
```

### Purchase Order Processing
```
1. PO Receipt → Receive Inventory
   - Increases on-hand quantity
   - Increases available quantity
   - Creates/updates inventory balance
```

## Database Schema

### Key Tables
- `item_master` - Item definitions
- `inventory_balances` - Current inventory levels by location
- `inventory_transactions` - Full audit trail
- `sales_order_lines` - Sales order details
- `purchase_order_lines` - Purchase order details

### Inventory Balance Fields
```sql
quantity_on_hand    -- Physical inventory
quantity_allocated  -- Reserved for orders
quantity_available  -- Available to promise (on_hand - allocated)
```

## Transaction Types
- `Receive` - From purchase orders
- `Ship` - To customers
- `Return` - From customers
- `Adjust` - Manual adjustments
- `Allocate` - Reserve for orders
- `Deallocate` - Release reservations
- `Transfer` - Between locations

## Testing

### Test Binary: `test_inventory_adjustments`
Located at: `src/bin/test_inventory_adjustments.rs`

Tests the full flow:
1. Creates test items in item master
2. Sets up warehouse locations
3. Initializes inventory balances
4. Tests sales order allocation/shipment
5. Tests purchase order receipt
6. Tests order cancellation/returns
7. Displays transaction history

### Running the Test
```bash
cargo build --bin test_inventory_adjustments
./target/debug/test_inventory_adjustments
```

## Example Usage

### Adjusting for Sales Order
```rust
let service = InventoryAdjustmentService::new(db_pool, event_sender);

// Allocate inventory when order is created
let results = service
    .adjust_for_sales_order(order_id, SalesOrderAdjustmentType::Allocate)
    .await?;

// Ship inventory when order ships
let results = service
    .adjust_for_sales_order(order_id, SalesOrderAdjustmentType::Ship)
    .await?;
```

### Receiving from Purchase Order
```rust
let receipt_lines = vec![
    PurchaseOrderReceiptLine {
        po_line_id: 123,
        quantity_received: Decimal::from(50),
        location_id: 1,
    },
];

let results = service
    .adjust_for_purchase_order_receipt(po_id, receipt_lines)
    .await?;
```

## Benefits

1. **Real-time Inventory Tracking** - Always know current stock levels
2. **Multi-location Support** - Track inventory across warehouses
3. **Full Audit Trail** - Every transaction is logged
4. **Event-Driven** - Integrates with other systems via events
5. **Atomic Operations** - All adjustments in transactions
6. **Type Safety** - Strongly typed with Rust

## Future Enhancements

1. **Lot/Serial Tracking** - Track specific batches
2. **Expiration Management** - Handle perishable goods
3. **Min/Max Alerts** - Automatic reorder points
4. **Cycle Count Integration** - Scheduled counts
5. **ABC Analysis** - Inventory classification
6. **Forecasting** - Demand prediction