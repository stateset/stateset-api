#!/bin/bash

#########################################################
# StateSet Manufacturing API - Demo 5
# Batch Production & Lot Tracking
#########################################################
#
# This demo shows batch/lot production workflow:
# 1. Create production batches with lot numbers
# 2. Track raw materials by lot
# 3. Record batch-level quality metrics
# 4. Implement lot traceability
# 5. Handle batch genealogy
# 6. Manage batch expiration and shelf life
#
# This demonstrates:
# - Batch manufacturing workflows
# - Lot number tracking and traceability
# - Batch quality control
# - Raw material lot tracking
# - Genealogy and forward/backward traceability
# - Expiration date management
# - Batch yield tracking
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

echo -e "${CYAN}================================================${NC}"
echo -e "${CYAN}  StateSet Manufacturing API - Demo 5${NC}"
echo -e "${CYAN}  Batch Production & Lot Tracking${NC}"
echo -e "${CYAN}================================================${NC}"
echo ""

#########################################################
# STEP 1: Setup - Define Product and Raw Materials
#########################################################

echo -e "${BLUE}[STEP 1]${NC} Setting up batch production environment..."
echo ""

# Product: Pharmaceutical tablets (requires strict lot tracking)
PRODUCT_NAME="Vitamin-C Tablets 500mg"
PRODUCT_SKU="VITC-500-TAB"
PRODUCT_ID="prod-vitc-500"

# Raw materials with lot numbers
RAW_MATERIAL_1="Ascorbic Acid (Active Ingredient)"
RAW_MATERIAL_1_LOT="RM-AA-2024-Q4-001"
RAW_MATERIAL_1_ID="rm-aa-001"

RAW_MATERIAL_2="Microcrystalline Cellulose (Excipient)"
RAW_MATERIAL_2_LOT="RM-MCC-2024-Q4-015"
RAW_MATERIAL_2_ID="rm-mcc-001"

RAW_MATERIAL_3="Magnesium Stearate (Lubricant)"
RAW_MATERIAL_3_LOT="RM-MS-2024-Q4-008"
RAW_MATERIAL_3_ID="rm-ms-001"

echo -e "${GREEN}Product Configuration:${NC}"
echo -e "  Product: ${PRODUCT_NAME}"
echo -e "  SKU: ${PRODUCT_SKU}"
echo -e "  Batch Size: 100,000 tablets"
echo ""

echo -e "${GREEN}Raw Materials with Lot Numbers:${NC}"
echo -e "  1. ${RAW_MATERIAL_1}"
echo -e "     Lot: ${RAW_MATERIAL_1_LOT}"
echo -e "     Expiry: 2026-06-30"
echo -e "     Qty Available: 50 kg"
echo ""
echo -e "  2. ${RAW_MATERIAL_2}"
echo -e "     Lot: ${RAW_MATERIAL_2_LOT}"
echo -e "     Expiry: 2027-03-15"
echo -e "     Qty Available: 200 kg"
echo ""
echo -e "  3. ${RAW_MATERIAL_3}"
echo -e "     Lot: ${RAW_MATERIAL_3_LOT}"
echo -e "     Expiry: 2026-12-31"
echo -e "     Qty Available: 10 kg"
echo ""

sleep 2

#########################################################
# STEP 2: Create Batch Production Record
#########################################################

echo -e "${BLUE}[STEP 2]${NC} Creating batch production record..."
echo ""

# Generate batch number using current date
BATCH_NUMBER="BATCH-VITC-$(date +%Y%m%d)-001"
PRODUCTION_DATE=$(date +%Y-%m-%d)
EXPIRY_DATE=$(date -d "+2 years" +%Y-%m-%d)

echo -e "${YELLOW}Generating batch number: ${BATCH_NUMBER}${NC}"

BATCH_RESPONSE=$(curl -s -X POST "$API_BASE/manufacturing/batches" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "batch_number": "'$BATCH_NUMBER'",
    "product_id": "'$PRODUCT_ID'",
    "product_name": "'$PRODUCT_NAME'",
    "batch_size": 100000,
    "production_date": "'$PRODUCTION_DATE'",
    "expiry_date": "'$EXPIRY_DATE'",
    "status": "PLANNED",
    "location_id": 100,
    "operator_id": "operator-001",
    "manufacturing_line": "Line-A",
    "shift": "Day-Shift-1"
  }')

BATCH_ID=$(echo $BATCH_RESPONSE | jq -r '.id')

