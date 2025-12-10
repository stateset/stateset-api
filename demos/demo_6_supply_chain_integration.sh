#!/bin/bash

#########################################################
# StateSet Manufacturing API - Demo 6
# End-to-End Supply Chain Integration
#########################################################
#
# This demo shows complete supply chain workflow:
# 1. Supplier order and ASN (Advanced Shipping Notice)
# 2. Receiving and quality inspection
# 3. Component warehousing and allocation
# 4. Production work order with component pull
# 5. Finished goods receipt
# 6. Customer order fulfillment and shipment
#
# This demonstrates:
# - Purchase order to receipt flow
# - ASN processing
# - Quality inspection workflows
# - Component reservation and consumption
# - Production execution
# - Order fulfillment
# - Complete supply chain visibility
#########################################################

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# API Configuration
API_BASE="http://localhost:8080/api/v1"
TOKEN="your_jwt_token_here"  # Replace with actual JWT token

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  StateSet Manufacturing API - Demo 6${NC}"
echo -e "${CYAN}  End-to-End Supply Chain Integration${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

#########################################################
# PHASE 1: PROCUREMENT - Purchase Order to Receipt
#########################################################

echo -e "${MAGENTA}═══ PHASE 1: PROCUREMENT ═══${NC}"
echo ""

# Step 1: Create Purchase Order
echo -e "${BLUE}[Step 1.1]${NC} Creating purchase order for components..."
echo ""

PO_NUMBER="PO-2024-1234"
SUPPLIER_ID="sup-electronics-001"

echo -e "${YELLOW}Creating PO: ${PO_NUMBER}${NC}"

PO_RESPONSE=$(curl -s -X POST "$API_BASE/purchase-orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "po_number": "'$PO_NUMBER'",
    "supplier_id": "'$SUPPLIER_ID'",
    "supplier_name": "Electronics Components Inc.",
    "organization_id": 1,
    "status": "APPROVED",
    "order_date": "'$(date +%Y-%m-%d)'",
    "expected_delivery_date": "'$(date -d "+7 days" +%Y-%m-%d)'",
    "shipping_address": {
      "line1": "123 Manufacturing Way",
      "city": "Austin",
      "state": "TX",
      "postal_code": "78701",
      "country": "US"
    },
    "items": [
      {
        "item_id": "item-pcb-001",
        "item_name": "Printed Circuit Board PCB-100",
        "quantity": 500,
        "unit_price": 12.50,
        "total": 6250.00
      },
      {
        "item_id": "item-res-001",
        "item_name": "Resistor 10K Ohm",
        "quantity": 5000,
        "unit_price": 0.05,
        "total": 250.00
      },
      {
        "item_id": "item-cap-001",
        "item_name": "Capacitor 100uF",
        "quantity": 3000,
        "unit_price": 0.15,
        "total": 450.00
      }
    ],
    "subtotal": 6950.00,
    "tax": 556.00,
    "total": 7506.00
  }')

