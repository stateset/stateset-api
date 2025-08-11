#!/bin/bash

# Shipment Tracking Demo Script
# Demonstrates order fulfillment, multi-carrier shipping, and real-time tracking

set -e

API_URL="http://localhost:8080"
AUTH_TOKEN="${AUTH_TOKEN:-test-token}"

echo "========================================="
echo "StateSet API - Shipment Tracking Demo"
echo "========================================="
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

# Step 1: Create customers
echo "Step 1: Creating customers..."
echo "-----------------------------"

# Customer 1 - Standard shipping
CUSTOMER1_DATA='{
  "name": "Alice Thompson",
  "email": "alice.thompson@example.com",
  "phone": "+1-555-100-2000",
  "type": "RETAIL",
  "address": {
    "street": "789 Oak Street",
    "city": "Portland",
    "state": "OR",
    "zip": "97201",
    "country": "US"
  }
}'
CUSTOMER1_ID=$(api_call POST "/api/customers" "$CUSTOMER1_DATA" | jq -r '.id')
echo "✓ Customer 1 created: $CUSTOMER1_ID"

# Customer 2 - Express shipping
CUSTOMER2_DATA='{
  "name": "TechCorp Industries",
  "email": "orders@techcorp.com",
  "phone": "+1-555-200-3000",
  "type": "B2B",
  "address": {
    "street": "500 Enterprise Blvd",
    "city": "Austin",
    "state": "TX",
    "zip": "78701",
    "country": "US"
  }
}'
CUSTOMER2_ID=$(api_call POST "/api/customers" "$CUSTOMER2_DATA" | jq -r '.id')
echo "✓ Customer 2 created: $CUSTOMER2_ID"
echo ""

# Step 2: Create orders
echo "Step 2: Creating orders..."
echo "--------------------------"

# Order 1 - Multiple items, standard shipping
ORDER1_DATA='{
  "customer_id": "'$CUSTOMER1_ID'",
  "items": [
    {
      "product_id": "prod-tablet-001",
      "product_name": "Smart Tablet 10\"",
      "quantity": 1,
      "price": 399.99
    },
    {
      "product_id": "prod-case-001",
      "product_name": "Tablet Protective Case",
      "quantity": 1,
      "price": 29.99
    },
    {
      "product_id": "prod-stylus-001",
      "product_name": "Digital Stylus",
      "quantity": 2,
      "price": 49.99
    }
  ],
  "shipping_address": {
    "street": "789 Oak Street",
    "city": "Portland",
    "state": "OR",
    "zip": "97201",
    "country": "US"
  },
  "shipping_method": "STANDARD",
  "priority": "NORMAL"
}'
ORDER1_ID=$(api_call POST "/api/orders" "$ORDER1_DATA" | jq -r '.id')
echo "✓ Order 1 created: $ORDER1_ID"

# Order 2 - Express shipping, high priority
ORDER2_DATA='{
  "customer_id": "'$CUSTOMER2_ID'",
  "items": [
    {
      "product_id": "prod-server-001",
      "product_name": "Enterprise Server Unit",
      "quantity": 2,
      "price": 4999.99
    }
  ],
  "shipping_address": {
    "street": "500 Enterprise Blvd",
    "city": "Austin",
    "state": "TX",
    "zip": "78701",
    "country": "US"
  },
  "shipping_method": "EXPRESS",
  "priority": "HIGH",
  "special_instructions": "Call receiving at 555-200-3001 before delivery"
}'
ORDER2_ID=$(api_call POST "/api/orders" "$ORDER2_DATA" | jq -r '.id')
echo "✓ Order 2 created: $ORDER2_ID"
echo ""

# Step 3: Allocate inventory and create pick lists
echo "Step 3: Creating pick lists..."
echo "------------------------------"