echo -e "${GREEN}✓ Batch record created${NC}"
echo -e "  Batch Number: ${BATCH_NUMBER}"
echo -e "  Batch ID: ${BATCH_ID}"
echo -e "  Product: ${PRODUCT_NAME}"
echo -e "  Batch Size: 100,000 tablets"
echo -e "  Production Date: ${PRODUCTION_DATE}"
echo -e "  Expiry Date: ${EXPIRY_DATE}"
echo -e "  Status: ${YELLOW}PLANNED${NC}"
echo ""

sleep 2

#########################################################
# STEP 3: Record Raw Material Lots
#########################################################

echo -e "${BLUE}[STEP 3]${NC} Recording raw material lots used in batch..."
echo ""

echo -e "${YELLOW}Recording lot traceability...${NC}"

# Record Ascorbic Acid lot
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/materials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "material_id": "'$RAW_MATERIAL_1_ID'",
    "material_name": "'$RAW_MATERIAL_1'",
    "lot_number": "'$RAW_MATERIAL_1_LOT'",
    "quantity_used": 25.0,
    "unit": "kg",
    "expiry_date": "2026-06-30",
    "supplier": "ChemCorp International",
    "coa_reference": "COA-AA-2024-Q4-001"
  }' > /dev/null

echo -e "  ${GREEN}✓${NC} ${RAW_MATERIAL_1}"
echo -e "     Lot: ${RAW_MATERIAL_1_LOT}"
echo -e "     Quantity: 25 kg"
echo -e "     COA: COA-AA-2024-Q4-001"
echo ""

# Record Microcrystalline Cellulose lot
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/materials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "material_id": "'$RAW_MATERIAL_2_ID'",
    "material_name": "'$RAW_MATERIAL_2'",
    "lot_number": "'$RAW_MATERIAL_2_LOT'",
    "quantity_used": 45.0,
    "unit": "kg",
    "expiry_date": "2027-03-15",
    "supplier": "Excipients Plus",
    "coa_reference": "COA-MCC-2024-Q4-015"
  }' > /dev/null

echo -e "  ${GREEN}✓${NC} ${RAW_MATERIAL_2}"
echo -e "     Lot: ${RAW_MATERIAL_2_LOT}"
echo -e "     Quantity: 45 kg"
echo -e "     COA: COA-MCC-2024-Q4-015"
echo ""

# Record Magnesium Stearate lot
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/materials" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "material_id": "'$RAW_MATERIAL_3_ID'",
    "material_name": "'$RAW_MATERIAL_3'",
    "lot_number": "'$RAW_MATERIAL_3_LOT'",
    "quantity_used": 2.0,
    "unit": "kg",
    "expiry_date": "2026-12-31",
    "supplier": "PharmaGrade Materials",
    "coa_reference": "COA-MS-2024-Q4-008"
  }' > /dev/null

echo -e "  ${GREEN}✓${NC} ${RAW_MATERIAL_3}"
echo -e "     Lot: ${RAW_MATERIAL_3_LOT}"
echo -e "     Quantity: 2 kg"
echo -e "     COA: COA-MS-2024-Q4-008"
echo ""

echo -e "${GREEN}✓ All raw material lots recorded${NC}"
echo ""

sleep 2

#########################################################
# STEP 4: Start Batch Production
#########################################################

echo -e "${BLUE}[STEP 4]${NC} Starting batch production..."
echo ""

echo -e "${YELLOW}Updating batch status to IN_PROGRESS...${NC}"

curl -s -X PUT "$API_BASE/manufacturing/batches/$BATCH_ID/start" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "started_by": "operator-001",
    "start_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "equipment_id": "TABLET-PRESS-01",
    "notes": "Batch production started. All materials verified and line cleared."
  }' > /dev/null

echo -e "${GREEN}✓ Batch production started${NC}"
echo -e "  Status: ${CYAN}IN_PROGRESS${NC}"
echo -e "  Started by: Operator-001"
echo -e "  Equipment: TABLET-PRESS-01"
echo -e "  Manufacturing Line: Line-A"
echo ""

sleep 2

#########################################################
# STEP 5: Record In-Process Quality Checks
#########################################################

echo -e "${BLUE}[STEP 5]${NC} Recording in-process quality checks..."
echo ""

echo -e "${YELLOW}Performing in-process quality control...${NC}"
echo ""