PO_ID=$(echo $PO_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ Purchase Order Created${NC}"
echo -e "  PO Number: ${PO_NUMBER}"
echo -e "  Supplier: Electronics Components Inc."
echo -e "  Items: 3 line items"
echo -e "  Total: $7,506.00"
echo -e "  Expected Delivery: $(date -d "+7 days" +%Y-%m-%d)"
echo ""

sleep 2

# Step 2: Supplier Creates ASN
echo -e "${BLUE}[Step 1.2]${NC} Supplier creating Advanced Shipping Notice..."
echo ""

ASN_NUMBER="ASN-SUP-001-$(date +%Y%m%d)"

echo -e "${YELLOW}Processing ASN from supplier...${NC}"

ASN_RESPONSE=$(curl -s -X POST "$API_BASE/asns" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "asn_number": "'$ASN_NUMBER'",
    "po_number": "'$PO_NUMBER'",
    "supplier_id": "'$SUPPLIER_ID'",
    "status": "IN_TRANSIT",
    "ship_date": "'$(date +%Y-%m-%d)'",
    "expected_delivery_date": "'$(date -d "+7 days" +%Y-%m-%d)'",
    "carrier": "FedEx Freight",
    "tracking_number": "FDX-$(date +%Y%m%d)-001",
    "total_packages": 3,
    "total_weight_lbs": 125.5,
    "items": [
      {
        "item_id": "item-pcb-001",
        "quantity": 500,
        "lot_number": "LOT-PCB-2024-Q4-042"
      },
      {
        "item_id": "item-res-001",
        "quantity": 5000,
        "lot_number": "LOT-RES-2024-Q4-158"
      },
      {
        "item_id": "item-cap-001",
        "quantity": 3000,
        "lot_number": "LOT-CAP-2024-Q4-089"
      }
    ]
  }')

ASN_ID=$(echo $ASN_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ ASN Received and Processed${NC}"
echo -e "  ASN Number: ${ASN_NUMBER}"
echo -e "  Status: ${CYAN}IN_TRANSIT${NC}"
echo -e "  Tracking: FDX-$(date +%Y%m%d)-001"
echo -e "  Carrier: FedEx Freight"
echo -e "  Packages: 3"
echo -e "  Weight: 125.5 lbs"
echo ""

sleep 2

# Step 3: Simulate Transit and Delivery
echo -e "${BLUE}[Step 1.3]${NC} Tracking shipment in transit..."
echo ""

echo -e "${YELLOW}Shipment status updates:${NC}"
echo -e "  Day 1: ${CYAN}Picked up from supplier${NC}"
echo -e "  Day 2: ${CYAN}In transit - Memphis, TN hub${NC}"
echo -e "  Day 3: ${CYAN}In transit - Dallas, TX hub${NC}"
echo -e "  Day 4: ${CYAN}Out for delivery - Austin, TX${NC}"
echo ""

sleep 2

# Mark ASN as delivered
curl -s -X POST "$API_BASE/asns/$ASN_ID/mark-delivered" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "actual_delivery_date": "'$(date +%Y-%m-%d)'",
    "received_by": "receiving-clerk-001",
    "dock_door": "Dock-A-3"
  }' > /dev/null

echo -e "${GREEN}✓ Shipment Delivered${NC}"
echo -e "  Delivered: $(date +%Y-%m-%d)"
echo -e "  Received by: Receiving Clerk 001"
echo -e "  Location: Dock-A-3"
echo ""

sleep 2

# Step 4: Quality Inspection
echo -e "${BLUE}[Step 1.4]${NC} Performing incoming quality inspection..."
echo ""

echo -e "${YELLOW}Quality Control inspection in progress...${NC}"
echo ""

# Inspect PCBs
echo -e "  ${CYAN}Inspecting: Printed Circuit Boards (500 units)${NC}"
echo -e "    Visual inspection: ${GREEN}PASS${NC}"
echo -e "    Dimensional check: ${GREEN}PASS${NC}"
echo -e "    Sample electrical test: ${GREEN}PASS (10/10)${NC}"
echo -e "    Certificate of Conformance: ${GREEN}Verified${NC}"
echo ""

# Inspect Resistors
echo -e "  ${CYAN}Inspecting: Resistors (5000 units)${NC}"
echo -e "    Resistance measurement: ${GREEN}PASS${NC}"
echo -e "    Sample size: 50 units"
echo -e "    Tolerance: ±1% specification met"
echo ""

# Inspect Capacitors
echo -e "  ${CYAN}Inspecting: Capacitors (3000 units)${NC}"
echo -e "    Capacitance test: ${GREEN}PASS${NC}"
echo -e "    Visual inspection: ${GREEN}PASS${NC}"
echo ""

# Create inspection record
curl -s -X POST "$API_BASE/quality/inspections" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "asn_id": "'$ASN_ID'",
    "po_number": "'$PO_NUMBER'",
    "inspection_type": "RECEIVING",
    "inspector_id": "qc-inspector-002",
    "inspection_date": "'$(date +%Y-%m-%d)'",
    "overall_result": "ACCEPT",
    "items_inspected": [
      {
        "item_id": "item-pcb-001",
        "quantity_inspected": 10,
        "result": "ACCEPT",
        "notes": "All samples passed visual and electrical tests"
      },
      {
        "item_id": "item-res-001",
        "quantity_inspected": 50,
        "result": "ACCEPT",
        "notes": "Resistance values within ±1% tolerance"
      },
      {
        "item_id": "item-cap-001",
        "quantity_inspected": 30,
        "result": "ACCEPT",
        "notes": "Capacitance values within specification"
      }
    ]
  }' > /dev/null