# Pick list for Order 1
PICK1_DATA='{
  "order_id": "'$ORDER1_ID'",
  "warehouse_id": "warehouse-west",
  "picker_id": "picker-001",
  "items": [
    {
      "product_id": "prod-tablet-001",
      "quantity": 1,
      "location": "W-A-15-3",
      "lot_number": "LOT-TAB-2024"
    },
    {
      "product_id": "prod-case-001",
      "quantity": 1,
      "location": "W-B-02-1"
    },
    {
      "product_id": "prod-stylus-001",
      "quantity": 2,
      "location": "W-C-08-5"
    }
  ]
}'
PICK1_ID=$(api_call POST "/api/warehouse/pick-lists" "$PICK1_DATA" | jq -r '.id')
echo "✓ Pick list 1 created: $PICK1_ID"

# Pick list for Order 2
PICK2_DATA='{
  "order_id": "'$ORDER2_ID'",
  "warehouse_id": "warehouse-central",
  "picker_id": "picker-002",
  "priority": "HIGH",
  "items": [
    {
      "product_id": "prod-server-001",
      "quantity": 2,
      "location": "C-BULK-01",
      "serial_numbers": ["SRV-2024-0100", "SRV-2024-0101"]
    }
  ]
}'
PICK2_ID=$(api_call POST "/api/warehouse/pick-lists" "$PICK2_DATA" | jq -r '.id')
echo "✓ Pick list 2 created: $PICK2_ID"
echo ""

# Step 4: Complete picking
echo "Step 4: Completing picking process..."
echo "------------------------------------"

# Complete picking for Order 1
PICK_COMPLETE1='{
  "pick_list_id": "'$PICK1_ID'",
  "completed_by": "picker-001",
  "items_picked": [
    {"product_id": "prod-tablet-001", "quantity": 1, "scanned": true},
    {"product_id": "prod-case-001", "quantity": 1, "scanned": true},
    {"product_id": "prod-stylus-001", "quantity": 2, "scanned": true}
  ],
  "time_taken_minutes": 12
}'
api_call POST "/api/warehouse/pick-lists/$PICK1_ID/complete" "$PICK_COMPLETE1"
echo "✓ Picking completed for Order 1"

# Complete picking for Order 2
PICK_COMPLETE2='{
  "pick_list_id": "'$PICK2_ID'",
  "completed_by": "picker-002",
  "items_picked": [
    {"product_id": "prod-server-001", "quantity": 2, "serial_numbers": ["SRV-2024-0100", "SRV-2024-0101"]}
  ],
  "time_taken_minutes": 25
}'
api_call POST "/api/warehouse/pick-lists/$PICK2_ID/complete" "$PICK_COMPLETE2"
echo "✓ Picking completed for Order 2"
echo ""

# Step 5: Pack orders
echo "Step 5: Packing orders..."
echo "-------------------------"

# Pack Order 1
PACK1_DATA='{
  "order_id": "'$ORDER1_ID'",
  "packed_by": "packer-001",
  "packages": [
    {
      "box_type": "MEDIUM_BOX",
      "weight_lbs": 3.5,
      "dimensions": {
        "length": 12,
        "width": 10,
        "height": 6
      },
      "items": [
        {"product_id": "prod-tablet-001", "quantity": 1},
        {"product_id": "prod-case-001", "quantity": 1},
        {"product_id": "prod-stylus-001", "quantity": 2}
      ],
      "packing_materials": ["bubble_wrap", "air_pillows"]
    }
  ]
}'
PACK1_ID=$(api_call POST "/api/warehouse/packing" "$PACK1_DATA" | jq -r '.id')
echo "✓ Order 1 packed: $PACK1_ID"

