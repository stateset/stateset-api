#!/bin/bash

# Purchase Order Management Demo Script
# Demonstrates supplier management, PO creation, receiving, and invoice processing

set -e

API_URL="http://localhost:8080"
AUTH_TOKEN="${AUTH_TOKEN:-test-token}"

echo "============================================="
echo "StateSet API - Purchase Order Management Demo"
echo "============================================="
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

# Step 1: Create suppliers
echo "Step 1: Creating suppliers..."
echo "-----------------------------"

# Primary supplier
SUPPLIER1_DATA='{
  "name": "Global Electronics Supply Co.",
  "code": "GESC-001",
  "tax_id": "12-3456789",
  "status": "ACTIVE",
  "contact": {
    "name": "Jane Smith",
    "email": "jane.smith@globalelectronics.com",
    "phone": "+1-555-111-2222",
    "role": "Account Manager"
  },
  "address": {
    "street": "100 Industrial Way",
    "city": "Chicago",
    "state": "IL",
    "zip": "60601",
    "country": "US"
  },
  "payment_terms": "NET30",
  "currency": "USD",
  "minimum_order_value": 500.00
}'
SUPPLIER1_ID=$(api_call POST "/api/suppliers" "$SUPPLIER1_DATA" | jq -r '.id')
echo "✓ Primary supplier created: $SUPPLIER1_ID"

# Secondary supplier
SUPPLIER2_DATA='{
  "name": "Quick Parts Distribution",
  "code": "QPD-002",
  "tax_id": "98-7654321",
  "status": "ACTIVE",
  "contact": {
    "name": "Bob Johnson",
    "email": "bob@quickparts.com",
    "phone": "+1-555-333-4444",
    "role": "Sales Director"
  },
  "address": {
    "street": "200 Commerce Blvd",
    "city": "Dallas",
    "state": "TX",
    "zip": "75201",
    "country": "US"
  },
  "payment_terms": "NET15",
  "currency": "USD",
  "expedited_shipping": true
}'
SUPPLIER2_ID=$(api_call POST "/api/suppliers" "$SUPPLIER2_DATA" | jq -r '.id')
echo "✓ Secondary supplier created: $SUPPLIER2_ID"
echo ""

# Step 2: Create products and link to suppliers
echo "Step 2: Creating products with supplier links..."
echo "-----------------------------------------------"
PRODUCT1_DATA='{
  "name": "Industrial LED Panel Light",
  "sku": "LED-PANEL-500W",
  "description": "500W LED Panel for warehouse lighting",
  "category": "Lighting",
  "unit_cost": 125.00,
  "preferred_supplier_id": "'$SUPPLIER1_ID'",
  "supplier_sku": "GESC-LED-500",
  "lead_time_days": 14,
  "minimum_order_quantity": 10
}'
PRODUCT1_ID=$(api_call POST "/api/products" "$PRODUCT1_DATA" | jq -r '.id')
echo "✓ Product 1 created: $PRODUCT1_ID"

PRODUCT2_DATA='{
  "name": "Emergency Exit Sign",
  "sku": "EXIT-SIGN-LED",
  "description": "LED Emergency Exit Sign with Battery Backup",
  "category": "Safety",
  "unit_cost": 45.00,
  "preferred_supplier_id": "'$SUPPLIER2_ID'",
  "supplier_sku": "QPD-EXIT-01",
  "lead_time_days": 5,
  "minimum_order_quantity": 5
}'
PRODUCT2_ID=$(api_call POST "/api/products" "$PRODUCT2_DATA" | jq -r '.id')
echo "✓ Product 2 created: $PRODUCT2_ID"
echo ""

# Step 3: Check inventory levels and determine reorder needs
echo "Step 3: Checking inventory levels..."
echo "------------------------------------"
api_call GET "/api/inventory/reorder-suggestions"
echo ""

# Step 4: Create purchase order
echo "Step 4: Creating purchase order..."
echo "----------------------------------"
PO_DATA='{
  "supplier_id": "'$SUPPLIER1_ID'",
  "warehouse_id": "warehouse-001",
  "order_date": "'$(date -u +"%Y-%m-%d")'",
  "expected_delivery_date": "'$(date -u -d "+14 days" +"%Y-%m-%d")'",
  "payment_terms": "NET30",
  "shipping_method": "FREIGHT",
  "items": [
    {
      "product_id": "'$PRODUCT1_ID'",
      "quantity": 50,
      "unit_price": 125.00,
      "supplier_sku": "GESC-LED-500",
      "notes": "Urgent - Low stock"
    }
  ],
  "notes": "Please confirm delivery date. Loading dock available 8AM-5PM weekdays.",
  "tags": ["URGENT", "LIGHTING_PROJECT"]
}'
PO_ID=$(api_call POST "/api/purchase-orders" "$PO_DATA" | jq -r '.id')
PO_NUMBER=$(api_call GET "/api/purchase-orders/$PO_ID" | jq -r '.po_number')
echo "✓ Purchase order created: $PO_NUMBER"
echo ""

