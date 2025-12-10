#!/bin/bash

#########################################################
# StateSet Manufacturing API - Demo 4
# Production Scheduling & Multi-Work Order Management
#########################################################
#
# This demo shows advanced production scheduling:
# 1. Create multiple work orders with different priorities
# 2. Check material availability across orders
# 3. Schedule production based on constraints
# 4. Handle resource conflicts
# 5. Track production capacity
# 6. Optimize production sequence
#
# This demonstrates:
# - Multi-work order planning
# - Material constraint management
# - Production scheduling optimization
# - Resource allocation
# - Capacity planning
# - Priority-based scheduling
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

# Production facility capacity (units per day)
DAILY_CAPACITY=500

echo -e "${CYAN}================================================${NC}"
echo -e "${CYAN}  StateSet Manufacturing API - Demo 4${NC}"
echo -e "${CYAN}  Production Scheduling & Planning${NC}"
echo -e "${CYAN}================================================${NC}"
echo ""

#########################################################
# STEP 1: Setup - Create Products and BOMs
#########################################################

echo -e "${BLUE}[STEP 1]${NC} Setting up products and BOMs..."
echo ""

# Product IDs (in production, these would exist)
PRODUCT_A_ID="prod-a-001"  # Industrial Motor
PRODUCT_B_ID="prod-b-001"  # Control Panel
PRODUCT_C_ID="prod-c-001"  # Sensor Assembly

# Shared component IDs
STEEL_PLATE_ID="comp-steel-001"
COPPER_WIRE_ID="comp-copper-001"
CIRCUIT_BOARD_ID="comp-circuit-001"
PLASTIC_HOUSING_ID="comp-plastic-001"

echo -e "${GREEN}✓ Products configured:${NC}"
echo -e "  • Industrial Motor (Product A)"
echo -e "  • Control Panel (Product B)"
echo -e "  • Sensor Assembly (Product C)"
echo ""

echo -e "${GREEN}✓ Shared components identified:${NC}"
echo -e "  • Steel plates"
echo -e "  • Copper wire"
echo -e "  • Circuit boards"
echo -e "  • Plastic housings"
echo ""

#########################################################
# STEP 2: Check Current Inventory Levels
#########################################################

echo -e "${BLUE}[STEP 2]${NC} Checking inventory availability..."
echo ""

echo -e "${YELLOW}Current inventory levels:${NC}"

# Check steel plates
echo -e "  Steel Plates:"
echo -e "    On-hand: ${GREEN}1,000 units${NC}"
echo -e "    Allocated: ${YELLOW}200 units${NC}"
echo -e "    Available: ${GREEN}800 units${NC}"
echo ""

# Check copper wire
echo -e "  Copper Wire:"
echo -e "    On-hand: ${GREEN}5,000 ft${NC}"
echo -e "    Allocated: ${YELLOW}1,000 ft${NC}"
echo -e "    Available: ${GREEN}4,000 ft${NC}"
echo ""

# Check circuit boards
echo -e "  Circuit Boards:"
echo -e "    On-hand: ${RED}150 units${NC}"
echo -e "    Allocated: ${YELLOW}50 units${NC}"
echo -e "    Available: ${RED}100 units${NC} ⚠ LOW STOCK"
echo ""

# Check plastic housings
echo -e "  Plastic Housings:"
echo -e "    On-hand: ${GREEN}2,000 units${NC}"
echo -e "    Allocated: ${YELLOW}500 units${NC}"
echo -e "    Available: ${GREEN}1,500 units${NC}"
echo ""

sleep 2

#########################################################
# STEP 3: Create Multiple Work Orders
#########################################################

echo -e "${BLUE}[STEP 3]${NC} Creating work orders with different priorities..."
echo ""

# Work Order 1: High Priority - Customer Order
echo -e "${YELLOW}Creating WO-001: Industrial Motors (HIGH PRIORITY)${NC}"
WO_001=$(curl -s -X POST "$API_BASE/work-orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "work_order_number": "WO-2024-001",
    "item_id": "'$PRODUCT_A_ID'",
    "organization_id": 1,
    "quantity_to_build": 100,
    "scheduled_start_date": "2024-12-05",
    "scheduled_completion_date": "2024-12-10",
    "location_id": 100,
    "priority": "HIGH",
    "customer_order_reference": "CO-12345"
  }' | jq -r '.id')

