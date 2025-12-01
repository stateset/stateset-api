#!/bin/bash

#########################################################
# StateSet Manufacturing API - Demo 2
# Quality Issue Management Workflow
#########################################################
#
# This demo shows a complete quality issue resolution cycle:
# 1. Robot fails a test
# 2. NCR is created to track the issue
# 3. Investigation identifies root cause
# 4. Corrective action is implemented
# 5. Robot is retested
# 6. NCR is closed with disposition
#
# This demonstrates:
# - Test failure handling
# - Non-Conformance Report (NCR) lifecycle
# - Root cause analysis
# - Rework process
# - Retest procedures
# - Quality issue resolution
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
API_BASE="http://localhost:3000/api/v1/manufacturing"
TOKEN="your_jwt_token_here"  # Replace with actual JWT token

# Test users (replace with actual user UUIDs from your system)
OPERATOR_ID="11111111-1111-1111-1111-111111111111"
QA_ENGINEER_ID="22222222-2222-2222-2222-222222222222"
MAINTENANCE_TECH_ID="33333333-3333-3333-3333-333333333333"

echo -e "${CYAN}================================================${NC}"
echo -e "${CYAN}  StateSet Manufacturing API - Demo 2${NC}"
echo -e "${CYAN}  Quality Issue Management Workflow${NC}"
echo -e "${CYAN}================================================${NC}"
echo ""

#########################################################
# STEP 1: Setup - Create Robot and Components
#########################################################

echo -e "${BLUE}[STEP 1]${NC} Setting up robot for testing..."
echo ""

# Create product reference (normally this would already exist)
PRODUCT_ID="aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"

# Create work order
WORK_ORDER_ID="bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"

# Create robot serial number
echo -e "${YELLOW}Creating robot serial number...${NC}"
ROBOT_RESPONSE=$(curl -s -X POST "$API_BASE/robots/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"serial_number\": \"IR6000-202412-00043\",
    \"product_id\": \"$PRODUCT_ID\",
    \"work_order_id\": \"$WORK_ORDER_ID\",
    \"robot_model\": \"IR-6000\",
    \"robot_type\": \"articulated_arm\",
    \"manufacturing_date\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"
  }")

ROBOT_ID=$(echo $ROBOT_RESPONSE | jq -r '.id')
echo -e "${GREEN}✓ Robot created: IR6000-202412-00043${NC}"
echo -e "  Robot ID: $ROBOT_ID"
echo ""

# Create and install components (simplified for demo)
echo -e "${YELLOW}Creating and installing components...${NC}"

# Create faulty encoder (this will cause test failure)
ENCODER_RESPONSE=$(curl -s -X POST "$API_BASE/components/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "ENC-2024-FAULTY-001",
    "component_type": "encoder",
    "component_sku": "ENC-6000-J3",
    "supplier_id": "sup-456",
    "supplier_lot_number": "LOT-2024-Q4-FAULTY",
    "manufacture_date": "2024-11-20",
    "receive_date": "2024-11-28",
    "location": "Warehouse-B-Bin-15"
  }')

ENCODER_ID=$(echo $ENCODER_RESPONSE | jq -r '.id')

# Install encoder
curl -s -X POST "$API_BASE/components/install" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$ENCODER_ID\",
    \"position\": \"joint_3\",
    \"installed_by\": \"$OPERATOR_ID\"
  }" > /dev/null

echo -e "${GREEN}✓ Components installed${NC}"
echo -e "  - Encoder ENC-2024-FAULTY-001 at joint_3"
echo ""

sleep 1

#########################################################
# STEP 2: Initial Testing - Discover Issue
#########################################################

echo -e "${BLUE}[STEP 2]${NC} Running positioning accuracy test..."
echo ""

# Get positioning test protocol
POSITIONING_TEST_ID="tp-002"  # Assuming this protocol exists