echo -e "${GREEN}✓ Quality Inspection Completed${NC}"
echo -e "  Result: ${GREEN}ACCEPTED${NC}"
echo -e "  Inspector: QC Inspector 002"
echo -e "  All items cleared for warehousing"
echo ""

sleep 2

# Step 5: Putaway to Warehouse
echo -e "${BLUE}[Step 1.5]${NC} Moving components to warehouse..."
echo ""

echo -e "${YELLOW}Warehouse putaway in progress...${NC}"
echo ""

curl -s -X POST "$API_BASE/inventory/receive" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "asn_id": "'$ASN_ID'",
    "location_id": 100,
    "received_by": "warehouse-operator-003",
    "items": [
      {
        "item_id": "item-pcb-001",
        "quantity": 500,
        "lot_number": "LOT-PCB-2024-Q4-042",
        "bin_location": "A-12-05",
        "expiry_date": "2026-12-31"
      },
      {
        "item_id": "item-res-001",
        "quantity": 5000,
        "lot_number": "LOT-RES-2024-Q4-158",
        "bin_location": "B-08-12",
        "expiry_date": "2027-06-30"
      },
      {
        "item_id": "item-cap-001",
        "quantity": 3000,
        "lot_number": "LOT-CAP-2024-Q4-089",
        "bin_location": "B-08-15",
        "expiry_date": "2027-03-31"
      }
    ]
  }' > /dev/null

echo -e "  ${GREEN}✓${NC} PCB-100: 500 units → Location A-12-05"
echo -e "  ${GREEN}✓${NC} Resistors: 5000 units → Location B-08-12"
echo -e "  ${GREEN}✓${NC} Capacitors: 3000 units → Location B-08-15"
echo ""

echo -e "${GREEN}✓ Components Successfully Warehoused${NC}"
echo ""

sleep 2

#########################################################
# PHASE 2: PRODUCTION - Work Order Execution
#########################################################

echo -e "${MAGENTA}═══ PHASE 2: PRODUCTION ═══${NC}"
echo ""

# Step 6: Create Work Order
echo -e "${BLUE}[Step 2.1]${NC} Creating manufacturing work order..."
echo ""

WO_NUMBER="WO-2024-CTRL-001"
PRODUCT_ID="prod-controller-001"

echo -e "${YELLOW}Creating work order: ${WO_NUMBER}${NC}"

WO_RESPONSE=$(curl -s -X POST "$API_BASE/work-orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "work_order_number": "'$WO_NUMBER'",
    "item_id": "'$PRODUCT_ID'",
    "product_name": "Industrial Controller IND-500",
    "organization_id": 1,
    "quantity_to_build": 100,
    "scheduled_start_date": "'$(date +%Y-%m-%d)'",
    "scheduled_completion_date": "'$(date -d "+5 days" +%Y-%m-%d)'",
    "location_id": 100,
    "priority": "HIGH",
    "bom_id": "bom-controller-001"
  }')

WO_ID=$(echo $WO_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ Work Order Created${NC}"
echo -e "  WO Number: ${WO_NUMBER}"
echo -e "  Product: Industrial Controller IND-500"
echo -e "  Quantity: 100 units"
echo -e "  Status: ${GREEN}READY${NC} (components available)"
echo ""

sleep 2

# Step 7: Component Picking and Kitting
echo -e "${BLUE}[Step 2.2]${NC} Picking components for production..."
echo ""

echo -e "${YELLOW}Generating pick list and kitting components...${NC}"
echo ""

echo -e "  ${CYAN}Pick List for WO-${WO_NUMBER}:${NC}"
echo -e "    • PCB-100: 100 units from A-12-05 (Lot: LOT-PCB-2024-Q4-042)"
echo -e "    • Resistors: 1000 units from B-08-12 (Lot: LOT-RES-2024-Q4-158)"
echo -e "    • Capacitors: 500 units from B-08-15 (Lot: LOT-CAP-2024-Q4-089)"
echo ""

curl -s -X POST "$API_BASE/inventory/pick" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "work_order_id": "'$WO_ID'",
    "picked_by": "warehouse-picker-001",
    "items": [
      {
        "item_id": "item-pcb-001",
        "quantity": 100,
        "from_location": "A-12-05",
        "lot_number": "LOT-PCB-2024-Q4-042"
      },
      {
        "item_id": "item-res-001",
        "quantity": 1000,
        "from_location": "B-08-12",
        "lot_number": "LOT-RES-2024-Q4-158"
      },
      {
        "item_id": "item-cap-001",
        "quantity": 500,
        "from_location": "B-08-15",
        "lot_number": "LOT-CAP-2024-Q4-089"
      }
    ]
  }' > /dev/null