echo -e "  ${GREEN}✓ WO-001 created${NC}"
echo -e "    Product: Industrial Motors"
echo -e "    Quantity: 100 units"
echo -e "    Priority: ${RED}HIGH${NC}"
echo -e "    Due Date: 2024-12-10"
echo -e "    Customer: CO-12345"
echo ""

# Work Order 2: Medium Priority - Stock Replenishment
echo -e "${YELLOW}Creating WO-002: Control Panels (MEDIUM PRIORITY)${NC}"
WO_002=$(curl -s -X POST "$API_BASE/work-orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "work_order_number": "WO-2024-002",
    "item_id": "'$PRODUCT_B_ID'",
    "organization_id": 1,
    "quantity_to_build": 200,
    "scheduled_start_date": "2024-12-06",
    "scheduled_completion_date": "2024-12-15",
    "location_id": 100,
    "priority": "MEDIUM",
    "order_type": "STOCK_REPLENISHMENT"
  }' | jq -r '.id')

echo -e "  ${GREEN}✓ WO-002 created${NC}"
echo -e "    Product: Control Panels"
echo -e "    Quantity: 200 units"
echo -e "    Priority: ${YELLOW}MEDIUM${NC}"
echo -e "    Due Date: 2024-12-15"
echo -e "    Type: Stock Replenishment"
echo ""

# Work Order 3: High Priority - Rush Order
echo -e "${YELLOW}Creating WO-003: Sensor Assemblies (HIGH PRIORITY - RUSH)${NC}"
WO_003=$(curl -s -X POST "$API_BASE/work-orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "work_order_number": "WO-2024-003",
    "item_id": "'$PRODUCT_C_ID'",
    "organization_id": 1,
    "quantity_to_build": 50,
    "scheduled_start_date": "2024-12-05",
    "scheduled_completion_date": "2024-12-08",
    "location_id": 100,
    "priority": "URGENT",
    "customer_order_reference": "CO-12346",
    "rush_order": true
  }' | jq -r '.id')

echo -e "  ${GREEN}✓ WO-003 created${NC}"
echo -e "    Product: Sensor Assemblies"
echo -e "    Quantity: 50 units"
echo -e "    Priority: ${RED}URGENT (RUSH)${NC}"
echo -e "    Due Date: 2024-12-08"
echo -e "    Customer: CO-12346"
echo ""

# Work Order 4: Low Priority - Build to Stock
echo -e "${YELLOW}Creating WO-004: Industrial Motors (LOW PRIORITY)${NC}"
WO_004=$(curl -s -X POST "$API_BASE/work-orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "work_order_number": "WO-2024-004",
    "item_id": "'$PRODUCT_A_ID'",
    "organization_id": 1,
    "quantity_to_build": 150,
    "scheduled_start_date": "2024-12-10",
    "scheduled_completion_date": "2024-12-20",
    "location_id": 100,
    "priority": "LOW",
    "order_type": "BUILD_TO_STOCK"
  }' | jq -r '.id')

echo -e "  ${GREEN}✓ WO-004 created${NC}"
echo -e "    Product: Industrial Motors"
echo -e "    Quantity: 150 units"
echo -e "    Priority: ${GREEN}LOW${NC}"
echo -e "    Due Date: 2024-12-20"
echo -e "    Type: Build to Stock"
echo ""

sleep 2

#########################################################
# STEP 4: Material Availability Analysis
#########################################################

echo -e "${BLUE}[STEP 4]${NC} Analyzing material availability..."
echo ""

echo -e "${YELLOW}Material requirements analysis:${NC}"
echo ""

# Calculate total material needs
echo -e "  ${CYAN}Steel Plates:${NC}"
echo -e "    WO-001 needs: 200 units (100 motors × 2)"
echo -e "    WO-002 needs: 200 units (200 panels × 1)"
echo -e "    WO-003 needs: 50 units (50 sensors × 1)"
echo -e "    WO-004 needs: 300 units (150 motors × 2)"
echo -e "    ${YELLOW}Total needed: 750 units${NC}"
echo -e "    Available: 800 units"
echo -e "    Status: ${GREEN}✓ SUFFICIENT${NC}"
echo ""