# Run positioning test - WILL FAIL
echo -e "${YELLOW}Executing positioning accuracy test...${NC}"
TEST_RESULT=$(curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"$POSITIONING_TEST_ID\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"work_order_id\": \"$WORK_ORDER_ID\",
    \"tested_by\": \"$OPERATOR_ID\",
    \"status\": \"fail\",
    \"measurements\": {
      \"joint_1_error_mm\": 0.03,
      \"joint_2_error_mm\": 0.04,
      \"joint_3_error_mm\": 0.85,
      \"joint_4_error_mm\": 0.02,
      \"joint_5_error_mm\": 0.03,
      \"joint_6_error_mm\": 0.02,
      \"max_allowed_error_mm\": 0.5,
      \"failed_joint\": \"joint_3\"
    },
    \"notes\": \"Joint 3 positioning error exceeds tolerance. Error measured at 0.85mm (spec: 0.5mm max). Possible encoder calibration issue.\"
  }")

TEST_RESULT_ID=$(echo $TEST_RESULT | jq -r '.id')

echo -e "${RED}✗ TEST FAILED - Positioning Accuracy Test${NC}"
echo -e "  Joint 3 error: ${RED}0.85mm${NC} (spec: ≤ 0.5mm)"
echo -e "  All other joints: ${GREEN}Within spec${NC}"
echo -e "  Test ID: $TEST_RESULT_ID"
echo ""

sleep 2

#########################################################
# STEP 3: Create Non-Conformance Report (NCR)
#########################################################

echo -e "${BLUE}[STEP 3]${NC} Creating Non-Conformance Report..."
echo ""

# Generate NCR number (in production, this would be auto-generated)
NCR_NUMBER="NCR-202412-00043"

echo -e "${YELLOW}Creating NCR for quality issue...${NC}"
NCR_RESPONSE=$(curl -s -X POST "$API_BASE/ncrs" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"ncr_number\": \"$NCR_NUMBER\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$ENCODER_ID\",
    \"work_order_id\": \"$WORK_ORDER_ID\",
    \"reported_by\": \"$OPERATOR_ID\",
    \"issue_type\": \"dimensional\",
    \"severity\": \"major\",
    \"description\": \"Joint 3 positioning accuracy exceeds tolerance by 0.35mm during positioning test. Measured error: 0.85mm (specification: 0.5mm maximum). Issue isolated to joint 3 encoder. All other joints within specification.\",
    \"detected_at_stage\": \"final_testing\",
    \"assigned_to\": \"$QA_ENGINEER_ID\"
  }")

NCR_ID=$(echo $NCR_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ NCR Created: $NCR_NUMBER${NC}"
echo -e "  NCR ID: $NCR_ID"
echo -e "  Severity: ${YELLOW}MAJOR${NC}"
echo -e "  Issue Type: Dimensional"
echo -e "  Assigned to: QA Engineer"
echo -e "  Status: ${RED}OPEN${NC}"
echo ""

sleep 2

#########################################################
# STEP 4: Investigation & Root Cause Analysis
#########################################################

echo -e "${BLUE}[STEP 4]${NC} Investigating root cause..."
echo ""

echo -e "${YELLOW}QA Engineer analyzing issue...${NC}"
sleep 2

# Update NCR with investigation findings
echo -e "${CYAN}Performing diagnostic checks:${NC}"
echo -e "  - Checking mechanical alignment... ${GREEN}OK${NC}"
echo -e "  - Checking cable connections... ${GREEN}OK${NC}"
echo -e "  - Checking encoder calibration... ${RED}OUT OF SPEC${NC}"
echo -e "  - Checking encoder firmware... ${GREEN}OK${NC}"
echo -e "  - Checking supplier lot... ${YELLOW}WARNING${NC}"
echo ""

sleep 2

# Update NCR with root cause
echo -e "${YELLOW}Updating NCR with root cause analysis...${NC}"
curl -s -X PUT "$API_BASE/ncrs/$NCR_ID" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"status\": \"investigating\",
    \"root_cause\": \"Joint 3 encoder from supplier lot LOT-2024-Q4-FAULTY arrived with incorrect factory calibration. Encoder offset values do not match specification datasheet. Manufacturing inspection records show this lot was flagged for expedited delivery and may have bypassed final supplier QC.\",
    \"investigation_notes\": \"Diagnostic testing revealed encoder calibration offset error of +0.42mm. Compared encoder EEPROM values against reference unit - significant deviation in zero-position offset. Contacted supplier - they confirmed this lot had calibration issues and issued recall notice yesterday. Two other encoders from same lot are in inventory and have been quarantined.\"
  }" > /dev/null

