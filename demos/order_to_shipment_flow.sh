#!/usr/bin/env bash

set -euo pipefail

# Demo script that creates an order and immediately creates a shipment for it.
# Requires stateset-api running plus valid authentication.
# Accepts the same knobs as order_creation_demo.sh and shipment_creation_demo.sh.

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

CUSTOMER_ID="${FLOW_CUSTOMER_ID:-${ORDER_CUSTOMER_ID:-$(uuidgen)}}"
ITEM_REF="${FLOW_ITEM_ID:-${ORDER_ITEM_ID:-sku-demo-001}}"
ITEM_QUANTITY="${FLOW_ITEM_QUANTITY:-${ORDER_ITEM_QUANTITY:-1}}"
ITEM_PRICE="${FLOW_ITEM_PRICE:-${ORDER_ITEM_PRICE:-29.99}}"

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
      notes: "Demo flow order created by order_to_shipment_flow.sh"
    }'
)

echo -e "${YELLOW}Creating order for customer ${CUSTOMER_ID} using product reference ${ITEM_REF}.${NC}"
print_step "Create order"
ORDER_RESPONSE=$(api_call POST "/api/v1/orders" "$ORDER_PAYLOAD")
echo "$ORDER_RESPONSE" | jq '.' || echo "$ORDER_RESPONSE"
echo

ORDER_ID=$(echo "$ORDER_RESPONSE" | jq -r '.data.id // empty')
if [ -z "$ORDER_ID" ]; then
  echo -e "${YELLOW}Unable to extract order id; aborting shipment creation.${NC}"
  exit 1
fi

TRACKING_NUMBER="${FLOW_TRACKING_NUMBER:-SHIP-$(date +%s)}"
SHIPPING_METHOD="${FLOW_SHIPPING_METHOD:-${SHIPMENT_METHOD:-standard}}"
RECIPIENT_NAME="${FLOW_RECIPIENT_NAME:-${SHIPMENT_RECIPIENT:-Demo Customer}}"
SHIPPING_ADDRESS="${FLOW_SHIPPING_ADDRESS:-${SHIPMENT_ADDRESS:-123 Demo Street, Springfield, NY 10001}}"

SHIPMENT_PAYLOAD=$(
  jq -n \
    --arg order_id "$ORDER_ID" \
    --arg tracking "$TRACKING_NUMBER" \
    --arg method "$SHIPPING_METHOD" \
    --arg address "$SHIPPING_ADDRESS" \
    --arg recipient "$RECIPIENT_NAME" \
    '{
      order_id: $order_id,
      tracking_number: $tracking,
      shipping_method: $method,
      shipping_address: $address,
      recipient_name: $recipient
    }'
)

print_step "Create shipment"
SHIPMENT_RESPONSE=$(api_call POST "/api/v1/shipments" "$SHIPMENT_PAYLOAD")
echo "$SHIPMENT_RESPONSE" | jq '.' || echo "$SHIPMENT_RESPONSE"
echo

SHIPMENT_ID=$(echo "$SHIPMENT_RESPONSE" | jq -r '.data.id // empty')

echo -e "${GREEN}Order id: ${ORDER_ID}${NC}"
if [ -n "$SHIPMENT_ID" ]; then
  echo -e "${GREEN}Shipment id: ${SHIPMENT_ID}${NC}"
else
  echo -e "${YELLOW}Shipment id not present; review response for errors.${NC}"
fi

echo
echo -e "${BLUE}Flow complete.${NC}"
