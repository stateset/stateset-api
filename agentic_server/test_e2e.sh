#!/bin/bash
set -e

echo "╔═══════════════════════════════════════════════════════════════╗"
echo "║   Agentic Commerce Server - End-to-End Test Suite            ║"
echo "║   Version: 0.3.0 (Release Candidate)                         ║"
echo "╚═══════════════════════════════════════════════════════════════╝"
echo

GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASSED=0
FAILED=0

test_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓ PASS${NC}"
        PASSED=$((PASSED + 1))
    else
        echo -e "${RED}✗ FAIL${NC}"
        FAILED=$((FAILED + 1))
    fi
}

# Start server in background
echo "🚀 Starting Agentic Commerce Server..."
./target/release/agentic-commerce-server > test_server.log 2>&1 &
SERVER_PID=$!
sleep 3

# Check if server started
if ! ps -p $SERVER_PID > /dev/null; then
    echo -e "${RED}✗ Server failed to start${NC}"
    cat test_server.log
    exit 1
fi

echo -e "${GREEN}✓ Server started (PID: $SERVER_PID)${NC}"
echo

# Test 1: Health Check
echo -n "Test 1: Health Check... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null http://localhost:8080/health)
[ "$STATUS" = "200" ]
test_result $?

# Test 2: Readiness Probe
echo -n "Test 2: Readiness Probe... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null http://localhost:8080/ready)
[ "$STATUS" = "200" ]
test_result $?

# Test 3: Metrics Endpoint
echo -n "Test 3: Prometheus Metrics... "
METRICS=$(curl -s http://localhost:8080/metrics)
echo "$METRICS" | grep -q "checkout_sessions_created_total"
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Authentication Tests${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 4: Valid API Key
echo -n "Test 4: Valid API Key Authentication... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"test","quantity":1}]}')
[ "$STATUS" = "201" ]
test_result $?

# Test 5: Invalid API Key
echo -n "Test 5: Invalid API Key Rejection... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer invalid_key" \
  -d '{"items":[{"id":"test","quantity":1}]}')
[ "$STATUS" = "401" ]
test_result $?

# Test 6: Missing API Key
echo -n "Test 6: Missing API Key Rejection... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -d '{"items":[{"id":"test","quantity":1}]}')
[ "$STATUS" = "401" ]
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Checkout Flow Tests${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 7: Create Checkout Session
echo -n "Test 7: Create Checkout Session... "
SESSION_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"laptop_pro_16_inch","quantity":1}],"fulfillment_address":{"name":"Test User","line_one":"123 Main St","city":"San Francisco","state":"CA","country":"US","postal_code":"94102"}}')

SESSION_ID=$(echo "$SESSION_RESPONSE" | jq -r '.id')
echo "$SESSION_RESPONSE" | jq -e '.id' > /dev/null
test_result $?

# Test 8: Retrieve Checkout Session
echo -n "Test 8: Retrieve Checkout Session... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null http://localhost:8080/checkout_sessions/$SESSION_ID)
[ "$STATUS" = "200" ]
test_result $?

# Test 9: Update Checkout Session
echo -n "Test 9: Update Checkout Session... "
UPDATE_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$SESSION_ID \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"buyer":{"first_name":"John","last_name":"Doe","email":"john@example.com"},"fulfillment_option_id":"standard_shipping"}')
echo "$UPDATE_RESPONSE" | jq -e '.status == "ready_for_payment"' > /dev/null
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Payment Tests${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 10: Delegated Payment (Vault Token)
echo -n "Test 10: Create Vault Token... "
VAULT_RESPONSE=$(curl -s -X POST http://localhost:8080/agentic_commerce/delegate_payment \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer psp_api_key_456" \
  -d "{\"payment_method\":{\"type\":\"card\",\"card_number_type\":\"fpan\",\"number\":\"4242424242424242\",\"exp_month\":\"12\",\"exp_year\":\"2027\",\"display_card_funding_type\":\"credit\",\"display_last4\":\"4242\",\"metadata\":{}},\"allowance\":{\"reason\":\"one_time\",\"max_amount\":500000,\"currency\":\"usd\",\"checkout_session_id\":\"$SESSION_ID\",\"merchant_id\":\"merchant_001\",\"expires_at\":\"2025-12-31T23:59:59Z\"},\"risk_signals\":[{\"type\":\"velocity_check\",\"score\":5,\"action\":\"authorized\"}],\"metadata\":{}}")

VAULT_TOKEN=$(echo "$VAULT_RESPONSE" | jq -r '.id')
echo "$VAULT_RESPONSE" | jq -e '.id' | grep -q "vt_"
test_result $?

# Test 11: Complete Checkout with Vault Token
echo -n "Test 11: Complete Checkout... "
COMPLETE_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$SESSION_ID/complete \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d "{\"payment_data\":{\"token\":\"$VAULT_TOKEN\",\"provider\":\"stripe\"}}")

echo "$COMPLETE_RESPONSE" | jq -e '.status == "completed" and .order.id' > /dev/null
test_result $?

# Test 12: Single-Use Token Enforcement
echo -n "Test 12: Single-Use Token Enforcement... "
# Create new session
NEW_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"test","quantity":1}],"fulfillment_address":{"name":"T","line_one":"1 St","city":"SF","state":"CA","country":"US","postal_code":"94102"}}' | jq -r '.id')

curl -s -X POST http://localhost:8080/checkout_sessions/$NEW_SESSION \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"buyer":{"first_name":"T","last_name":"U","email":"t@e.com"},"fulfillment_option_id":"standard_shipping"}' > /dev/null