echo -e "${GREEN}✓ Root cause identified${NC}"
echo -e "  ${RED}Faulty encoder calibration from supplier${NC}"
echo -e "  Supplier lot: LOT-2024-Q4-FAULTY"
echo -e "  Other units quarantined"
echo ""

sleep 2

#########################################################
# STEP 5: Corrective Action Plan
#########################################################

echo -e "${BLUE}[STEP 5]${NC} Developing corrective action plan..."
echo ""

echo -e "${YELLOW}Updating NCR with corrective actions...${NC}"
curl -s -X PUT "$API_BASE/ncrs/$NCR_ID" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"status\": \"action_required\",
    \"corrective_action\": \"1. Remove faulty encoder ENC-2024-FAULTY-001 from joint 3\\n2. Install replacement encoder from verified good lot (LOT-2024-Q4-GOOD)\\n3. Perform encoder calibration procedure per WI-CAL-003\\n4. Verify calibration with reference gauge block\\n5. Re-run complete positioning accuracy test\\n6. If pass, proceed to full test suite\\n\\nPreventive Action:\\n- Added supplier lot verification to incoming inspection (SOP-INS-001)\\n- All encoders from LOT-2024-Q4-FAULTY quarantined pending supplier disposition\\n- Supplier corrective action request submitted\",
    \"preventive_action\": \"Update incoming inspection checklist to include encoder calibration spot-check for 10% of lots. Implement supplier scorecard review for quality issues. Schedule supplier audit within 30 days.\"
  }" > /dev/null

echo -e "${GREEN}✓ Corrective action plan approved${NC}"
echo -e "  Actions:"
echo -e "  1. Replace faulty encoder"
echo -e "  2. Calibrate replacement"
echo -e "  3. Retest positioning accuracy"
echo -e "  4. Run full test suite if pass"
echo ""
echo -e "  Preventive actions:"
echo -e "  - Enhanced incoming inspection"
echo -e "  - Supplier quality audit scheduled"
echo ""

sleep 2

#########################################################
# STEP 6: Execute Rework
#########################################################

echo -e "${BLUE}[STEP 6]${NC} Executing rework..."
echo ""

# Create replacement encoder
echo -e "${YELLOW}Obtaining replacement encoder...${NC}"
REPLACEMENT_ENCODER=$(curl -s -X POST "$API_BASE/components/serials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "serial_number": "ENC-2024-GOOD-789",
    "component_type": "encoder",
    "component_sku": "ENC-6000-J3",
    "supplier_id": "sup-456",
    "supplier_lot_number": "LOT-2024-Q4-GOOD",
    "manufacture_date": "2024-11-18",
    "receive_date": "2024-11-26",
    "location": "Warehouse-A-Bin-08"
  }')

REPLACEMENT_ENCODER_ID=$(echo $REPLACEMENT_ENCODER | jq -r '.id')
echo -e "${GREEN}✓ Replacement encoder obtained: ENC-2024-GOOD-789${NC}"
echo ""

# Remove faulty encoder (simulated - would update genealogy table)
echo -e "${YELLOW}Removing faulty encoder from joint 3...${NC}"
sleep 1
echo -e "${GREEN}✓ Faulty encoder removed${NC}"
echo ""

# Install replacement encoder
echo -e "${YELLOW}Installing replacement encoder...${NC}"
curl -s -X POST "$API_BASE/components/install" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"component_serial_id\": \"$REPLACEMENT_ENCODER_ID\",
    \"position\": \"joint_3\",
    \"installed_by\": \"$MAINTENANCE_TECH_ID\"
  }" > /dev/null

echo -e "${GREEN}✓ Replacement encoder installed${NC}"
echo ""

# Calibration process
echo -e "${YELLOW}Performing encoder calibration (WI-CAL-003)...${NC}"
sleep 2
echo -e "  Step 1: Zero position calibration... ${GREEN}OK${NC}"
sleep 1
echo -e "  Step 2: Reference position verification... ${GREEN}OK${NC}"
sleep 1
echo -e "  Step 3: Full range sweep... ${GREEN}OK${NC}"
sleep 1
echo -e "  Step 4: Repeatability test... ${GREEN}OK${NC}"
echo ""
echo -e "${GREEN}✓ Calibration complete - All checks passed${NC}"
echo ""

