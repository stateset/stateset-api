#!/bin/bash

# Returns Processing Demo Script
# Demonstrates the complete return merchandise authorization (RMA) workflow

set -e

API_URL="http://localhost:8080"
AUTH_TOKEN="${AUTH_TOKEN:-test-token}"

echo "=========================================="
echo "StateSet API - Returns Processing Demo"
echo "=========================================="
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

# Step 1: Create a customer
echo "Step 1: Creating a customer..."
echo "------------------------------"
CUSTOMER_DATA='{
  "name": "Sarah Johnson",
  "email": "sarah.johnson@example.com",
  "phone": "+1-555-123-4567",
  "address": {
    "street": "123 Return Lane",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94105",
    "country": "US"
  }
}'
CUSTOMER_ID=$(api_call POST "/api/customers" "$CUSTOMER_DATA" | jq -r '.id')
echo "✓ Customer created with ID: $CUSTOMER_ID"
echo ""

# Step 2: Create an order (to return from)
echo "Step 2: Creating an order..."
echo "----------------------------"
ORDER_DATA='{
  "customer_id": "'$CUSTOMER_ID'",
  "items": [
    {
      "product_id": "prod-laptop-001",
      "product_name": "Premium Laptop 15\"",
      "quantity": 1,
      "price": 1299.99
    },
    {
      "product_id": "prod-mouse-001",
      "product_name": "Wireless Mouse",
      "quantity": 2,
      "price": 49.99
    }
  ],
  "shipping_address": {
    "street": "123 Return Lane",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94105",
    "country": "US"
  },
  "status": "DELIVERED",
  "delivered_at": "2024-12-20T10:00:00Z"
}'
ORDER_ID=$(api_call POST "/api/orders" "$ORDER_DATA" | jq -r '.id')
echo "✓ Order created with ID: $ORDER_ID"
echo ""

# Step 3: Initiate a return request
echo "Step 3: Initiating return request..."
echo "------------------------------------"
RETURN_REQUEST_DATA='{
  "order_id": "'$ORDER_ID'",
  "customer_id": "'$CUSTOMER_ID'",
  "reason": "DEFECTIVE",
  "items": [
    {
      "product_id": "prod-laptop-001",
      "quantity": 1,
      "reason": "Screen has dead pixels",
      "condition": "DAMAGED"
    }
  ],
  "customer_notes": "The laptop screen has multiple dead pixels. Noticed after 3 days of use.",
  "preferred_resolution": "REPLACEMENT"
}'
RETURN_ID=$(api_call POST "/api/returns" "$RETURN_REQUEST_DATA" | jq -r '.id')
echo "✓ Return request created with ID: $RETURN_ID"
echo ""

# Step 4: Generate RMA number and shipping label
echo "Step 4: Generating RMA number and return label..."
echo "------------------------------------------------"
RMA_DATA='{
  "return_id": "'$RETURN_ID'",
  "warehouse_id": "warehouse-returns-001",
  "shipping_method": "PREPAID_LABEL"
}'
RMA_RESPONSE=$(api_call POST "/api/returns/$RETURN_ID/rma" "$RMA_DATA")
RMA_NUMBER=$(echo "$RMA_RESPONSE" | jq -r '.rma_number')
echo "✓ RMA Number: $RMA_NUMBER"
echo ""

# Step 5: Update return status to approved
echo "Step 5: Approving the return..."
echo "-------------------------------"
APPROVAL_DATA='{
  "status": "APPROVED",
  "internal_notes": "Return approved. Customer provided valid reason with photo evidence.",
  "approved_by": "support-agent-001"
}'
api_call PATCH "/api/returns/$RETURN_ID/status" "$APPROVAL_DATA"
echo "✓ Return approved"
echo ""

# Step 6: Generate return shipping label
echo "Step 6: Generating return shipping label..."
echo "------------------------------------------"
LABEL_DATA='{
  "carrier": "FEDEX",
  "service_type": "GROUND",
  "from_address": {
    "name": "Sarah Johnson",
    "street": "123 Return Lane",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94105",
    "country": "US"
  },
  "to_address": {
    "name": "StateSet Returns Center",
    "street": "456 Warehouse Blvd",
    "city": "Los Angeles",
    "state": "CA",
    "zip": "90001",
    "country": "US"
  }
}'
LABEL_RESPONSE=$(api_call POST "/api/returns/$RETURN_ID/shipping-label" "$LABEL_DATA")
TRACKING_NUMBER=$(echo "$LABEL_RESPONSE" | jq -r '.tracking_number')
echo "✓ Shipping label created with tracking: $TRACKING_NUMBER"
echo ""

# Step 7: Simulate package in transit
echo "Step 7: Updating return shipment status..."
echo "------------------------------------------"
TRANSIT_DATA='{
  "status": "IN_TRANSIT",
  "tracking_number": "'$TRACKING_NUMBER'",
  "carrier_status": "PICKED_UP",
  "estimated_delivery": "2025-01-05T18:00:00Z"
}'
api_call PATCH "/api/returns/$RETURN_ID/shipment" "$TRANSIT_DATA"
echo "✓ Return package in transit"
echo ""

