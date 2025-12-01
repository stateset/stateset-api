#!/bin/bash

# ============================================================================
# DEMO 1: Complete Robot Build & Test Workflow
# ============================================================================
# This demo shows the complete lifecycle of building an industrial robot:
# 1. Receive components from suppliers
# 2. Create robot serial number
# 3. Install components with traceability
# 4. Run test protocols
# 5. Add certifications
# 6. Mark ready for shipment
#
# Robot: IR-6000 Industrial Articulated Arm
# - 6 degrees of freedom
# - 50kg payload
# - 3000mm reach
# ============================================================================

set -e  # Exit on error

API_BASE="http://localhost:8080/api/v1/manufacturing"
TOKEN="your_jwt_token_here"  # Replace with actual token

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   Robot Manufacturing Demo${NC}"
echo -e "${BLUE}   Building IR-6000 Articulated Arm${NC}"
echo -e "${BLUE}========================================${NC}\n"

# ============================================================================
# STEP 1: Receive Components from Suppliers
# ============================================================================
echo -e "${GREEN}Step 1: Receiving Components from Suppliers${NC}"
echo "Creating component serial numbers for incoming parts..."

# Servo Motor for Joint 1
MOTOR_1=$(curl -s -X POST "$API_BASE/components/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "MOTOR-2024-12345",
    "component_type": "servo_motor",
    "component_sku": "MTR-6000-J1",
    "supplier_id": "550e8400-e29b-41d4-a716-446655440001",
    "supplier_lot_number": "LOT-2024-Q4-001",
    "manufacture_date": "2024-11-15",
    "receive_date": "2024-11-25",
    "location": "Warehouse-A-Bin-42"
  }' | jq -r '.id')

echo "  ✓ Motor 1 created: $MOTOR_1"

# Servo Motor for Joint 2
MOTOR_2=$(curl -s -X POST "$API_BASE/components/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "MOTOR-2024-12346",
    "component_type": "servo_motor",
    "component_sku": "MTR-6000-J2",
    "supplier_id": "550e8400-e29b-41d4-a716-446655440001",
    "supplier_lot_number": "LOT-2024-Q4-001",
    "manufacture_date": "2024-11-15",
    "receive_date": "2024-11-25",
    "location": "Warehouse-A-Bin-42"
  }' | jq -r '.id')

echo "  ✓ Motor 2 created: $MOTOR_2"

# Controller
CONTROLLER=$(curl -s -X POST "$API_BASE/components/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "CTRL-2024-98765",
    "component_type": "controller",
    "component_sku": "CTRL-6000",
    "supplier_id": "550e8400-e29b-41d4-a716-446655440002",
    "supplier_lot_number": "LOT-2024-Q4-010",
    "manufacture_date": "2024-11-20",
    "receive_date": "2024-11-28",
    "location": "Warehouse-A-Bin-15"
  }' | jq -r '.id')

echo "  ✓ Controller created: $CONTROLLER"

# Safety PLC
SAFETY_PLC=$(curl -s -X POST "$API_BASE/components/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "PLC-2024-54321",
    "component_type": "safety_plc",
    "component_sku": "PLC-SAFE-6000",
    "supplier_id": "550e8400-e29b-41d4-a716-446655440003",
    "supplier_lot_number": "LOT-2024-Q4-015",
    "manufacture_date": "2024-11-18",
    "receive_date": "2024-11-27",
    "location": "Warehouse-A-Bin-08"
  }' | jq -r '.id')

echo "  ✓ Safety PLC created: $SAFETY_PLC"

# Encoder
ENCODER=$(curl -s -X POST "$API_BASE/components/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "ENC-2024-11111",
    "component_type": "encoder",
    "component_sku": "ENC-HIGH-RES-6000",
    "supplier_id": "550e8400-e29b-41d4-a716-446655440004",
    "supplier_lot_number": "LOT-2024-Q4-020",
    "manufacture_date": "2024-11-10",
    "receive_date": "2024-11-22",
    "location": "Warehouse-A-Bin-25"
  }' | jq -r '.id')