# Weight variation test
echo -e "  ${CYAN}Test 1: Weight Variation Test${NC}"
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/quality-checks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "test_name": "Weight Variation",
    "test_type": "in_process",
    "sample_size": 20,
    "acceptance_criteria": "±5% of target weight (625mg ±31.25mg)",
    "results": {
      "average_weight_mg": 626.3,
      "min_weight_mg": 602.1,
      "max_weight_mg": 647.8,
      "standard_deviation": 12.4,
      "within_spec": true
    },
    "status": "PASS",
    "tested_by": "qc-analyst-001",
    "test_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
  }' > /dev/null

echo -e "     Result: ${GREEN}PASS${NC}"
echo -e "     Average weight: 626.3 mg (spec: 625 ±31.25 mg)"
echo -e "     Range: 602.1 - 647.8 mg"
echo ""

# Hardness test
echo -e "  ${CYAN}Test 2: Tablet Hardness Test${NC}"
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/quality-checks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "test_name": "Hardness Test",
    "test_type": "in_process",
    "sample_size": 10,
    "acceptance_criteria": "5-7 kp (kiloponds)",
    "results": {
      "average_hardness_kp": 5.8,
      "min_hardness_kp": 5.2,
      "max_hardness_kp": 6.4,
      "within_spec": true
    },
    "status": "PASS",
    "tested_by": "qc-analyst-001",
    "test_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
  }' > /dev/null

echo -e "     Result: ${GREEN}PASS${NC}"
echo -e "     Average hardness: 5.8 kp (spec: 5-7 kp)"
echo -e "     Range: 5.2 - 6.4 kp"
echo ""

# Dissolution test
echo -e "  ${CYAN}Test 3: Dissolution Test${NC}"
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/quality-checks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "test_name": "Dissolution Test",
    "test_type": "in_process",
    "sample_size": 6,
    "acceptance_criteria": "Not less than 80% (Q) in 30 minutes",
    "results": {
      "dissolution_30min_percent": 92.3,
      "individual_results": [91.2, 93.1, 92.8, 91.5, 93.9, 91.7],
      "within_spec": true
    },
    "status": "PASS",
    "tested_by": "qc-analyst-002",
    "test_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
  }' > /dev/null

echo -e "     Result: ${GREEN}PASS${NC}"
echo -e "     Dissolution at 30 min: 92.3% (spec: ≥80%)"
echo -e "     All individual values: 91.2-93.9%"
echo ""

echo -e "${GREEN}✓ All in-process quality checks passed${NC}"
echo ""

sleep 2

#########################################################
# STEP 6: Complete Batch Production
#########################################################

echo -e "${BLUE}[STEP 6]${NC} Completing batch production..."
echo ""

echo -e "${YELLOW}Recording batch completion...${NC}"

curl -s -X PUT "$API_BASE/manufacturing/batches/$BATCH_ID/complete" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "completed_by": "operator-001",
    "completion_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "actual_quantity_produced": 98500,
    "yield_percentage": 98.5,
    "production_notes": "Batch completed successfully. Minor tablet losses during compression startup (1500 units). All quality checks passed."
  }' > /dev/null

echo -e "${GREEN}✓ Batch production completed${NC}"
echo -e "  Status: ${GREEN}COMPLETED${NC}"
echo -e "  Target quantity: 100,000 tablets"
echo -e "  Actual quantity: 98,500 tablets"
echo -e "  Yield: ${GREEN}98.5%${NC}"
echo -e "  Production time: ~4 hours"
echo ""

sleep 2

#########################################################
# STEP 7: Final Quality Release Testing
#########################################################

echo -e "${BLUE}[STEP 7]${NC} Performing final quality release testing..."
echo ""

echo -e "${YELLOW}Running final release tests...${NC}"
echo ""

# Assay test (potency)
echo -e "  ${CYAN}Test 1: Assay (Potency)${NC}"
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/quality-checks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "test_name": "Assay (Active Ingredient Content)",
    "test_type": "final_release",
    "acceptance_criteria": "95.0% - 105.0% of label claim (500mg)",
    "results": {
      "assay_percent": 101.2,
      "actual_content_mg": 506.0,
      "within_spec": true
    },
    "status": "PASS",
    "tested_by": "qc-analyst-003",
    "test_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "method": "HPLC-UV"
  }' > /dev/null

echo -e "     Result: ${GREEN}PASS${NC}"
echo -e "     Assay: 101.2% (spec: 95-105%)"
echo -e "     Actual content: 506.0 mg per tablet"
echo -e "     Method: HPLC-UV"
echo ""