echo -e "  ${CYAN}Circuit Boards:${NC}"
echo -e "    WO-001 needs: 100 units (100 motors × 1)"
echo -e "    WO-002 needs: 200 units (200 panels × 1)"
echo -e "    WO-003 needs: 100 units (50 sensors × 2)"
echo -e "    WO-004 needs: 150 units (150 motors × 1)"
echo -e "    ${RED}Total needed: 550 units${NC}"
echo -e "    Available: 100 units"
echo -e "    Status: ${RED}✗ SHORTAGE (450 units short)${NC}"
echo ""

echo -e "  ${CYAN}Copper Wire:${NC}"
echo -e "    WO-001 needs: 1000 ft (100 motors × 10 ft)"
echo -e "    WO-002 needs: 1000 ft (200 panels × 5 ft)"
echo -e "    WO-003 needs: 500 ft (50 sensors × 10 ft)"
echo -e "    WO-004 needs: 1500 ft (150 motors × 10 ft)"
echo -e "    ${YELLOW}Total needed: 4000 ft${NC}"
echo -e "    Available: 4000 ft"
echo -e "    Status: ${YELLOW}⚠ AT CAPACITY${NC}"
echo ""

sleep 3

#########################################################
# STEP 5: Create Optimized Production Schedule
#########################################################

echo -e "${BLUE}[STEP 5]${NC} Generating optimized production schedule..."
echo ""

echo -e "${YELLOW}Scheduling constraints:${NC}"
echo -e "  • Daily capacity: ${DAILY_CAPACITY} units"
echo -e "  • Circuit board shortage: 450 units"
echo -e "  • Priority levels: URGENT > HIGH > MEDIUM > LOW"
echo ""

sleep 2

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  OPTIMIZED PRODUCTION SCHEDULE${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

# Schedule based on priority and material availability
echo -e "${MAGENTA}Day 1-2 (Dec 5-6):${NC} ${RED}WO-003${NC} - Sensor Assemblies (URGENT)"
echo -e "  Quantity: 50 units"
echo -e "  Status: ${GREEN}Materials Available${NC}"
echo -e "  Circuit boards needed: 100 (Available after production)"
echo -e "  Start: ${GREEN}IMMEDIATE${NC}"
echo ""

echo -e "${MAGENTA}Day 3-4 (Dec 7-8):${NC} ${YELLOW}WAIT${NC} - Purchase Order Incoming"
echo -e "  ${YELLOW}⏸ Production on hold${NC}"
echo -e "  Reason: Circuit board shortage"
echo -e "  PO Expected: Dec 8 (350 units incoming)"
echo -e "  Actions:"
echo -e "    • Expedite circuit board delivery"
echo -e "    • Prepare WO-001 materials"
echo -e "    • Quality pre-checks"
echo ""

echo -e "${MAGENTA}Day 5-6 (Dec 9-10):${NC} ${RED}WO-001${NC} - Industrial Motors (HIGH)"
echo -e "  Quantity: 100 units"
echo -e "  Status: ${GREEN}Materials Available (after PO receipt)${NC}"
echo -e "  Circuit boards: 100 from new shipment"
echo -e "  Customer: CO-12345"
echo ""

echo -e "${MAGENTA}Day 7-10 (Dec 11-14):${NC} ${YELLOW}WO-002${NC} - Control Panels (MEDIUM)"
echo -e "  Quantity: 200 units"
echo -e "  Status: ${GREEN}Materials Available${NC}"
echo -e "  Circuit boards: 200 from new shipment"
echo -e "  Type: Stock Replenishment"
echo ""

echo -e "${MAGENTA}Day 11-14 (Dec 15-18):${NC} ${GREEN}WO-004${NC} - Industrial Motors (LOW)"
echo -e "  Quantity: 150 units"
echo -e "  Status: ${YELLOW}Partial - Material Dependent${NC}"
echo -e "  Circuit boards: 50 remaining (need 100 more)"
echo -e "  Recommendation: ${YELLOW}Split order or defer${NC}"
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

sleep 3

#########################################################
# STEP 6: Handle Material Shortage
#########################################################

echo -e "${BLUE}[STEP 6]${NC} Creating purchase order for circuit boards..."
echo ""

echo -e "${YELLOW}Creating emergency PO for circuit boards...${NC}"

PO_RESPONSE=$(curl -s -X POST "$API_BASE/purchase-orders" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "po_number": "PO-2024-099-URGENT",
    "supplier_id": "sup-electronics-001",
    "organization_id": 1,
    "status": "APPROVED",
    "priority": "URGENT",
    "requested_delivery_date": "2024-12-08",
    "items": [
      {
        "item_id": "'$CIRCUIT_BOARD_ID'",
        "quantity": 500,
        "unit_price": 15.50,
        "line_total": 7750.00
      }
    ],
    "notes": "URGENT: Production shortage. Expedite delivery required."
  }')

PO_ID=$(echo $PO_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ Purchase Order Created: PO-2024-099-URGENT${NC}"
echo -e "  Item: Circuit Boards"
echo -e "  Quantity: 500 units"
echo -e "  Supplier: Electronics Supply Co."
echo -e "  Expected: 2024-12-08"
echo -e "  Priority: ${RED}URGENT${NC}"
echo -e "  Total: $7,750.00"
echo ""

sleep 2

#########################################################
# STEP 7: Update Work Order Priorities
#########################################################

echo -e "${BLUE}[STEP 7]${NC} Updating work order priorities based on schedule..."
echo ""

# Put WO-002 and WO-004 on hold until materials arrive
echo -e "${YELLOW}Adjusting work order statuses...${NC}"

curl -s -X PUT "$API_BASE/work-orders/$WO_002/hold" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "reason": "Waiting for circuit board delivery (PO-2024-099-URGENT). Expected Dec 8."
  }' > /dev/null

echo -e "  ${YELLOW}⏸ WO-002 placed on hold${NC}"
echo -e "    Reason: Material shortage"
echo ""

curl -s -X PUT "$API_BASE/work-orders/$WO_004/hold" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "reason": "Low priority - deferred until material availability confirmed"
  }' > /dev/null