# Try to reuse same vault token (should fail)
REUSE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$NEW_SESSION/complete \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d "{\"payment_data\":{\"token\":\"$VAULT_TOKEN\",\"provider\":\"stripe\"}}")

echo "$REUSE" | jq -e '.type == "invalid_request"' > /dev/null
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Product & Inventory Tests${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 13: Product Pricing
echo -n "Test 13: Real Product Pricing (MacBook)... "
LAPTOP_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"laptop_pro_16_inch","quantity":1}]}')

LAPTOP_PRICE=$(echo "$LAPTOP_SESSION" | jq -r '.line_items[0].base_amount')
[ "$LAPTOP_PRICE" = "349900" ] # $3,499.00
test_result $?

# Test 14: Out of Stock Detection
echo -n "Test 14: Out of Stock Detection... "
OOS_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"laptop_pro_16_inch","quantity":1000}]}')

echo "$OOS_RESPONSE" | jq -e '.type' | grep -q "error"
test_result $?

# Test 15: Tax Calculation (CA vs TX)
echo -n "Test 15: State-Based Tax Calculation... "
CA_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"item_123","quantity":1}],"fulfillment_address":{"name":"T","line_one":"1 St","city":"SF","state":"CA","country":"US","postal_code":"94102"}}')

TX_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"item_123","quantity":1}],"fulfillment_address":{"name":"T","line_one":"1 St","city":"Austin","state":"TX","country":"US","postal_code":"78701"}}')

CA_TAX=$(echo "$CA_SESSION" | jq -r '.totals[] | select(.type=="tax") | .amount')
TX_TAX=$(echo "$TX_SESSION" | jq -r '.totals[] | select(.type=="tax") | .amount')

[ "$CA_TAX" -gt "$TX_TAX" ] # CA has higher tax rate
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Session Management Tests${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 16: Cancel Session
echo -n "Test 16: Cancel Checkout Session... "
CANCEL_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"test","quantity":1}]}' | jq -r '.id')

STATUS=$(curl -s -w "%{http_code}" -o /dev/null -X POST http://localhost:8080/checkout_sessions/$CANCEL_SESSION/cancel \
  -H "Authorization: Bearer api_key_demo_123")

[ "$STATUS" = "200" ]
test_result $?

# Test 17: Cannot Cancel Completed Session
echo -n "Test 17: Cannot Cancel Completed Session... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null -X POST http://localhost:8080/checkout_sessions/$SESSION_ID/cancel \
  -H "Authorization: Bearer api_key_demo_123")

[ "$STATUS" = "400" ] || [ "$STATUS" = "405" ]
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Payment Method Tests${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 18: Stripe SharedPaymentToken (spt_)
echo -n "Test 18: Stripe SharedPaymentToken Support... "
SPT_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"test","quantity":1}],"fulfillment_address":{"name":"T","line_one":"1 St","city":"SF","state":"CA","country":"US","postal_code":"94102"}}' | jq -r '.id')

curl -s -X POST http://localhost:8080/checkout_sessions/$SPT_SESSION \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"buyer":{"first_name":"T","last_name":"U","email":"t@e.com"},"fulfillment_option_id":"standard_shipping"}' > /dev/null

SPT_COMPLETE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$SPT_SESSION/complete \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"payment_data":{"token":"spt_mock_123","provider":"stripe"}}')

echo "$SPT_COMPLETE" | jq -e '.status == "completed"' > /dev/null
test_result $?

# Test 19: Regular Payment Method
echo -n "Test 19: Regular Payment Method Support... "
REG_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"test","quantity":1}],"fulfillment_address":{"name":"T","line_one":"1 St","city":"SF","state":"CA","country":"US","postal_code":"94102"}}' | jq -r '.id')

curl -s -X POST http://localhost:8080/checkout_sessions/$REG_SESSION \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"buyer":{"first_name":"T","last_name":"U","email":"t@e.com"},"fulfillment_option_id":"standard_shipping"}' > /dev/null

REG_COMPLETE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$REG_SESSION/complete \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"payment_data":{"token":"pm_card_123","provider":"stripe"}}')

echo "$REG_COMPLETE" | jq -e '.status == "completed"' > /dev/null
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Error Handling Tests${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 20: Invalid Product ID
echo -n "Test 20: Invalid Product ID... "
ERROR_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"nonexistent_product","quantity":1}]}')

echo "$ERROR_RESPONSE" | jq -e '.type' | grep -q "error"
test_result $?

# Test 21: Session Not Found
echo -n "Test 21: Session Not Found (404)... "
STATUS=$(curl -s -w "%{http_code}" -o /dev/null http://localhost:8080/checkout_sessions/00000000-0000-0000-0000-000000000000)
[ "$STATUS" = "404" ]
test_result $?

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${BLUE}Final Summary${NC}"
echo "═══════════════════════════════════════════════════════════════"
echo

# Stop server
kill $SERVER_PID 2>/dev/null
wait $SERVER_PID 2>/dev/null

TOTAL=$((PASSED + FAILED))
PERCENTAGE=$((PASSED * 100 / TOTAL))

echo "Tests Run:    $TOTAL"
echo "Tests Passed: $PASSED"
echo "Tests Failed: $FAILED"
echo "Success Rate: $PERCENTAGE%"
echo

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║  ✓ ALL TESTS PASSED - READY FOR RELEASE!                     ║${NC}"
    echo -e "${GREEN}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 0
else
    echo -e "${RED}╔═══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${RED}║  ✗ SOME TESTS FAILED - REVIEW BEFORE RELEASE                 ║${NC}"
    echo -e "${RED}╚═══════════════════════════════════════════════════════════════╝${NC}"
    exit 1
fi 