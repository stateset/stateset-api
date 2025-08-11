#!/bin/bash

# Warranty Claim Demo Script
# Demonstrates warranty registration, claim filing, and resolution process

set -e

API_URL="http://localhost:8080"
AUTH_TOKEN="${AUTH_TOKEN:-test-token}"

echo "======================================="
echo "StateSet API - Warranty Claim Demo"
echo "======================================="
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
  "name": "Michael Chen",
  "email": "michael.chen@example.com",
  "phone": "+1-555-987-6543",
  "address": {
    "street": "456 Tech Drive",
    "city": "Seattle",
    "state": "WA",
    "zip": "98101",
    "country": "US"
  }
}'
CUSTOMER_ID=$(api_call POST "/api/customers" "$CUSTOMER_DATA" | jq -r '.id')
echo "✓ Customer created with ID: $CUSTOMER_ID"
echo ""

# Step 2: Create a product with warranty information
echo "Step 2: Creating a product with warranty..."
echo "------------------------------------------"
PRODUCT_DATA='{
  "name": "Smart Home Security Camera",
  "sku": "CAM-SEC-2024",
  "description": "4K HD WiFi Security Camera with Night Vision",
  "price": 249.99,
  "category": "Electronics",
  "warranty_period_months": 24,
  "extended_warranty_available": true,
  "manufacturer": "TechVision Inc"
}'
PRODUCT_ID=$(api_call POST "/api/products" "$PRODUCT_DATA" | jq -r '.id')
echo "✓ Product created with ID: $PRODUCT_ID"
echo ""

# Step 3: Create an order
echo "Step 3: Creating an order..."
echo "----------------------------"
ORDER_DATA='{
  "customer_id": "'$CUSTOMER_ID'",
  "items": [
    {
      "product_id": "'$PRODUCT_ID'",
      "product_name": "Smart Home Security Camera",
      "quantity": 1,
      "price": 249.99,
      "serial_number": "TVSC2024-789456"
    }
  ],
  "shipping_address": {
    "street": "456 Tech Drive",
    "city": "Seattle",
    "state": "WA",
    "zip": "98101",
    "country": "US"
  },
  "status": "DELIVERED",
  "delivered_at": "2024-01-15T10:00:00Z"
}'
ORDER_ID=$(api_call POST "/api/orders" "$ORDER_DATA" | jq -r '.id')
echo "✓ Order created with ID: $ORDER_ID"
echo ""

# Step 4: Register warranty
echo "Step 4: Registering product warranty..."
echo "---------------------------------------"
WARRANTY_REG_DATA='{
  "customer_id": "'$CUSTOMER_ID'",
  "product_id": "'$PRODUCT_ID'",
  "order_id": "'$ORDER_ID'",
  "serial_number": "TVSC2024-789456",
  "purchase_date": "2024-01-15",
  "warranty_type": "STANDARD",
  "duration_months": 24,
  "start_date": "2024-01-15",
  "end_date": "2026-01-15",
  "registration_method": "ONLINE",
  "proof_of_purchase": "order-'$ORDER_ID'-receipt.pdf"
}'
WARRANTY_ID=$(api_call POST "/api/warranties/register" "$WARRANTY_REG_DATA" | jq -r '.id')
echo "✓ Warranty registered with ID: $WARRANTY_ID"
echo ""

# Step 5: Check warranty status
echo "Step 5: Checking warranty status..."
echo "-----------------------------------"
api_call GET "/api/warranties/$WARRANTY_ID/status"
echo ""

# Step 6: File a warranty claim
echo "Step 6: Filing warranty claim..."
echo "--------------------------------"
CLAIM_DATA='{
  "warranty_id": "'$WARRANTY_ID'",
  "customer_id": "'$CUSTOMER_ID'",
  "product_id": "'$PRODUCT_ID'",
  "serial_number": "TVSC2024-789456",
  "issue_description": "Camera stopped connecting to WiFi after 3 months. LED indicator flashing red continuously.",
  "issue_date": "2024-04-20",
  "category": "MALFUNCTION",
  "severity": "HIGH",
  "customer_troubleshooting": [
    "Reset camera to factory settings",
    "Updated firmware to latest version",
    "Checked router settings and WiFi password",
    "Tried connecting to different network"
  ],
  "preferred_resolution": "REPAIR",
  "contact_preference": "EMAIL"
}'
CLAIM_ID=$(api_call POST "/api/warranties/claims" "$CLAIM_DATA" | jq -r '.id')
echo "✓ Warranty claim filed with ID: $CLAIM_ID"
echo ""

# Step 7: Upload supporting documents
echo "Step 7: Uploading supporting documents..."
echo "-----------------------------------------"
DOC_DATA='{
  "claim_id": "'$CLAIM_ID'",
  "documents": [
    {
      "type": "PHOTO",
      "name": "camera_led_indicator.jpg",
      "description": "Photo showing red flashing LED"
    },
    {
      "type": "VIDEO",
      "name": "connection_attempt.mp4",
      "description": "Video of failed WiFi connection attempts"
    }
  ]
}'
api_call POST "/api/warranties/claims/$CLAIM_ID/documents" "$DOC_DATA"
echo "✓ Supporting documents uploaded"
echo ""