sleep 2

#########################################################
# STEP 7: Retest After Rework
#########################################################

echo -e "${BLUE}[STEP 7]${NC} Retesting after rework..."
echo ""

# Run positioning test again - WILL PASS
echo -e "${YELLOW}Re-running positioning accuracy test...${NC}"
RETEST_RESULT=$(curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"$POSITIONING_TEST_ID\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"work_order_id\": \"$WORK_ORDER_ID\",
    \"tested_by\": \"$QA_ENGINEER_ID\",
    \"status\": \"pass\",
    \"measurements\": {
      \"joint_1_error_mm\": 0.03,
      \"joint_2_error_mm\": 0.04,
      \"joint_3_error_mm\": 0.02,
      \"joint_4_error_mm\": 0.02,
      \"joint_5_error_mm\": 0.03,
      \"joint_6_error_mm\": 0.02,
      \"max_allowed_error_mm\": 0.5,
      \"all_joints_pass\": true
    },
    \"notes\": \"Retest after encoder replacement and calibration. All joints now within specification. Joint 3 error reduced from 0.85mm to 0.02mm. Test PASSED.\"
  }")

RETEST_ID=$(echo $RETEST_RESULT | jq -r '.id')

echo -e "${GREEN}✓ TEST PASSED - Positioning Accuracy Test${NC}"
echo -e "  Joint 3 error: ${GREEN}0.02mm${NC} (spec: ≤ 0.5mm)"
echo -e "  Previous: ${RED}0.85mm${NC} → Current: ${GREEN}0.02mm${NC}"
echo -e "  Improvement: ${GREEN}97.6%${NC}"
echo -e "  All joints: ${GREEN}Within spec${NC}"
echo -e "  Retest ID: $RETEST_ID"
echo ""

sleep 2

# Run additional verification tests
echo -e "${YELLOW}Running verification test suite...${NC}"
sleep 2

# Joint torque test
curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"tp-001\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"tested_by\": \"$QA_ENGINEER_ID\",
    \"status\": \"pass\",
    \"measurements\": {
      \"joint_1_torque\": 187.2,
      \"joint_2_torque\": 179.8,
      \"joint_3_torque\": 191.5,
      \"all_within_spec\": true
    }
  }" > /dev/null

echo -e "  ${GREEN}✓ Joint Torque Test - PASS${NC}"

# Repeatability test
curl -s -X POST "$API_BASE/test-results" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"test_protocol_id\": \"tp-005\",
    \"robot_serial_id\": \"$ROBOT_ID\",
    \"tested_by\": \"$QA_ENGINEER_ID\",
    \"status\": \"pass\",
    \"measurements\": {
      \"repeatability_mm\": 0.015,
      \"specification_mm\": 0.05,
      \"passes\": true
    }
  }" > /dev/null

echo -e "  ${GREEN}✓ Repeatability Test - PASS${NC}"
echo ""

echo -e "${GREEN}✓ All verification tests passed${NC}"
echo ""

sleep 2

#########################################################
# STEP 8: Close NCR
#########################################################

echo -e "${BLUE}[STEP 8]${NC} Closing Non-Conformance Report..."
echo ""

echo -e "${YELLOW}Finalizing NCR closure...${NC}"
CLOSE_RESPONSE=$(curl -s -X POST "$API_BASE/ncrs/$NCR_ID/close" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"resolution_notes\": \"Issue successfully resolved through component replacement and calibration.\\n\\nActions Completed:\\n- Faulty encoder ENC-2024-FAULTY-001 removed and quarantined\\n- Replacement encoder ENC-2024-GOOD-789 installed at joint 3\\n- Calibration procedure WI-CAL-003 completed successfully\\n- All calibration checks passed\\n- Positioning accuracy test re-run: PASS (joint 3 error 0.85mm → 0.02mm)\\n- Verification test suite completed: All tests PASS\\n\\nPreventive Actions Implemented:\\n- Incoming inspection updated to include encoder calibration spot-checks\\n- All units from lot LOT-2024-Q4-FAULTY quarantined (2 units)\\n- Supplier corrective action request submitted\\n- Supplier audit scheduled for 2025-01-15\\n\\nRobot IR6000-202412-00043 approved for continued production and final testing.\",
    \"disposition\": \"rework\",
    \"verification_notes\": \"Verified by QA Engineer. All corrective actions completed. Retest results confirm issue resolution. Robot meets all specifications.\"
  }")

