#!/bin/bash

# Inventory Management Demo Script
# Demonstrates inventory operations including adjustments, allocations, transfers, and monitoring

set -e

API_URL="http://localhost:8080"
AUTH_TOKEN="${AUTH_TOKEN:-test-token}"

echo "========================================"
echo "StateSet API - Inventory Management Demo"
echo "========================================"
echo ""

# Helper function for API calls
api_call() {
    local method=$1
    local endpoint=$2
    local data=$3
    
    echo "→ $method $endpoint"
    
    if [ -z "$data" ]; then
        response=$(curl -s -X "$method" \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            "$API_URL$endpoint")
    else
        response=$(curl -s -X "$method" \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$API_URL$endpoint")
    fi
    
    echo "$response" | jq '.' 2>/dev/null || echo "$response"
    echo ""
}

# Step 1: Create a product
echo "Step 1: Creating a new product..."
echo "--------------------------------"
PRODUCT_DATA='{
  "name": "Wireless Bluetooth Headphones",
  "sku": "WBH-2024-001",
  "description": "Premium noise-cancelling wireless headphones",
  "price": 299.99,
  "cost": 120.00,
  "category": "Electronics",
  "weight": 0.35,
  "dimensions": {
    "length": 20,
    "width": 18,
    "height": 8
  }
}'
PRODUCT_ID=$(api_call POST "/api/products" "$PRODUCT_DATA" | jq -r '.id')
echo "✓ Product created with ID: $PRODUCT_ID"
echo ""

# Step 2: Set initial inventory levels at multiple warehouses
echo "Step 2: Setting initial inventory levels..."
echo "------------------------------------------"

# Warehouse 1 - Main Distribution Center
INVENTORY_DATA_1='{
  "product_id": "'$PRODUCT_ID'",
  "warehouse_id": "warehouse-001",
  "quantity": 1000,
  "location": "A-12-3",
  "reorder_point": 200,
  "reorder_quantity": 500
}'
api_call POST "/api/inventory/levels" "$INVENTORY_DATA_1"
echo "✓ Inventory set at Warehouse 001"

# Warehouse 2 - Regional Center
INVENTORY_DATA_2='{
  "product_id": "'$PRODUCT_ID'",
  "warehouse_id": "warehouse-002",
  "quantity": 500,
  "location": "B-05-2",
  "reorder_point": 100,
  "reorder_quantity": 300
}'
api_call POST "/api/inventory/levels" "$INVENTORY_DATA_2"
echo "✓ Inventory set at Warehouse 002"
echo ""

# Step 3: Check current inventory levels
echo "Step 3: Checking current inventory levels..."
echo "-------------------------------------------"
api_call GET "/api/inventory/products/$PRODUCT_ID"
echo ""

# Step 4: Perform inventory adjustment (damage/loss)
echo "Step 4: Adjusting inventory for damaged items..."
echo "-----------------------------------------------"
ADJUSTMENT_DATA='{
  "product_id": "'$PRODUCT_ID'",
  "warehouse_id": "warehouse-001",
  "quantity": -25,
  "reason": "DAMAGE",
  "notes": "Water damage from roof leak in section A-12"
}'
api_call POST "/api/inventory/adjustments" "$ADJUSTMENT_DATA"
echo "✓ Inventory adjusted for damaged items"
echo ""

# Step 5: Allocate inventory for an order
echo "Step 5: Allocating inventory for customer order..."
echo "-------------------------------------------------"
ALLOCATION_DATA='{
  "product_id": "'$PRODUCT_ID'",
  "warehouse_id": "warehouse-001",
  "quantity": 50,
  "order_id": "order-12345",
  "expires_at": "2025-01-01T00:00:00Z"
}'
ALLOCATION_ID=$(api_call POST "/api/inventory/allocations" "$ALLOCATION_DATA" | jq -r '.id')
echo "✓ Inventory allocated with ID: $ALLOCATION_ID"
echo ""

# Step 6: Transfer inventory between warehouses
echo "Step 6: Transferring inventory between warehouses..."
echo "--------------------------------------------------"
TRANSFER_DATA='{
  "product_id": "'$PRODUCT_ID'",
  "from_warehouse_id": "warehouse-001",
  "to_warehouse_id": "warehouse-002",
  "quantity": 100,
  "reason": "REBALANCING",
  "notes": "Rebalancing inventory for regional demand"
}'
TRANSFER_ID=$(api_call POST "/api/inventory/transfers" "$TRANSFER_DATA" | jq -r '.id')
echo "✓ Transfer initiated with ID: $TRANSFER_ID"
echo ""

# Step 7: Check low stock alerts
echo "Step 7: Checking for low stock alerts..."
echo "----------------------------------------"
api_call GET "/api/inventory/alerts?type=low_stock"
echo ""

# Step 8: Perform cycle count
echo "Step 8: Performing cycle count..."
echo "---------------------------------"
CYCLE_COUNT_DATA='{
  "warehouse_id": "warehouse-001",
  "items": [{
    "product_id": "'$PRODUCT_ID'",
    "counted_quantity": 823,
    "expected_quantity": 825,
    "location": "A-12-3"
  }]
}'
api_call POST "/api/inventory/cycle-counts" "$CYCLE_COUNT_DATA"
echo "✓ Cycle count completed"
echo ""

# Step 9: Generate inventory report
echo "Step 9: Generating inventory movement report..."
echo "----------------------------------------------"
REPORT_PARAMS="start_date=2025-01-01&end_date=2025-12-31&product_id=$PRODUCT_ID"
api_call GET "/api/reports/inventory/movements?$REPORT_PARAMS"
echo ""

# Step 10: Check inventory valuation
echo "Step 10: Checking inventory valuation..."
echo "----------------------------------------"
api_call GET "/api/inventory/valuation?warehouse_id=all"
echo ""

# Step 11: Release the allocation (simulate order cancellation)
echo "Step 11: Releasing inventory allocation..."
echo "-----------------------------------------"
api_call DELETE "/api/inventory/allocations/$ALLOCATION_ID"
echo "✓ Allocation released"
echo ""

# Step 12: Set up automatic reorder
echo "Step 12: Setting up automatic reorder rule..."
echo "--------------------------------------------"
REORDER_RULE='{
  "product_id": "'$PRODUCT_ID'",
  "warehouse_id": "warehouse-001",
  "enabled": true,
  "reorder_point": 200,
  "reorder_quantity": 500,
  "preferred_supplier_id": "supplier-001",
  "lead_time_days": 7
}'
api_call POST "/api/inventory/reorder-rules" "$REORDER_RULE"
echo "✓ Automatic reorder rule created"
echo ""

echo "========================================"
echo "Inventory Management Demo Complete!"
echo "========================================"
echo ""
echo "Summary of operations:"
echo "- Created product: $PRODUCT_ID"
echo "- Set inventory at 2 warehouses"
echo "- Adjusted inventory for damage"
echo "- Allocated inventory for order"
echo "- Transferred inventory between warehouses"
echo "- Performed cycle count"
echo "- Generated movement reports"
echo "- Set up automatic reorder rules"
echo "" 