echo -e "  ✓ Encoder created: $ENCODER\n"

# ============================================================================
# STEP 2: Create Robot Serial Number
# ============================================================================
echo -e "${GREEN}Step 2: Creating Robot Serial Number${NC}"
echo "Generating serial number for new IR-6000 robot..."

ROBOT=$(curl -s -X POST "$API_BASE/robots/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "IR6000-202412-00042",
    "robot_model": "IR-6000",
    "robot_type": "articulated_arm",
    "product_id": "650e8400-e29b-41d4-a716-446655440001",
    "work_order_id": "750e8400-e29b-41d4-a716-446655440001",
    "manufacturing_date": "2024-12-01T08:00:00Z"
  }')

ROBOT_ID=$(echo $ROBOT | jq -r '.id')
ROBOT_SERIAL=$(echo $ROBOT | jq -r '.serial_number')

echo -e "  ✓ Robot created: $ROBOT_SERIAL (ID: $ROBOT_ID)\n"

# ============================================================================
# STEP 3: Install Components (Building the Robot)
# ============================================================================
echo -e "${GREEN}Step 3: Installing Components${NC}"
echo "Installing components into robot with traceability..."

# Install Motor 1
curl -s -X POST "$API_BASE/components/install" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$MOTOR_1\",
    \"position\": \"joint_1\",
    \"installed_by\": \"850e8400-e29b-41d4-a716-446655440001\"
  }" > /dev/null

echo "  ✓ Motor 1 installed at joint_1"

# Install Motor 2
curl -s -X POST "$API_BASE/components/install" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$MOTOR_2\",
    \"position\": \"joint_2\",
    \"installed_by\": \"850e8400-e29b-41d4-a716-446655440001\"
  }" > /dev/null

echo "  ✓ Motor 2 installed at joint_2"

# Install Controller
curl -s -X POST "$API_BASE/components/install" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$CONTROLLER\",
    \"position\": \"main_controller\",
    \"installed_by\": \"850e8400-e29b-41d4-a716-446655440001\"
  }" > /dev/null

echo "  ✓ Controller installed"

# Install Safety PLC
curl -s -X POST "$API_BASE/components/install" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$SAFETY_PLC\",
    \"position\": \"safety_controller\",
    \"installed_by\": \"850e8400-e29b-41d4-a716-446655440001\"
  }" > /dev/null

echo "  ✓ Safety PLC installed"

# Install Encoder
curl -s -X POST "$API_BASE/components/install" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$ENCODER\",
    \"position\": \"joint_1_encoder\",
    \"installed_by\": \"850e8400-e29b-41d4-a716-446655440001\"
  }" > /dev/null

echo -e "  ✓ Encoder installed\n"

# ============================================================================
# STEP 4: Run Test Protocols
# ============================================================================
echo -e "${GREEN}Step 4: Running Test Protocols${NC}"
echo "Executing quality control tests..."

# Get available test protocols
PROTOCOLS=$(curl -s -X GET "$API_BASE/test-protocols" \
  -H "Authorization: Bearer $TOKEN")

echo "Available test protocols:"
echo "$PROTOCOLS" | jq -r '.[] | "  - \(.protocol_number): \(.name)"'
echo ""

# Run Joint Torque Test
echo "Running TP-001: Joint Torque Test..."
TORQUE_TEST=$(curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"$(echo $PROTOCOLS | jq -r '.[0].id')\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"tested_by\": \"850e8400-e29b-41d4-a716-446655440001\",
    \"status\": \"pass\",
    \"measurements\": {
      \"joint_1_torque_nm\": 185.5,
      \"joint_2_torque_nm\": 178.2,
      \"joint_3_torque_nm\": 192.1,
      \"joint_4_torque_nm\": 88.5,
      \"joint_5_torque_nm\": 91.2,
      \"joint_6_torque_nm\": 45.8,
      \"all_within_spec\": true
    },
    \"notes\": \"All joints within torque specification (50-200 Nm)\"
  }")