# Step 8: Receive returned items at warehouse
echo "Step 8: Receiving returned items..."
echo "-----------------------------------"
RECEIVING_DATA='{
  "received_by": "warehouse-staff-001",
  "received_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "items": [
    {
      "product_id": "prod-laptop-001",
      "quantity_received": 1,
      "condition": "DAMAGED",
      "inspection_notes": "Confirmed - screen has 5 dead pixels in upper right corner",
      "disposition": "DEFECTIVE"
    }
  ],
  "warehouse_id": "warehouse-returns-001"
}'
api_call POST "/api/returns/$RETURN_ID/receive" "$RECEIVING_DATA"
echo "✓ Items received at warehouse"
echo ""

# Step 9: Quality inspection
echo "Step 9: Performing quality inspection..."
echo "---------------------------------------"
INSPECTION_DATA='{
  "inspector_id": "qc-inspector-001",
  "inspection_date": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "items": [
    {
      "product_id": "prod-laptop-001",
      "passed": false,
      "defect_type": "SCREEN_DAMAGE",
      "defect_severity": "MAJOR",
      "photos": ["dead-pixels-photo-1.jpg", "dead-pixels-photo-2.jpg"],
      "recommendation": "APPROVE_REPLACEMENT"
    }
  ]
}'
api_call POST "/api/returns/$RETURN_ID/inspection" "$INSPECTION_DATA"
echo "✓ Quality inspection completed"
echo ""

# Step 10: Process replacement
echo "Step 10: Processing replacement order..."
echo "----------------------------------------"
REPLACEMENT_DATA='{
  "return_id": "'$RETURN_ID'",
  "type": "REPLACEMENT",
  "items": [
    {
      "original_product_id": "prod-laptop-001",
      "replacement_product_id": "prod-laptop-001",
      "quantity": 1
    }
  ],
  "expedited_shipping": true,
  "notes": "Customer approved for replacement due to screen defect"
}'
REPLACEMENT_ORDER=$(api_call POST "/api/returns/$RETURN_ID/resolution" "$REPLACEMENT_DATA" | jq -r '.replacement_order_id')
echo "✓ Replacement order created: $REPLACEMENT_ORDER"
echo ""

# Step 11: Update inventory for defective item
echo "Step 11: Updating inventory for defective item..."
echo "-------------------------------------------------"
DEFECTIVE_INVENTORY='{
  "product_id": "prod-laptop-001",
  "warehouse_id": "warehouse-returns-001",
  "quantity": 1,
  "status": "DEFECTIVE",
  "location": "DEFECTIVE-BIN-03",
  "notes": "Dead pixels - RMA: '$RMA_NUMBER'"
}'
api_call POST "/api/inventory/defective" "$DEFECTIVE_INVENTORY"
echo "✓ Defective inventory updated"
echo ""

# Step 12: Send notifications
echo "Step 12: Sending customer notifications..."
echo "------------------------------------------"
NOTIFICATION_DATA='{
  "type": "RETURN_UPDATE",
  "recipient": "'$CUSTOMER_ID'",
  "channel": "EMAIL",
  "template": "return_replacement_shipped",
  "data": {
    "customer_name": "Sarah Johnson",
    "rma_number": "'$RMA_NUMBER'",
    "replacement_order_id": "'$REPLACEMENT_ORDER'",
    "tracking_number": "NEW-TRACKING-12345"
  }
}'
api_call POST "/api/notifications/send" "$NOTIFICATION_DATA"
echo "✓ Customer notification sent"
echo ""

# Step 13: Complete the return
echo "Step 13: Completing the return process..."
echo "-----------------------------------------"
COMPLETION_DATA='{
  "status": "COMPLETED",
  "resolution": "REPLACEMENT_SENT",
  "completed_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "final_notes": "Customer received replacement. Original defective unit processed for vendor return."
}'
api_call PATCH "/api/returns/$RETURN_ID/complete" "$COMPLETION_DATA"
echo "✓ Return process completed"
echo ""

# Step 14: Generate return metrics
echo "Step 14: Generating return analytics..."
echo "---------------------------------------"
api_call GET "/api/analytics/returns?period=last_30_days"
echo ""

# Step 15: Check customer return history
echo "Step 15: Checking customer return history..."
echo "--------------------------------------------"
api_call GET "/api/customers/$CUSTOMER_ID/returns"
echo ""

echo "=========================================="
echo "Returns Processing Demo Complete!"
echo "=========================================="
echo ""
echo "Summary of operations:"
echo "- Created return request: $RETURN_ID"
echo "- Generated RMA number: $RMA_NUMBER"
echo "- Approved and processed return"
echo "- Received and inspected items"
echo "- Created replacement order: $REPLACEMENT_ORDER"
echo "- Updated inventory for defective items"
echo "- Sent customer notifications"
echo "- Generated analytics and reports"
echo "" 