# Microbial limits test
echo -e "  ${CYAN}Test 2: Microbial Limits${NC}"
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/quality-checks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "test_name": "Microbial Limits",
    "test_type": "final_release",
    "acceptance_criteria": "Total aerobic count: <1000 CFU/g, Yeast/Mold: <100 CFU/g",
    "results": {
      "total_aerobic_count": 12,
      "yeast_mold_count": 3,
      "within_spec": true,
      "specific_pathogens": "Not detected"
    },
    "status": "PASS",
    "tested_by": "micro-analyst-001",
    "test_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
  }' > /dev/null

echo -e "     Result: ${GREEN}PASS${NC}"
echo -e "     Total aerobic: 12 CFU/g (spec: <1000 CFU/g)"
echo -e "     Yeast/Mold: 3 CFU/g (spec: <100 CFU/g)"
echo -e "     Pathogens: Not detected"
echo ""

# Uniformity of dosage units
echo -e "  ${CYAN}Test 3: Uniformity of Dosage Units${NC}"
curl -s -X POST "$API_BASE/manufacturing/batches/$BATCH_ID/quality-checks" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "test_name": "Uniformity of Dosage Units (Content Uniformity)",
    "test_type": "final_release",
    "sample_size": 30,
    "acceptance_criteria": "AV ≤ 15.0",
    "results": {
      "acceptance_value": 8.2,
      "mean_content_percent": 101.5,
      "rsd_percent": 3.1,
      "within_spec": true
    },
    "status": "PASS",
    "tested_by": "qc-analyst-003",
    "test_time": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"
  }' > /dev/null

echo -e "     Result: ${GREEN}PASS${NC}"
echo -e "     Acceptance Value: 8.2 (spec: ≤15.0)"
echo -e "     Mean content: 101.5%"
echo -e "     RSD: 3.1%"
echo ""

echo -e "${GREEN}✓ All final release tests passed${NC}"
echo ""

sleep 2

#########################################################
# STEP 8: Approve Batch for Release
#########################################################

echo -e "${BLUE}[STEP 8]${NC} Approving batch for release..."
echo ""

echo -e "${YELLOW}QA Manager reviewing batch records...${NC}"

sleep 2

curl -s -X PUT "$API_BASE/manufacturing/batches/$BATCH_ID/release" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "released_by": "qa-manager-001",
    "release_date": "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'",
    "release_status": "APPROVED",
    "review_notes": "Batch review complete. All manufacturing records, quality checks, and test results reviewed and found satisfactory. Batch approved for commercial distribution.",
    "certificate_of_analysis_id": "COA-'$BATCH_NUMBER'"
  }' > /dev/null

echo -e "${GREEN}✓ Batch approved for release${NC}"
echo -e "  Status: ${GREEN}RELEASED${NC}"
echo -e "  Released by: QA Manager"
echo -e "  Release date: $(date +%Y-%m-%d)"
echo -e "  COA: COA-${BATCH_NUMBER}"
echo ""

sleep 2

#########################################################
# STEP 9: Generate Batch Genealogy Report
#########################################################

echo -e "${BLUE}[STEP 9]${NC} Generating complete batch genealogy report..."
echo ""

GENEALOGY=$(curl -s -X GET "$API_BASE/manufacturing/batches/$BATCH_ID/genealogy" \
  -H "Authorization: Bearer $TOKEN")

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo -e "${CYAN}  BATCH GENEALOGY REPORT${NC}"
echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

echo -e "${MAGENTA}Batch Information:${NC}"
echo -e "  Batch Number: ${BATCH_NUMBER}"
echo -e "  Product: ${PRODUCT_NAME}"
echo -e "  Batch Size: 98,500 tablets (98.5% yield)"
echo -e "  Production Date: ${PRODUCTION_DATE}"
echo -e "  Expiry Date: ${EXPIRY_DATE}"
echo -e "  Status: ${GREEN}RELEASED${NC}"
echo ""