# Step 5: Submit PO for approval
echo "Step 5: Submitting PO for approval..."
echo "-------------------------------------"
SUBMIT_DATA='{
  "action": "SUBMIT_FOR_APPROVAL",
  "submitted_by": "purchasing-agent-001"
}'
api_call POST "/api/purchase-orders/$PO_ID/workflow" "$SUBMIT_DATA"
echo "✓ PO submitted for approval"
echo ""

# Step 6: Approve purchase order
echo "Step 6: Approving purchase order..."
echo "-----------------------------------"
APPROVAL_DATA='{
  "action": "APPROVE",
  "approved_by": "finance-manager-001",
  "notes": "Approved - within budget allocation"
}'
api_call POST "/api/purchase-orders/$PO_ID/workflow" "$APPROVAL_DATA"
echo "✓ PO approved"
echo ""

# Step 7: Send PO to supplier
echo "Step 7: Sending PO to supplier..."
echo "---------------------------------"
SEND_DATA='{
  "method": "EMAIL",
  "include_attachments": ["purchase_order_pdf"],
  "message": "Please find attached our purchase order '$PO_NUMBER'. Kindly confirm receipt and expected delivery date."
}'
api_call POST "/api/purchase-orders/$PO_ID/send" "$SEND_DATA"
echo "✓ PO sent to supplier"
echo ""

# Step 8: Record supplier acknowledgment
echo "Step 8: Recording supplier acknowledgment..."
echo "--------------------------------------------"
ACK_DATA='{
  "acknowledged_by": "Jane Smith",
  "acknowledged_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "confirmed_delivery_date": "'$(date -u -d "+12 days" +"%Y-%m-%d")'",
  "supplier_reference": "GESC-ORD-2024-1234",
  "notes": "Delivery confirmed for 12 days. Will ship via dedicated truck."
}'
api_call POST "/api/purchase-orders/$PO_ID/acknowledgment" "$ACK_DATA"
echo "✓ Supplier acknowledgment recorded"
echo ""

# Step 9: Create Advanced Shipping Notice (ASN)
echo "Step 9: Creating Advanced Shipping Notice..."
echo "--------------------------------------------"
ASN_DATA='{
  "purchase_order_id": "'$PO_ID'",
  "ship_date": "'$(date -u -d "+10 days" +"%Y-%m-%d")'",
  "expected_arrival": "'$(date -u -d "+12 days" +"%Y-%m-%d")'",
  "carrier": "ABC Freight",
  "tracking_number": "ABC-TRACK-789456",
  "items": [
    {
      "product_id": "'$PRODUCT1_ID'",
      "quantity_shipped": 50,
      "pallet_count": 2,
      "serial_numbers": ["SN-LED-001 through SN-LED-050"]
    }
  ]
}'
ASN_ID=$(api_call POST "/api/asn" "$ASN_DATA" | jq -r '.id')
echo "✓ ASN created: $ASN_ID"
echo ""

# Step 10: Receive shipment
echo "Step 10: Receiving shipment at warehouse..."
echo "-------------------------------------------"
RECEIVING_DATA='{
  "asn_id": "'$ASN_ID'",
  "received_by": "warehouse-receiver-001",
  "received_at": "'$(date -u -d "+12 days" +"%Y-%m-%dT%H:%M:%SZ")'",
  "items": [
    {
      "product_id": "'$PRODUCT1_ID'",
      "quantity_received": 50,
      "quantity_damaged": 2,
      "quantity_accepted": 48,
      "lot_number": "LOT-2024-0612",
      "location": "A-15-2",
      "notes": "2 units damaged in transit - outer packaging compromised"
    }
  ],
  "overall_condition": "GOOD",
  "photos": ["receiving_damage_01.jpg", "pallet_condition.jpg"]
}'
RECEIPT_ID=$(api_call POST "/api/purchase-orders/$PO_ID/receive" "$RECEIVING_DATA")
echo "✓ Shipment received"
echo ""

# Step 11: Quality inspection
echo "Step 11: Performing quality inspection..."
echo "-----------------------------------------"
QC_DATA='{
  "receipt_id": "'$RECEIPT_ID'",
  "inspector_id": "qc-inspector-002",
  "inspection_date": "'$(date -u -d "+12 days" +"%Y-%m-%dT%H:%M:%SZ")'",
  "samples_tested": 5,
  "test_results": {
    "visual_inspection": "PASS",
    "functional_test": "PASS",
    "specification_compliance": "PASS"
  },
  "overall_result": "PASS",
  "notes": "All samples meet specifications. Approved for inventory."
}'
api_call POST "/api/quality/inspections" "$QC_DATA"
echo "✓ Quality inspection completed"
echo ""

