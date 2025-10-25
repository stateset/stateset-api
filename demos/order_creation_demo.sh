#!/usr/bin/env bash

set -euo pipefail

# Demo script for creating an order through the API
# Requires a running stateset-api instance with authentication configured.
# Set AUTH_TOKEN (Bearer token) or AUTH_HEADER (full header) for authenticated access.

API_URL="${API_URL:-http://localhost:8080}"
AUTH_TOKEN="${AUTH_TOKEN:-}"
AUTH_HEADER="${AUTH_HEADER:-}"

if [ -n "$AUTH_TOKEN" ] && [ -z "$AUTH_HEADER" ]; then
  AUTH_HEADER="Authorization: Bearer $AUTH_TOKEN"
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required for this demo" >&2
  exit 1
fi

if ! command -v uuidgen >/dev/null 2>&1; then
  echo "uuidgen is required for this demo" >&2
  exit 1
fi

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

api_call() {
  local method=$1
  local path=$2
  local data=${3:-}

  if [ -z "$data" ]; then
    curl -s -X "$method" "$API_URL$path" \
      -H "Content-Type: application/json" \
      ${AUTH_HEADER:+-H "$AUTH_HEADER"}
  else
    curl -s -X "$method" "$API_URL$path" \
      -H "Content-Type: application/json" \
      ${AUTH_HEADER:+-H "$AUTH_HEADER"} \
      -d "$data"
  fi
}

print_step() {
  echo -e "${BLUE}=== $1 ===${NC}"
}

print_step "Health check"
HEALTH_RESPONSE=$(api_call GET "/health")
echo "$HEALTH_RESPONSE" | jq '.' || echo "$HEALTH_RESPONSE"
echo

CUSTOMER_ID="${ORDER_CUSTOMER_ID:-$(uuidgen)}"
ITEM_REF="${ORDER_ITEM_ID:-sku-demo-001}"
ITEM_QUANTITY="${ORDER_ITEM_QUANTITY:-1}"
ITEM_PRICE="${ORDER_ITEM_PRICE:-29.99}"

ORDER_PAYLOAD=$(
  jq -n \
    --arg customer_id "$CUSTOMER_ID" \
    --arg item_ref "$ITEM_REF" \
    --argjson quantity "$ITEM_QUANTITY" \
    --arg price "$ITEM_PRICE" \
    '{
      customer_id: $customer_id,
      items: [
        {
          product_id: $item_ref,
          quantity: ($quantity | tonumber),
          unit_price: ($price | tonumber)
        }
      ],
      notes: "Demo order created by order_creation_demo.sh"
    }'
)

echo -e "${YELLOW}Using customer_id=${CUSTOMER_ID}${NC}"
echo -e "${YELLOW}Using product reference=${ITEM_REF}${NC}"
echo

print_step "Creating order"
ORDER_RESPONSE=$(api_call POST "/api/v1/orders" "$ORDER_PAYLOAD")
echo "$ORDER_RESPONSE" | jq '.' || echo "$ORDER_RESPONSE"
echo

ORDER_ID=$(echo "$ORDER_RESPONSE" | jq -r '.data.id // .data.order_id // empty')
ORDER_NUMBER=$(echo "$ORDER_RESPONSE" | jq -r '.data.order_number // empty')

if [ -n "$ORDER_ID" ]; then
  echo -e "${GREEN}Order created with id=${ORDER_ID}${NC}"
  [ -n "$ORDER_NUMBER" ] && echo -e "${GREEN}Order number=${ORDER_NUMBER}${NC}"
else
  echo -e "${YELLOW}Order identifier not present. Check response for validation or auth errors.${NC}"
fi

echo
echo -e "${BLUE}Demo complete.${NC}"