echo -e "${GREEN}✓ Components Picked and Kitted${NC}"
echo -e "  Kitting location: Production-Staging-A"
echo -e "  All materials ready for production"
echo ""

sleep 2

# Step 8: Start Production
echo -e "${BLUE}[Step 2.3]${NC} Starting production..."
echo ""

curl -s -X POST "$API_BASE/work-orders/$WO_ID/start" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "location_id": 100,
    "operator_id": "operator-002",
    "manufacturing_line": "Assembly-Line-B"
  }' > /dev/null

echo -e "${GREEN}✓ Production Started${NC}"
echo -e "  Status: ${CYAN}IN_PROGRESS${NC}"
echo -e "  Manufacturing Line: Assembly-Line-B"
echo -e "  Operator: Operator-002"
echo -e "  Start Time: $(date +"%H:%M")"
echo ""

sleep 2

# Step 9: Production Progress
echo -e "${BLUE}[Step 2.4]${NC} Production in progress..."
echo ""

echo -e "${YELLOW}Assembly progress:${NC}"
echo -e "  Hour 1: 20 units completed (20%)"
echo -e "  Hour 2: 45 units completed (45%)"
echo -e "  Hour 3: 70 units completed (70%)"
echo -e "  Hour 4: 95 units completed (95%)"
echo -e "  Hour 5: 100 units completed (100%)"
echo ""

echo -e "${CYAN}[●●●●●●●●●●●●●●●●●●●●] 100%${NC}"
echo ""

sleep 2

# Step 10: Complete Work Order
echo -e "${BLUE}[Step 2.5]${NC} Completing work order..."
echo ""

curl -s -X POST "$API_BASE/work-orders/$WO_ID/complete" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "completed_quantity": 100,
    "location_id": 100,
    "completion_notes": "All 100 units assembled successfully. Final QC inspection passed."
  }' > /dev/null

echo -e "${GREEN}✓ Work Order Completed${NC}"
echo -e "  Status: ${GREEN}COMPLETED${NC}"
echo -e "  Quantity: 100/100 units (100% yield)"
echo -e "  Completion Time: $(date +"%H:%M")"
echo -e "  Production Duration: 5 hours"
echo ""

# Add finished goods to inventory
curl -s -X POST "$API_BASE/inventory/finished-goods-receipt" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "work_order_id": "'$WO_ID'",
    "item_id": "'$PRODUCT_ID'",
    "quantity": 100,
    "location": "FG-Warehouse-Zone-C",
    "bin_location": "C-15-08"
  }' > /dev/null

echo -e "${GREEN}✓ Finished Goods Received to Inventory${NC}"
echo -e "  Location: FG-Warehouse-Zone-C"
echo -e "  Bin: C-15-08"
echo -e "  Available for sale: 100 units"
echo ""

sleep 2

#########################################################
# PHASE 3: FULFILLMENT - Customer Order to Shipment
#########################################################

echo -e "${MAGENTA}═══ PHASE 3: FULFILLMENT ═══${NC}"
echo ""

# Step 11: Customer Order
echo -e "${BLUE}[Step 3.1]${NC} Receiving customer order..."
echo ""

ORDER_NUMBER="SO-2024-5678"
CUSTOMER_ID="cust-acme-corp"

echo -e "${YELLOW}Processing customer order: ${ORDER_NUMBER}${NC}"

ORDER_RESPONSE=$(curl -s -X POST "$API_BASE/orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "order_number": "'$ORDER_NUMBER'",
    "customer_id": "'$CUSTOMER_ID'",
    "customer_name": "ACME Corporation",
    "order_date": "'$(date +%Y-%m-%d)'",
    "status": "PENDING",
    "items": [
      {
        "item_id": "'$PRODUCT_ID'",
        "product_name": "Industrial Controller IND-500",
        "quantity": 25,
        "unit_price": 495.00,
        "total": 12375.00
      }
    ],
    "subtotal": 12375.00,
    "tax": 990.00,
    "shipping": 75.00,
    "total": 13440.00,
    "shipping_address": {
      "company": "ACME Corporation",
      "line1": "456 Industrial Parkway",
      "city": "Chicago",
      "state": "IL",
      "postal_code": "60601",
      "country": "US"
    }
  }')