# Pack Order 2
PACK2_DATA='{
  "order_id": "'$ORDER2_ID'",
  "packed_by": "packer-002",
  "packages": [
    {
      "box_type": "PALLET",
      "weight_lbs": 120,
      "dimensions": {
        "length": 48,
        "width": 40,
        "height": 36
      },
      "items": [
        {"product_id": "prod-server-001", "quantity": 2, "serial_numbers": ["SRV-2024-0100", "SRV-2024-0101"]}
      ],
      "packing_materials": ["foam_corners", "stretch_wrap"],
      "special_handling": ["FRAGILE", "THIS_SIDE_UP"]
    }
  ]
}'
PACK2_ID=$(api_call POST "/api/warehouse/packing" "$PACK2_DATA" | jq -r '.id')
echo "✓ Order 2 packed: $PACK2_ID"
echo ""

# Step 6: Create shipments and get rates
echo "Step 6: Creating shipments and comparing rates..."
echo "-------------------------------------------------"

# Get shipping rates for Order 1
RATE_REQUEST1='{
  "from_address": {
    "street": "100 Warehouse Way",
    "city": "Portland",
    "state": "OR",
    "zip": "97210",
    "country": "US"
  },
  "to_address": {
    "street": "789 Oak Street",
    "city": "Portland",
    "state": "OR",
    "zip": "97201",
    "country": "US"
  },
  "packages": [{
    "weight_lbs": 3.5,
    "length": 12,
    "width": 10,
    "height": 6
  }],
  "carriers": ["FEDEX", "UPS", "USPS"]
}'
echo "Getting shipping rates for Order 1..."
api_call POST "/api/shipping/rates" "$RATE_REQUEST1"

# Create shipment for Order 1
SHIPMENT1_DATA='{
  "order_id": "'$ORDER1_ID'",
  "carrier": "FEDEX",
  "service": "GROUND",
  "packages": [{
    "tracking_number": "FDX-1234567890",
    "weight_lbs": 3.5,
    "dimensions": {"length": 12, "width": 10, "height": 6}
  }],
  "ship_date": "'$(date -u +"%Y-%m-%d")'",
  "estimated_delivery": "'$(date -u -d "+3 days" +"%Y-%m-%d")'",
  "cost": 12.99,
  "insurance_amount": 479.97
}'
SHIPMENT1_ID=$(api_call POST "/api/shipments" "$SHIPMENT1_DATA" | jq -r '.id')
echo "✓ Shipment 1 created: $SHIPMENT1_ID"

# Create shipment for Order 2 (Express)
SHIPMENT2_DATA='{
  "order_id": "'$ORDER2_ID'",
  "carrier": "UPS",
  "service": "NEXT_DAY_AIR",
  "packages": [{
    "tracking_number": "1Z999AA10123456784",
    "weight_lbs": 120,
    "dimensions": {"length": 48, "width": 40, "height": 36}
  }],
  "ship_date": "'$(date -u +"%Y-%m-%d")'",
  "estimated_delivery": "'$(date -u -d "+1 day" +"%Y-%m-%d")'",
  "cost": 285.00,
  "insurance_amount": 9999.98,
  "special_services": ["SIGNATURE_REQUIRED", "SATURDAY_DELIVERY"]
}'
SHIPMENT2_ID=$(api_call POST "/api/shipments" "$SHIPMENT2_DATA" | jq -r '.id')
echo "✓ Shipment 2 created: $SHIPMENT2_ID"
echo ""

# Step 7: Print shipping labels
echo "Step 7: Generating shipping labels..."
echo "------------------------------------"
api_call POST "/api/shipments/$SHIPMENT1_ID/label" '{"format": "PDF", "size": "4x6"}'
echo "✓ Label generated for Shipment 1"

api_call POST "/api/shipments/$SHIPMENT2_ID/label" '{"format": "PDF", "size": "4x6"}'
echo "✓ Label generated for Shipment 2"
echo ""

# Step 8: Update tracking information
echo "Step 8: Updating tracking information..."
echo "----------------------------------------"

# Shipment 1 - Picked up
TRACKING_UPDATE1='{
  "status": "IN_TRANSIT",
  "location": "Portland, OR",
  "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Package picked up by FEDEX",
  "carrier_status_code": "PU"
}'
api_call POST "/api/shipments/$SHIPMENT1_ID/tracking" "$TRACKING_UPDATE1"
echo "✓ Tracking updated: Shipment 1 picked up"