echo -e "${MAGENTA}Raw Material Traceability:${NC}"
echo -e "  1. Ascorbic Acid"
echo -e "     Lot: ${RAW_MATERIAL_1_LOT}"
echo -e "     Quantity: 25 kg"
echo -e "     Supplier: ChemCorp International"
echo -e "     COA: COA-AA-2024-Q4-001"
echo -e "     Expiry: 2026-06-30"
echo ""
echo -e "  2. Microcrystalline Cellulose"
echo -e "     Lot: ${RAW_MATERIAL_2_LOT}"
echo -e "     Quantity: 45 kg"
echo -e "     Supplier: Excipients Plus"
echo -e "     COA: COA-MCC-2024-Q4-015"
echo -e "     Expiry: 2027-03-15"
echo ""
echo -e "  3. Magnesium Stearate"
echo -e "     Lot: ${RAW_MATERIAL_3_LOT}"
echo -e "     Quantity: 2 kg"
echo -e "     Supplier: PharmaGrade Materials"
echo -e "     COA: COA-MS-2024-Q4-008"
echo -e "     Expiry: 2026-12-31"
echo ""

echo -e "${MAGENTA}Quality Test Summary:${NC}"
echo -e "  In-Process Tests: 3/3 ${GREEN}PASS${NC}"
echo -e "    ✓ Weight Variation"
echo -e "    ✓ Hardness"
echo -e "    ✓ Dissolution"
echo ""
echo -e "  Final Release Tests: 3/3 ${GREEN}PASS${NC}"
echo -e "    ✓ Assay (101.2%)"
echo -e "    ✓ Microbial Limits"
echo -e "    ✓ Content Uniformity"
echo ""

echo -e "${MAGENTA}Production Details:${NC}"
echo -e "  Manufacturing Line: Line-A"
echo -e "  Equipment: TABLET-PRESS-01"
echo -e "  Shift: Day-Shift-1"
echo -e "  Operator: Operator-001"
echo -e "  Production Time: ~4 hours"
echo ""

echo -e "${MAGENTA}Distribution:${NC}"
echo -e "  Available for distribution: 98,500 tablets"
echo -e "  COA Number: COA-${BATCH_NUMBER}"
echo -e "  Storage Conditions: Store at 15-30°C"
echo -e "  Shelf Life: 2 years from manufacture"
echo ""

echo -e "${CYAN}═══════════════════════════════════════════════${NC}"
echo ""

sleep 2

#########################################################
# STEP 10: Demonstrate Traceability Query
#########################################################

echo -e "${BLUE}[STEP 10]${NC} Demonstrating traceability queries..."
echo ""

echo -e "${YELLOW}Example 1: Forward Traceability${NC}"
echo -e "  Query: Where did raw material lot ${RAW_MATERIAL_1_LOT} go?"
echo -e "  Answer: Used in batch ${BATCH_NUMBER}"
echo -e "         Quantity consumed: 25 kg"
echo -e "         Final product: 98,500 tablets of ${PRODUCT_NAME}"
echo ""

echo -e "${YELLOW}Example 2: Backward Traceability${NC}"
echo -e "  Query: What raw materials are in batch ${BATCH_NUMBER}?"
echo -e "  Answer:"
echo -e "    • ${RAW_MATERIAL_1_LOT} (25 kg)"
echo -e "    • ${RAW_MATERIAL_2_LOT} (45 kg)"
echo -e "    • ${RAW_MATERIAL_3_LOT} (2 kg)"
echo ""

echo -e "${YELLOW}Example 3: Recall Scenario${NC}"
echo -e "  Scenario: Raw material lot ${RAW_MATERIAL_1_LOT} failed post-release stability"
echo -e "  Action Required:"
echo -e "    1. Query all batches using lot ${RAW_MATERIAL_1_LOT}"
echo -e "    2. Identified batches: ${BATCH_NUMBER}"
echo -e "    3. Retrieve distribution records for batch ${BATCH_NUMBER}"
echo -e "    4. ${RED}Initiate recall for 98,500 tablets${NC}"
echo -e "    5. Notify customers and regulatory agencies"
echo ""

#########################################################
# Summary
#########################################################

echo -e "${GREEN}Demo Complete!${NC}"
echo ""
echo -e "This demo showed:"
echo -e "  • Batch production record creation"
echo -e "  • Raw material lot tracking"
echo -e "  • In-process quality control"
echo -e "  • Final release testing"
echo -e "  • Batch approval workflow"
echo -e "  • Complete genealogy and traceability"
echo -e "  • Forward and backward traceability queries"
echo -e "  • Recall scenario management"
echo ""
echo -e "Key Benefits:"
echo -e "  • Full compliance with GMP requirements"
echo -e "  • Complete traceability for recalls"
echo -e "  • Quality assurance at every step"
echo -e "  • Automated batch record keeping"
echo -e "  • Real-time quality data"
echo -e "  • Rapid response to quality issues"
echo ""