ORDER_ID=$(echo $ORDER_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ Customer Order Created${NC}"
echo -e "  Order: ${ORDER_NUMBER}"
echo -e "  Customer: ACME Corporation"
echo -e "  Product: Industrial Controller IND-500"
echo -e "  Quantity: 25 units"
echo -e "  Total: $13,440.00"
echo ""

sleep 2

# Step 12: Pick and Pack
echo -e "${BLUE}[Step 3.2]${NC} Picking and packing order..."
echo ""

echo -e "${YELLOW}Fulfillment process:${NC}"
echo -e "  Picking items from C-15-08..."
echo -e "  Quantity: 25 units"
echo ""

curl -s -X POST "$API_BASE/orders/$ORDER_ID/pick" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "picked_by": "warehouse-picker-002",
    "items": [
      {
        "item_id": "'$PRODUCT_ID'",
        "quantity": 25,
        "bin_location": "C-15-08"
      }
    ]
  }' > /dev/null

echo -e "${GREEN}✓ Items Picked${NC}"
echo ""

echo -e "  Packing items..."
echo -e "  Boxes: 2 (Box 1: 15 units, Box 2: 10 units)"
echo -e "  Packing slip printed"
echo -e "  Commercial invoice prepared"
echo ""

curl -s -X POST "$API_BASE/orders/$ORDER_ID/pack" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "packed_by": "warehouse-packer-001",
    "packages": [
      {
        "package_number": 1,
        "weight_lbs": 45.2,
        "dimensions": "24x18x12",
        "items": [{"item_id": "'$PRODUCT_ID'", "quantity": 15}]
      },
      {
        "package_number": 2,
        "weight_lbs": 30.8,
        "dimensions": "18x18x12",
        "items": [{"item_id": "'$PRODUCT_ID'", "quantity": 10}]
      }
    ]
  }' > /dev/null

echo -e "${GREEN}✓ Order Packed${NC}"
echo -e "  Total packages: 2"
echo -e "  Total weight: 76 lbs"
echo ""

sleep 2

# Step 13: Create Shipment
echo -e "${BLUE}[Step 3.3]${NC} Creating shipment..."
echo ""

TRACKING_NUMBER="UPS-$(date +%Y%m%d)-$(openssl rand -hex 4)"

SHIPMENT_RESPONSE=$(curl -s -X POST "$API_BASE/shipments" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": "'$ORDER_ID'",
    "carrier": "UPS Ground",
    "tracking_number": "'$TRACKING_NUMBER'",
    "ship_date": "'$(date +%Y-%m-%d)'",
    "estimated_delivery": "'$(date -d "+3 days" +%Y-%m-%d)'",
    "total_packages": 2,
    "total_weight_lbs": 76.0,
    "shipping_cost": 75.00
  }')

SHIPMENT_ID=$(echo $SHIPMENT_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ Shipment Created${NC}"
echo -e "  Carrier: UPS Ground"
echo -e "  Tracking: ${TRACKING_NUMBER}"
echo -e "  Estimated Delivery: $(date -d "+3 days" +%Y-%m-%d)"
echo ""

# Mark order as shipped
curl -s -X POST "$API_BASE/orders/$ORDER_ID/ship" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "shipment_id": "'$SHIPMENT_ID'"
  }' > /dev/null

echo -e "${GREEN}✓ Order Marked as Shipped${NC}"
echo -e "  Order Status: ${CYAN}SHIPPED${NC}"
echo -e "  Customer notified via email"
echo ""

sleep 2

#########################################################
# PHASE 4: VISIBILITY - Supply Chain Dashboard
#########################################################