# Shipment 2 - Picked up
TRACKING_UPDATE2='{
  "status": "IN_TRANSIT",
  "location": "Austin, TX",
  "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Package picked up by UPS",
  "carrier_status_code": "PU"
}'
api_call POST "/api/shipments/$SHIPMENT2_ID/tracking" "$TRACKING_UPDATE2"
echo "✓ Tracking updated: Shipment 2 picked up"
echo ""

# Step 9: Simulate tracking events
echo "Step 9: Simulating tracking events..."
echo "-------------------------------------"

# Shipment 1 - In transit
sleep 2
TRACKING_UPDATE3='{
  "status": "IN_TRANSIT",
  "location": "Portland Distribution Center, OR",
  "timestamp": "'$(date -u -d "+2 hours" +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Departed FedEx location",
  "carrier_status_code": "DP"
}'
api_call POST "/api/shipments/$SHIPMENT1_ID/tracking" "$TRACKING_UPDATE3"

# Shipment 2 - Out for delivery
TRACKING_UPDATE4='{
  "status": "OUT_FOR_DELIVERY",
  "location": "Austin, TX",
  "timestamp": "'$(date -u -d "+1 day" +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Out for delivery",
  "carrier_status_code": "OD",
  "delivery_attempt": 1
}'
api_call POST "/api/shipments/$SHIPMENT2_ID/tracking" "$TRACKING_UPDATE4"
echo "✓ Tracking events simulated"
echo ""

# Step 10: Handle delivery
echo "Step 10: Processing deliveries..."
echo "---------------------------------"

# Delivery attempt for Shipment 2
DELIVERY_ATTEMPT='{
  "status": "DELIVERY_ATTEMPTED",
  "location": "500 Enterprise Blvd, Austin, TX",
  "timestamp": "'$(date -u -d "+1 day +4 hours" +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Delivery attempted - business closed",
  "carrier_status_code": "A1",
  "notes": "Will retry next business day"
}'
api_call POST "/api/shipments/$SHIPMENT2_ID/tracking" "$DELIVERY_ATTEMPT"
echo "✓ Delivery attempted for Shipment 2"

# Successful delivery for Shipment 2
DELIVERY_SUCCESS2='{
  "status": "DELIVERED",
  "location": "500 Enterprise Blvd, Austin, TX",
  "timestamp": "'$(date -u -d "+2 days" +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Delivered to receiving dock",
  "carrier_status_code": "DL",
  "signature": "J.Smith",
  "delivery_photo_url": "proof_of_delivery_12345.jpg"
}'
api_call POST "/api/shipments/$SHIPMENT2_ID/tracking" "$DELIVERY_SUCCESS2"
echo "✓ Shipment 2 delivered"

# Successful delivery for Shipment 1
DELIVERY_SUCCESS1='{
  "status": "DELIVERED",
  "location": "789 Oak Street, Portland, OR",
  "timestamp": "'$(date -u -d "+3 days" +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Delivered to front door",
  "carrier_status_code": "DL",
  "delivery_photo_url": "proof_of_delivery_67890.jpg"
}'
api_call POST "/api/shipments/$SHIPMENT1_ID/tracking" "$DELIVERY_SUCCESS1"
echo "✓ Shipment 1 delivered"
echo ""

# Step 11: Send delivery notifications
echo "Step 11: Sending delivery notifications..."
echo "------------------------------------------"
NOTIFICATION1='{
  "type": "DELIVERY_CONFIRMATION",
  "recipient": "'$CUSTOMER1_ID'",
  "shipment_id": "'$SHIPMENT1_ID'",
  "channels": ["EMAIL", "SMS"],
  "data": {
    "order_id": "'$ORDER1_ID'",
    "tracking_number": "FDX-1234567890",
    "delivery_photo": "proof_of_delivery_67890.jpg"
  }
}'
api_call POST "/api/notifications/send" "$NOTIFICATION1"
echo "✓ Delivery notification sent to Customer 1"