echo -e "${GREEN}✓ NCR Closed: $NCR_NUMBER${NC}"
echo -e "  Status: ${GREEN}CLOSED${NC}"
echo -e "  Disposition: ${CYAN}REWORK${NC}"
echo -e "  Resolution: Component replacement successful"
echo -e "  Verification: All tests passed"
echo ""

sleep 1

#########################################################
# STEP 9: Summary & Metrics
#########################################################

echo -e "${BLUE}[STEP 9]${NC} Quality Issue Resolution Summary"
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  Quality Issue Resolution Complete${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

echo -e "${MAGENTA}Robot Information:${NC}"
echo -e "  Serial Number: IR6000-202412-00043"
echo -e "  Robot ID: $ROBOT_ID"
echo -e "  Model: IR-6000"
echo ""

echo -e "${MAGENTA}Issue Timeline:${NC}"
echo -e "  1. Test Failure Detected"
echo -e "     ${RED}✗${NC} Positioning test failed (Joint 3: 0.85mm error)"
echo -e ""
echo -e "  2. NCR Created"
echo -e "     NCR Number: $NCR_NUMBER"
echo -e "     Severity: ${YELLOW}MAJOR${NC}"
echo -e ""
echo -e "  3. Root Cause Identified"
echo -e "     ${RED}⚠${NC} Faulty encoder calibration from supplier"
echo -e "     Lot: LOT-2024-Q4-FAULTY"
echo -e ""
echo -e "  4. Corrective Action"
echo -e "     ${GREEN}✓${NC} Encoder replaced (ENC-2024-GOOD-789)"
echo -e "     ${GREEN}✓${NC} Calibration completed"
echo -e ""
echo -e "  5. Verification"
echo -e "     ${GREEN}✓${NC} Retest: PASS (0.02mm error)"
echo -e "     ${GREEN}✓${NC} Improvement: 97.6%"
echo -e "     ${GREEN}✓${NC} Additional tests: All PASS"
echo -e ""
echo -e "  6. NCR Closed"
echo -e "     Status: ${GREEN}CLOSED${NC}"
echo -e "     Disposition: REWORK"
echo ""

echo -e "${MAGENTA}Quality Metrics:${NC}"
echo -e "  Issue Detection: Final Testing (before shipment)"
echo -e "  Resolution Time: ~2 hours (estimated)"
echo -e "  Cost Impact: Component replacement + labor"
echo -e "  Customer Impact: ${GREEN}None (caught before shipment)${NC}"
echo ""

echo -e "${MAGENTA}Preventive Actions:${NC}"
echo -e "  ${GREEN}✓${NC} Enhanced incoming inspection procedures"
echo -e "  ${GREEN}✓${NC} Supplier quality issue escalation"
echo -e "  ${GREEN}✓${NC} Faulty lot quarantined (2 additional units)"
echo -e "  ${GREEN}✓${NC} Supplier audit scheduled (2025-01-15)"
echo ""

echo -e "${MAGENTA}Final Status:${NC}"
echo -e "  Robot: ${GREEN}APPROVED FOR PRODUCTION${NC}"
echo -e "  Quality: ${GREEN}MEETS ALL SPECIFICATIONS${NC}"
echo -e "  Ready for: Final test suite & shipment prep"
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

echo -e "${GREEN}Demo Complete!${NC}"
echo ""
echo -e "This demo showed:"
echo -e "  • Test failure detection and handling"
echo -e "  • NCR creation and tracking"
echo -e "  • Root cause analysis methodology"
echo -e "  • Corrective action implementation"
echo -e "  • Rework and component replacement"
echo -e "  • Retest verification"
echo -e "  • NCR closure with full documentation"
echo -e "  • Preventive action implementation"
echo ""
echo -e "Key Benefits:"
echo -e "  • Complete traceability of quality issues"
echo -e "  • Structured problem-solving workflow"
echo -e "  • Prevention of customer escapes"
echo -e "  • Continuous improvement through preventive actions"
echo -e "  • Supplier quality management"
echo ""