echo "  ✓ Torque Test: PASSED"

# Run Positioning Accuracy Test
echo "Running TP-002: Positioning Accuracy Test..."
curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"$(echo $PROTOCOLS | jq -r '.[1].id')\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"tested_by\": \"850e8400-e29b-41d4-a716-446655440001\",
    \"status\": \"pass\",
    \"measurements\": {
      \"repeatability_mm\": 0.03,
      \"accuracy_mm\": 0.08,
      \"max_deviation_mm\": 0.09,
      \"test_points\": 10,
      \"cycles\": 5
    },
    \"notes\": \"Repeatability: ±0.03mm, Accuracy: ±0.08mm - within spec\"
  }" > /dev/null

echo "  ✓ Positioning Test: PASSED"

# Run Safety System Test
echo "Running TP-003: Safety System Test..."
curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"$(echo $PROTOCOLS | jq -r '.[2].id')\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"tested_by\": \"850e8400-e29b-41d4-a716-446655440001\",
    \"status\": \"pass\",
    \"measurements\": {
      \"emergency_stop_response_ms\": 85,
      \"light_curtain_response_ms\": 92,
      \"safe_torque_off_response_ms\": 78,
      \"enabling_device_test\": \"pass\",
      \"stop_distance_mm\": 42
    },
    \"notes\": \"All safety systems operational. Response times within spec (<100ms)\"
  }" > /dev/null

echo "  ✓ Safety Test: PASSED"

# Run Controller Test
echo "Running TP-004: Controller Functional Test..."
curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"$(echo $PROTOCOLS | jq -r '.[3].id')\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"tested_by\": \"850e8400-e29b-41d4-a716-446655440001\",
    \"status\": \"pass\",
    \"measurements\": {
      \"voltage_v\": 24.2,
      \"io_signals_tested\": 32,
      \"io_signals_passed\": 32,
      \"communication_success_rate\": 100,
      \"error_log_count\": 0
    },
    \"notes\": \"Controller fully operational. All I/O signals functional.\"
  }" > /dev/null

echo "  ✓ Controller Test: PASSED"

# Run Software Integration Test
echo "Running TP-005: Software Integration Test..."
curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"$(echo $PROTOCOLS | jq -r '.[4].id')\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"tested_by\": \"850e8400-e29b-41d4-a716-446655440001\",
    \"status\": \"pass\",
    \"measurements\": {
      \"trajectory_error_mm\": 0.5,
      \"cycle_time_variance_percent\": 2.1,
      \"motion_sequences_tested\": 15,
      \"motion_sequences_passed\": 15,
      \"firmware_version\": \"v2.5.1\",
      \"motion_control_version\": \"v3.2.0\"
    },
    \"notes\": \"Software integration successful. All motion sequences executed correctly.\"
  }" > /dev/null

echo -e "  ✓ Software Test: PASSED\n"

# ============================================================================
# STEP 5: Add Certifications
# ============================================================================
echo -e "${GREEN}Step 5: Adding Certifications${NC}"
echo "Recording safety certifications..."

# CE Certification
curl -s -X POST "$API_BASE/robots/$ROBOT_ID/certifications" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "robot_serial_id": "'$ROBOT_ID'",
    "certification_type": "CE",
    "certification_number": "CE-IR6000-2024-042",
    "issuing_authority": "TÜV SÜD",
    "issue_date": "2024-12-01",
    "expiration_date": "2029-12-01",
    "certification_scope": "Machinery Directive 2006/42/EC",
    "certificate_document_url": "/docs/certs/CE-IR6000-2024-042.pdf"
  }' > /dev/null

echo "  ✓ CE Certification added"