echo -e "${MAGENTA}═══ PHASE 4: SUPPLY CHAIN VISIBILITY ═══${NC}"
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  SUPPLY CHAIN DASHBOARD${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

echo -e "${MAGENTA}Procurement Status:${NC}"
echo -e "  Purchase Orders:"
echo -e "    • ${PO_NUMBER}: ${GREEN}Received & Inspected${NC}"
echo -e "    • Supplier: Electronics Components Inc."
echo -e "    • Value: $7,506.00"
echo ""
echo -e "  Inbound Shipments:"
echo -e "    • ${ASN_NUMBER}: ${GREEN}Delivered${NC}"
echo -e "    • Tracking: FDX-$(date +%Y%m%d)-001"
echo ""

echo -e "${MAGENTA}Inventory Status:${NC}"
echo -e "  Raw Materials:"
echo -e "    • PCB-100: 400 units (was 500, used 100)"
echo -e "    • Resistors: 4,000 units (was 5000, used 1000)"
echo -e "    • Capacitors: 2,500 units (was 3000, used 500)"
echo ""
echo -e "  Finished Goods:"
echo -e "    • Industrial Controller IND-500: 75 units"
echo -e "      (Produced 100, shipped 25)"
echo ""

echo -e "${MAGENTA}Production Status:${NC}"
echo -e "  Work Orders:"
echo -e "    • ${WO_NUMBER}: ${GREEN}Completed${NC}"
echo -e "    • Quantity: 100 units (100% yield)"
echo -e "    • Duration: 5 hours"
echo ""

echo -e "${MAGENTA}Fulfillment Status:${NC}"
echo -e "  Customer Orders:"
echo -e "    • ${ORDER_NUMBER}: ${CYAN}Shipped${NC}"
echo -e "    • Customer: ACME Corporation"
echo -e "    • Quantity: 25 units"
echo -e "    • Tracking: ${TRACKING_NUMBER}"
echo -e "    • Expected Delivery: $(date -d "+3 days" +%Y-%m-%d)"
echo ""

echo -e "${MAGENTA}Complete Traceability:${NC}"
echo -e "  Component Genealogy for Order ${ORDER_NUMBER}:"
echo -e "    ↓ PO-2024-1234 (Supplier: Electronics Components Inc.)"
echo -e "    ↓ ASN-SUP-001-$(date +%Y%m%d) (Delivered)"
echo -e "    ↓ Quality Inspection (Accepted)"
echo -e "    ↓ Warehouse Receipt (Bin: A-12-05, B-08-12, B-08-15)"
echo -e "    ↓ WO-2024-CTRL-001 (Production)"
echo -e "      • PCB Lot: LOT-PCB-2024-Q4-042"
echo -e "      • Resistor Lot: LOT-RES-2024-Q4-158"
echo -e "      • Capacitor Lot: LOT-CAP-2024-Q4-089"
echo -e "    ↓ Finished Goods Receipt (C-15-08)"
echo -e "    ↓ SO-2024-5678 (Customer: ACME Corp)"
echo -e "    ↓ Shipment (UPS: ${TRACKING_NUMBER})"
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

#########################################################
# Summary
#########################################################

echo -e "${GREEN}Demo Complete!${NC}"
echo ""
echo -e "This demo showed complete supply chain integration:"
echo ""
echo -e "${CYAN}Phase 1: PROCUREMENT${NC}"
echo -e "  • Purchase order creation"
echo -e "  • ASN (Advanced Shipping Notice) processing"
echo -e "  • Shipment tracking"
echo -e "  • Receiving and quality inspection"
echo -e "  • Warehouse putaway"
echo ""
echo -e "${CYAN}Phase 2: PRODUCTION${NC}"
echo -e "  • Work order creation"
echo -e "  • Component picking and kitting"
echo -e "  • Production execution"
echo -e "  • Finished goods receipt"
echo ""
echo -e "${CYAN}Phase 3: FULFILLMENT${NC}"
echo -e "  • Customer order processing"
echo -e "  • Order picking and packing"
echo -e "  • Shipment creation"
echo -e "  • Order shipment"
echo ""
echo -e "${CYAN}Phase 4: VISIBILITY${NC}"
echo -e "  • Real-time supply chain dashboard"
echo -e "  • Complete traceability"
echo -e "  • Lot tracking across the supply chain"
echo ""
echo -e "Key Benefits:"
echo -e "  • End-to-end supply chain visibility"
echo -e "  • Complete lot traceability"
echo -e "  • Automated workflows"
echo -e "  • Real-time inventory tracking"
echo -e "  • Quality control integration"
echo -e "  • Customer satisfaction through on-time delivery"
echo ""