echo -e "  ${YELLOW}⏸ WO-004 placed on hold${NC}"
echo -e "    Reason: Low priority + material shortage"
echo ""

sleep 2

#########################################################
# STEP 8: Start Production (Highest Priority)
#########################################################

echo -e "${BLUE}[STEP 8]${NC} Starting production on highest priority work order..."
echo ""

echo -e "${YELLOW}Starting WO-003: Sensor Assemblies (URGENT)...${NC}"

curl -s -X POST "$API_BASE/work-orders/$WO_003/start" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "location_id": 100,
    "operator_id": "operator-001"
  }' > /dev/null

echo -e "${GREEN}✓ WO-003 started${NC}"
echo -e "  Status: ${CYAN}IN_PROGRESS${NC}"
echo -e "  Product: Sensor Assemblies"
echo -e "  Quantity: 50 units"
echo -e "  Estimated completion: 2 days"
echo ""

sleep 2

#########################################################
# STEP 9: Capacity Planning Report
#########################################################

echo -e "${BLUE}[STEP 9]${NC} Generating capacity planning report..."
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  CAPACITY PLANNING REPORT${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

echo -e "${MAGENTA}Work Order Summary:${NC}"
echo -e "  Total Orders: 4"
echo -e "  In Progress: 1 (WO-003)"
echo -e "  On Hold: 2 (WO-002, WO-004)"
echo -e "  Ready: 0"
echo -e "  Pending Materials: 1 (WO-001)"
echo ""

echo -e "${MAGENTA}Material Status:${NC}"
echo -e "  ${GREEN}✓ Steel Plates:${NC} Sufficient (800/750 needed)"
echo -e "  ${RED}✗ Circuit Boards:${NC} Shortage (100/550 needed)"
echo -e "  ${YELLOW}⚠ Copper Wire:${NC} At capacity (4000/4000 needed)"
echo -e "  ${GREEN}✓ Plastic Housings:${NC} Sufficient (1500/800 needed)"
echo ""

echo -e "${MAGENTA}Production Capacity:${NC}"
echo -e "  Daily Capacity: ${DAILY_CAPACITY} units"
echo -e "  Total Planned: 500 units over 14 days"
echo -e "  Capacity Utilization: ${GREEN}71%${NC}"
echo -e "  Available Capacity: 2500 units"
echo ""

echo -e "${MAGENTA}Critical Path:${NC}"
echo -e "  1. ${RED}Circuit board shortage${NC} - Blocking 3 work orders"
echo -e "  2. Purchase order PO-2024-099-URGENT due Dec 8"
echo -e "  3. ${YELLOW}2-day production delay${NC} expected"
echo ""

echo -e "${MAGENTA}Risk Factors:${NC}"
echo -e "  ${RED}HIGH:${NC} Circuit board delivery delay"
echo -e "  ${YELLOW}MEDIUM:${NC} Copper wire at capacity (no buffer)"
echo -e "  ${GREEN}LOW:${NC} Other materials well-stocked"
echo ""

echo -e "${MAGENTA}Recommendations:${NC}"
echo -e "  1. ${YELLOW}Monitor PO-2024-099-URGENT delivery status daily${NC}"
echo -e "  2. Increase safety stock for circuit boards to 200 units"
echo -e "  3. Consider alternative supplier for circuit boards"
echo -e "  4. Order additional copper wire (buffer stock)"
echo -e "  5. Schedule capacity review for Q1 2025"
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

sleep 2

#########################################################
# STEP 10: Create Production Schedule Dashboard
#########################################################

echo -e "${BLUE}[STEP 10]${NC} Production Schedule Timeline"
echo ""

echo -e "${CYAN}┌────────────────────────────────────────────────────┐${NC}"
echo -e "${CYAN}│  PRODUCTION SCHEDULE - Next 14 Days               │${NC}"
echo -e "${CYAN}├────────────────────────────────────────────────────┤${NC}"
echo -e "${CYAN}│                                                    │${NC}"
echo -e "${CYAN}│  Dec 5-6:  ${RED}[■■■■■■■■■■]${CYAN} WO-003 URGENT (50u)    │${NC}"
echo -e "${CYAN}│  Dec 7-8:  ${YELLOW}[░░░░░░░░░░]${CYAN} HOLD - Materials        │${NC}"
echo -e "${CYAN}│  Dec 9-10: ${RED}[■■■■■■■■■■]${CYAN} WO-001 HIGH (100u)      │${NC}"
echo -e "${CYAN}│  Dec 11-14:${YELLOW}[■■■■■■■■■■]${CYAN} WO-002 MEDIUM (200u)   │${NC}"
echo -e "${CYAN}│  Dec 15-18:${GREEN}[■■■■■░░░░░]${CYAN} WO-004 LOW (75u)       │${NC}"
echo -e "${CYAN}│  Dec 19-20:${YELLOW}[░░░░░░░░░░]${CYAN} Available capacity      │${NC}"
echo -e "${CYAN}│                                                    │${NC}"
echo -e "${CYAN}│  Legend:                                           │${NC}"
echo -e "${CYAN}│    ${RED}■${CYAN} High Priority    ${YELLOW}■${CYAN} Medium Priority         │${NC}"
echo -e "${CYAN}│    ${GREEN}■${CYAN} Low Priority     ${YELLOW}░${CYAN} On Hold/Available       │${NC}"
echo -e "${CYAN}│                                                    │${NC}"
echo -e "${CYAN}└────────────────────────────────────────────────────┘${NC}"
echo ""

#########################################################
# Summary
#########################################################

echo -e "${GREEN}Demo Complete!${NC}"
echo ""
echo -e "This demo showed:"
echo -e "  • Multi-work order creation with priorities"
echo -e "  • Material availability analysis"
echo -e "  • Production constraint identification"
echo -e "  • Optimized scheduling based on priorities"
echo -e "  • Material shortage handling (emergency PO)"
echo -e "  • Capacity planning and utilization"
echo -e "  • Risk identification and mitigation"
echo -e "  • Production timeline visualization"
echo ""
echo -e "Key Benefits:"
echo -e "  • Maximize throughput with priority-based scheduling"
echo -e "  • Identify material shortages early"
echo -e "  • Optimize resource allocation"
echo -e "  • Track capacity utilization"
echo -e "  • Reduce production delays"
echo -e "  • Improve on-time delivery"
echo ""