# Step 12: Update inventory
echo "Step 12: Updating inventory levels..."
echo "-------------------------------------"
INVENTORY_UPDATE='{
  "product_id": "'$PRODUCT1_ID'",
  "warehouse_id": "warehouse-001",
  "quantity": 48,
  "lot_number": "LOT-2024-0612",
  "location": "A-15-2",
  "source": "PURCHASE_ORDER",
  "reference_id": "'$PO_ID'"
}'
api_call POST "/api/inventory/add" "$INVENTORY_UPDATE"
echo "✓ Inventory updated"
echo ""

# Step 13: File damage claim for damaged units
echo "Step 13: Filing damage claim..."
echo "--------------------------------"
DAMAGE_CLAIM='{
  "purchase_order_id": "'$PO_ID'",
  "claim_type": "SHIPPING_DAMAGE",
  "items": [
    {
      "product_id": "'$PRODUCT1_ID'",
      "quantity": 2,
      "unit_value": 125.00,
      "description": "Units damaged during shipping - outer packaging compromised"
    }
  ],
  "total_claim_amount": 250.00,
  "evidence": ["receiving_damage_01.jpg"],
  "filed_with": "ABC Freight"
}'
api_call POST "/api/claims/shipping-damage" "$DAMAGE_CLAIM"
echo "✓ Damage claim filed"
echo ""

# Step 14: Process supplier invoice
echo "Step 14: Processing supplier invoice..."
echo "---------------------------------------"
INVOICE_DATA='{
  "purchase_order_id": "'$PO_ID'",
  "invoice_number": "INV-GESC-2024-5678",
  "invoice_date": "'$(date -u -d "+13 days" +"%Y-%m-%d")'",
  "due_date": "'$(date -u -d "+43 days" +"%Y-%m-%d")'",
  "items": [
    {
      "product_id": "'$PRODUCT1_ID'",
      "quantity": 50,
      "unit_price": 125.00,
      "line_total": 6250.00
    }
  ],
  "subtotal": 6250.00,
  "tax_amount": 500.00,
  "shipping_cost": 150.00,
  "total_amount": 6900.00,
  "payment_terms": "NET30"
}'
INVOICE_ID=$(api_call POST "/api/invoices/supplier" "$INVOICE_DATA" | jq -r '.id')
echo "✓ Invoice recorded: $INVOICE_ID"
echo ""

# Step 15: Match invoice with PO and receipt
echo "Step 15: Performing 3-way match..."
echo "----------------------------------"
MATCH_DATA='{
  "invoice_id": "'$INVOICE_ID'",
  "purchase_order_id": "'$PO_ID'",
  "receipt_id": "'$RECEIPT_ID'",
  "match_tolerance": 0.02
}'
MATCH_RESULT=$(api_call POST "/api/invoices/three-way-match" "$MATCH_DATA")
echo "✓ 3-way match completed"
echo ""

# Step 16: Schedule payment
echo "Step 16: Scheduling payment..."
echo "------------------------------"
PAYMENT_DATA='{
  "invoice_id": "'$INVOICE_ID'",
  "payment_method": "ACH",
  "scheduled_date": "'$(date -u -d "+25 days" +"%Y-%m-%d")'",
  "amount": 6900.00,
  "notes": "Payment for PO '$PO_NUMBER' - approved via 3-way match"
}'
api_call POST "/api/payments/schedule" "$PAYMENT_DATA"
echo "✓ Payment scheduled"
echo ""

# Step 17: Close purchase order
echo "Step 17: Closing purchase order..."
echo "----------------------------------"
CLOSE_DATA='{
  "status": "CLOSED",
  "closed_by": "purchasing-agent-001",
  "closed_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "final_notes": "PO fulfilled. 48/50 units received. Damage claim filed for 2 units."
}'
api_call PATCH "/api/purchase-orders/$PO_ID/close" "$CLOSE_DATA"
echo "✓ Purchase order closed"
echo ""

# Step 18: Generate procurement analytics
echo "Step 18: Generating procurement analytics..."
echo "--------------------------------------------"
api_call GET "/api/analytics/procurement?supplier_id=$SUPPLIER1_ID&period=last_quarter"
echo ""

echo "============================================="
echo "Purchase Order Management Demo Complete!"
echo "============================================="
echo ""
echo "Summary of operations:"
echo "- Created suppliers and products"
echo "- Generated PO: $PO_NUMBER"
echo "- Processed approval workflow"
echo "- Created ASN: $ASN_ID"
echo "- Received 48/50 units (2 damaged)"
echo "- Updated inventory"
echo "- Filed damage claim"
echo "- Processed invoice: $INVOICE_ID"
echo "- Scheduled payment"
echo "- Generated analytics"
echo "" 