# UL Certification
curl -s -X POST "$API_BASE/robots/$ROBOT_ID/certifications" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "robot_serial_id": "'$ROBOT_ID'",
    "certification_type": "UL",
    "certification_number": "UL-1740-2024-042",
    "issuing_authority": "Underwriters Laboratories",
    "issue_date": "2024-12-01",
    "expiration_date": "2029-12-01",
    "certification_scope": "UL 1740 Industrial Robots",
    "certificate_document_url": "/docs/certs/UL-1740-2024-042.pdf"
  }' > /dev/null

echo "  ✓ UL Certification added"

# ISO Certification
curl -s -X POST "$API_BASE/robots/$ROBOT_ID/certifications" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "robot_serial_id": "'$ROBOT_ID'",
    "certification_type": "ISO",
    "certification_number": "ISO-10218-2024-042",
    "issuing_authority": "International Organization for Standardization",
    "issue_date": "2024-12-01",
    "expiration_date": "2029-12-01",
    "certification_scope": "ISO 10218-1:2011 Robots and robotic devices - Safety requirements",
    "certificate_document_url": "/docs/certs/ISO-10218-2024-042.pdf"
  }' > /dev/null

echo -e "  ✓ ISO Certification added\n"

# ============================================================================
# STEP 6: Mark Ready for Shipment
# ============================================================================
echo -e "${GREEN}Step 6: Marking Robot Ready for Shipment${NC}"
echo "Updating robot status to 'ready'..."

curl -s -X PUT "$API_BASE/robots/serials/$ROBOT_ID" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "status": "ready",
    "manufacturing_date": "2024-12-01T08:00:00Z"
  }' > /dev/null

echo -e "  ✓ Robot marked as READY for shipment\n"

# ============================================================================
# STEP 7: Get Complete Robot Profile
# ============================================================================
echo -e "${GREEN}Step 7: Generating Robot Documentation${NC}"
echo "Retrieving complete robot genealogy and test results..."

GENEALOGY=$(curl -s -X GET "$API_BASE/robots/serials/$ROBOT_ID/genealogy" \
  -H "Authorization: Bearer $TOKEN")

echo ""
echo -e "${YELLOW}========================================${NC}"
echo -e "${YELLOW}   ROBOT MANUFACTURING COMPLETE${NC}"
echo -e "${YELLOW}========================================${NC}"
echo ""
echo "Robot Serial: $ROBOT_SERIAL"
echo "Robot Model: $(echo $GENEALOGY | jq -r '.robot_model')"
echo "Status: READY FOR SHIPMENT"
echo ""
echo "Installed Components:"
echo "$GENEALOGY" | jq -r '.components[] | "  • \(.component_type) (\(.component_sku)): \(.component_serial_number) at \(.position)"'
echo ""
echo "Test Results: 5/5 PASSED (100% pass rate)"
echo "  ✓ Joint Torque Test"
echo "  ✓ Positioning Accuracy Test"
echo "  ✓ Safety System Test"
echo "  ✓ Controller Functional Test"
echo "  ✓ Software Integration Test"
echo ""
echo "Certifications:"
echo "  ✓ CE (Expires: 2029-12-01)"
echo "  ✓ UL (Expires: 2029-12-01)"
echo "  ✓ ISO (Expires: 2029-12-01)"
echo ""
echo -e "${GREEN}✓ Robot ready for customer delivery!${NC}"
echo ""

# Get test results summary
TEST_RESULTS=$(curl -s -X GET "$API_BASE/robots/$ROBOT_ID/test-results" \
  -H "Authorization: Bearer $TOKEN")

echo "Detailed Test Results:"
echo "$TEST_RESULTS" | jq -r '.[] | "  • \(.test_protocol_name // "Test"): \(.status) (\(.test_date))"'
echo ""

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}   Demo Complete!${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Next steps:"
echo "  1. Review robot documentation at /robots/serials/$ROBOT_ID"
echo "  2. Generate shipping documentation"
echo "  3. Create customer service portal account"
echo "  4. Ship robot to customer"
echo ""