# Step 8: Review and approve claim
echo "Step 8: Reviewing warranty claim..."
echo "-----------------------------------"
REVIEW_DATA='{
  "reviewed_by": "warranty-specialist-001",
  "review_notes": "Valid warranty claim. Device malfunction confirmed. Customer attempted all basic troubleshooting.",
  "status": "APPROVED",
  "resolution_type": "REPLACEMENT",
  "reason": "Manufacturing defect - WiFi module failure"
}'
api_call PATCH "/api/warranties/claims/$CLAIM_ID/review" "$REVIEW_DATA"
echo "✓ Warranty claim approved"
echo ""

# Step 9: Generate RMA for warranty claim
echo "Step 9: Generating RMA for warranty return..."
echo "---------------------------------------------"
WARRANTY_RMA_DATA='{
  "claim_id": "'$CLAIM_ID'",
  "return_required": true,
  "prepaid_shipping": true,
  "shipping_carrier": "UPS",
  "instructions": "Please include original accessories and pack securely"
}'
RMA_RESPONSE=$(api_call POST "/api/warranties/claims/$CLAIM_ID/rma" "$WARRANTY_RMA_DATA")
RMA_NUMBER=$(echo "$RMA_RESPONSE" | jq -r '.rma_number')
echo "✓ RMA generated: $RMA_NUMBER"
echo ""

# Step 10: Schedule replacement shipment
echo "Step 10: Scheduling replacement shipment..."
echo "------------------------------------------"
REPLACEMENT_DATA='{
  "claim_id": "'$CLAIM_ID'",
  "replacement_product_id": "'$PRODUCT_ID'",
  "new_serial_number": "TVSC2024-NEW-123456",
  "ship_after_return_received": false,
  "expedited_shipping": true,
  "shipping_address": {
    "street": "456 Tech Drive",
    "city": "Seattle",
    "state": "WA",
    "zip": "98101",
    "country": "US"
  }
}'
REPLACEMENT_ORDER=$(api_call POST "/api/warranties/claims/$CLAIM_ID/replacement" "$REPLACEMENT_DATA" | jq -r '.order_id')
echo "✓ Replacement order created: $REPLACEMENT_ORDER"
echo ""

# Step 11: Update warranty for replacement unit
echo "Step 11: Transferring warranty to replacement..."
echo "------------------------------------------------"
WARRANTY_TRANSFER='{
  "original_warranty_id": "'$WARRANTY_ID'",
  "new_serial_number": "TVSC2024-NEW-123456",
  "transfer_reason": "WARRANTY_REPLACEMENT",
  "remaining_months": 20,
  "notes": "Warranty transferred to replacement unit per claim '$CLAIM_ID'"
}'
NEW_WARRANTY=$(api_call POST "/api/warranties/transfer" "$WARRANTY_TRANSFER" | jq -r '.new_warranty_id')
echo "✓ Warranty transferred to new unit: $NEW_WARRANTY"
echo ""

# Step 12: Send notifications
echo "Step 12: Sending customer notifications..."
echo "------------------------------------------"
NOTIFICATION_DATA='{
  "type": "WARRANTY_CLAIM_UPDATE",
  "recipient": "'$CUSTOMER_ID'",
  "channel": ["EMAIL", "SMS"],
  "template": "warranty_replacement_shipped",
  "data": {
    "customer_name": "Michael Chen",
    "claim_number": "'$CLAIM_ID'",
    "product_name": "Smart Home Security Camera",
    "tracking_number": "1Z999AA10123456784",
    "new_serial": "TVSC2024-NEW-123456"
  }
}'
api_call POST "/api/notifications/send" "$NOTIFICATION_DATA"
echo "✓ Customer notifications sent"
echo ""

# Step 13: Close warranty claim
echo "Step 13: Closing warranty claim..."
echo "----------------------------------"
CLOSE_DATA='{
  "status": "CLOSED",
  "resolution": "REPLACED",
  "customer_satisfied": true,
  "closed_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "final_notes": "Customer received replacement unit. Original unit returned for analysis."
}'
api_call PATCH "/api/warranties/claims/$CLAIM_ID/close" "$CLOSE_DATA"
echo "✓ Warranty claim closed"
echo ""

# Step 14: Generate warranty analytics
echo "Step 14: Generating warranty analytics..."
echo "-----------------------------------------"
api_call GET "/api/analytics/warranties?product_id=$PRODUCT_ID&period=last_90_days"
echo ""

# Step 15: Check customer warranty history
echo "Step 15: Checking customer warranty history..."
echo "----------------------------------------------"
api_call GET "/api/customers/$CUSTOMER_ID/warranties"
echo ""

echo "======================================="
echo "Warranty Claim Demo Complete!"
echo "======================================="
echo ""
echo "Summary of operations:"
echo "- Registered warranty: $WARRANTY_ID"
echo "- Filed warranty claim: $CLAIM_ID"
echo "- Generated RMA: $RMA_NUMBER"
echo "- Created replacement order: $REPLACEMENT_ORDER"
echo "- Transferred warranty to new unit: $NEW_WARRANTY"
echo "- Sent customer notifications"
echo "- Generated analytics reports"
echo "" 