NOTIFICATION2='{
  "type": "DELIVERY_CONFIRMATION",
  "recipient": "'$CUSTOMER2_ID'",
  "shipment_id": "'$SHIPMENT2_ID'",
  "channels": ["EMAIL"],
  "data": {
    "order_id": "'$ORDER2_ID'",
    "tracking_number": "1Z999AA10123456784",
    "signature": "J.Smith",
    "delivery_photo": "proof_of_delivery_12345.jpg"
  }
}'
api_call POST "/api/notifications/send" "$NOTIFICATION2"
echo "✓ Delivery notification sent to Customer 2"
echo ""

# Step 12: Handle exceptions
echo "Step 12: Demonstrating exception handling..."
echo "--------------------------------------------"

# Create a problem shipment
PROBLEM_ORDER='{
  "customer_id": "'$CUSTOMER1_ID'",
  "items": [{
    "product_id": "prod-monitor-001",
    "product_name": "4K Monitor",
    "quantity": 1,
    "price": 599.99
  }],
  "shipping_address": {
    "street": "999 Invalid Address",
    "city": "Portland",
    "state": "OR",
    "zip": "00000",
    "country": "US"
  }
}'
PROBLEM_ORDER_ID=$(api_call POST "/api/orders" "$PROBLEM_ORDER" | jq -r '.id')

PROBLEM_SHIPMENT='{
  "order_id": "'$PROBLEM_ORDER_ID'",
  "carrier": "UPS",
  "service": "GROUND",
  "packages": [{
    "tracking_number": "1Z999BB20987654321",
    "weight_lbs": 15
  }]
}'
PROBLEM_SHIP_ID=$(api_call POST "/api/shipments" "$PROBLEM_SHIPMENT" | jq -r '.id')

# Report exception
EXCEPTION_DATA='{
  "status": "EXCEPTION",
  "location": "Portland, OR",
  "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "description": "Address not found - unable to deliver",
  "carrier_status_code": "X1",
  "exception_type": "INVALID_ADDRESS",
  "action_required": true
}'
api_call POST "/api/shipments/$PROBLEM_SHIP_ID/tracking" "$EXCEPTION_DATA"
echo "✓ Delivery exception reported"
echo ""

# Step 13: Generate shipping analytics
echo "Step 13: Generating shipping analytics..."
echo "-----------------------------------------"
api_call GET "/api/analytics/shipping?period=last_30_days&metrics=delivery_performance,carrier_costs,exception_rate"
echo ""

# Step 14: Check shipping performance
echo "Step 14: Checking shipping performance..."
echo "-----------------------------------------"
PERFORMANCE_DATA='{
  "carriers": ["FEDEX", "UPS", "USPS"],
  "date_range": {
    "start": "'$(date -u -d "-30 days" +"%Y-%m-%d")'",
    "end": "'$(date -u +"%Y-%m-%d")'"
  },
  "metrics": ["on_time_delivery", "average_transit_time", "damage_rate"]
}'
api_call POST "/api/reports/shipping-performance" "$PERFORMANCE_DATA"
echo ""

echo "========================================="
echo "Shipment Tracking Demo Complete!"
echo "========================================="
echo ""
echo "Summary of operations:"
echo "- Created 2 orders with different shipping priorities"
echo "- Processed picking and packing"
echo "- Created shipments with FedEx and UPS"
echo "- Tracked packages through delivery"
echo "- Handled delivery exception"
echo "- Generated shipping analytics"
echo ""
echo "Shipment IDs:"
echo "- Standard shipping: $SHIPMENT1_ID (Delivered)"
echo "- Express shipping: $SHIPMENT2_ID (Delivered)"
echo "- Exception case: $PROBLEM_SHIP_ID (Address issue)"
echo "" 