#!/bin/bash
set -e

echo "═══════════════════════════════════════════════════════════════"
echo "   Agentic Commerce + Delegated Payment - Full Demo Test"
echo "═══════════════════════════════════════════════════════════════"
echo

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}1️⃣  Creating Checkout Session${NC}"
echo "   Creating session with 1 laptop..."
echo

SESSION_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -H "API-Version: 2025-09-29" \
  -H "Idempotency-Key: demo_create_$(date +%s)" \
  -d '{
    "items": [
      {
        "id": "laptop_pro_16_inch",
        "quantity": 1
      }
    ],
    "customer": {
      "shipping_address": {
        "name": "Alice Smith",
        "line1": "456 Tech Avenue",
        "city": "San Francisco",
        "region": "CA",
        "country": "US",
        "postal_code": "94105"
      }
    }
  }')

echo "$SESSION_RESPONSE" | jq .
SESSION_ID=$(echo "$SESSION_RESPONSE" | jq -r '.id')

echo
echo -e "${GREEN}✓ Session created: $SESSION_ID${NC}"
echo -e "   Status: $(echo "$SESSION_RESPONSE" | jq -r '.status')"
echo -e "   Total: \$$(echo "$SESSION_RESPONSE" | jq -r '.totals.grand_total.amount')  (cents)"
echo

echo "═══════════════════════════════════════════════════════════════"
echo

echo -e "${BLUE}2️⃣  Delegating Payment to PSP${NC}"
echo "   Sending card details to get vault token..."
echo

VAULT_RESPONSE=$(curl -s -X POST http://localhost:8080/agentic_commerce/delegate_payment \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer psp_api_key_456" \
  -H "Idempotency-Key: demo_delegate_$(date +%s)" \
  -d "{
    \"payment_method\": {
      \"type\": \"card\",
      \"card_number_type\": \"fpan\",
      \"number\": \"4242424242424242\",
      \"exp_month\": \"12\",
      \"exp_year\": \"2027\",
      \"name\": \"Alice Smith\",
      \"cvc\": \"123\",
      \"display_card_funding_type\": \"credit\",
      \"display_brand\": \"Visa\",
      \"display_last4\": \"4242\",
      \"metadata\": {}
    },
    \"allowance\": {
      \"reason\": \"one_time\",
      \"max_amount\": 100000,
      \"currency\": \"usd\",
      \"checkout_session_id\": \"$SESSION_ID\",
      \"merchant_id\": \"demo_merchant_001\",
      \"expires_at\": \"2025-12-31T23:59:59Z\"
    },
    \"billing_address\": {
      \"name\": \"Alice Smith\",
      \"line_one\": \"456 Tech Avenue\",
      \"city\": \"San Francisco\",
      \"state\": \"CA\",
      \"country\": \"US\",
      \"postal_code\": \"94105\"
    },
    \"risk_signals\": [
      {
        \"type\": \"velocity_check\",
        \"score\": 3,
        \"action\": \"authorized\"
      }
    ],
    \"metadata\": {
      \"source\": \"chatgpt_demo\",
      \"customer_id\": \"cust_demo_12345\"
    }
  }")

echo "$VAULT_RESPONSE" | jq .
VAULT_TOKEN=$(echo "$VAULT_RESPONSE" | jq -r '.id')

echo
echo -e "${GREEN}✓ Vault token created: $VAULT_TOKEN${NC}"
echo -e "   Provider: Stripe (mock)"
echo -e "   Single-use: Yes"
echo

echo "═══════════════════════════════════════════════════════════════"
echo

echo -e "${BLUE}3️⃣  Adding Buyer Info & Selecting Shipping${NC}"
echo "   Updating session with buyer and fulfillment option..."
echo

UPDATE_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$SESSION_ID \
  -H "Content-Type: application/json" \
  -d '{
    "customer": {
      "shipping_address": {
        "name": "Alice Smith",
        "line1": "456 Tech Avenue",
        "city": "San Francisco",
        "region": "CA",
        "country": "US",
        "postal_code": "94105",
        "email": "alice.smith@example.com",
        "phone": "+14155559876"
      }
    },
    "fulfillment": {
      "selected_id": "standard_shipping"
    }
  }')

echo "$UPDATE_RESPONSE" | jq '{id, status, customer: .customer.shipping_address.email, fulfillment: .fulfillment.selected_id, totals: .totals}'

echo
echo -e "${GREEN}✓ Session updated${NC}"
echo -e "   Status: $(echo "$UPDATE_RESPONSE" | jq -r '.status')"
echo -e "   Buyer: $(echo "$UPDATE_RESPONSE" | jq -r '.customer.shipping_address.email')"
echo

echo "═══════════════════════════════════════════════════════════════"
echo

echo -e "${BLUE}4️⃣  Completing Checkout with Vault Token${NC}"
echo "   Finalizing purchase with delegated payment..."
echo

COMPLETE_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$SESSION_ID/complete \
  -H "Content-Type: application/json" \
  -d "{
    \"payment\": {
      \"delegated_token\": \"$VAULT_TOKEN\"
    }
  }")

echo "$COMPLETE_RESPONSE" | jq '{status, order: .order, customer: .customer.shipping_address.email}'

echo
ORDER_ID=$(echo "$COMPLETE_RESPONSE" | jq -r '.order.id')
ORDER_URL=$(echo "$COMPLETE_RESPONSE" | jq -r '.order.permalink_url')
echo -e "${GREEN}✓ Order created: $ORDER_ID${NC}"
echo -e "   Order URL: $ORDER_URL"
echo -e "   Payment processed with vault token: $VAULT_TOKEN"
echo

echo "═══════════════════════════════════════════════════════════════"
echo

echo -e "${YELLOW}5️⃣  Testing Single-Use Token Enforcement${NC}"
echo "   Attempting to reuse vault token (should fail)..."
echo

# Create new session
NEW_SESSION=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -d '{"items":[{"id":"item_test","quantity":1}],"customer":{"shipping_address":{"name":"Test User","line1":"789 Test St","city":"San Francisco","region":"CA","country":"US","postal_code":"94102"}}}')
NEW_SESSION_ID=$(echo "$NEW_SESSION" | jq -r '.id')

# Update with customer + fulfillment
curl -s -X POST http://localhost:8080/checkout_sessions/$NEW_SESSION_ID \
  -H "Content-Type: application/json" \
  -d '{"customer":{"shipping_address":{"name":"Test User","line1":"789 Test St","city":"San Francisco","region":"CA","country":"US","postal_code":"94102","email":"test@example.com"}},"fulfillment":{"selected_id":"standard_shipping"}}' > /dev/null

# Try to use same vault token
REUSE_RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions/$NEW_SESSION_ID/complete \
  -H "Content-Type: application/json" \
  -d "{\"payment\":{\"delegated_token\":\"$VAULT_TOKEN\"}}")

echo "$REUSE_RESPONSE" | jq .

if echo "$REUSE_RESPONSE" | jq -e '.order' > /dev/null 2>&1; then
    echo -e "${YELLOW}⚠  Token was reused (should have failed!)${NC}"
else
    echo -e "${GREEN}✓ Token reuse correctly prevented${NC}"
fi

echo
echo "═══════════════════════════════════════════════════════════════"
echo -e "${GREEN}🎉 Demo Complete!${NC}"
echo "═══════════════════════════════════════════════════════════════" 
