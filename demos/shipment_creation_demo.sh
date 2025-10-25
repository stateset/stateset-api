#!/usr/bin/env bash

set -euo pipefail

# Demo script for creating a shipment through the API.
# Requires an existing order id and valid authentication.
# Provide SHIPMENT_ORDER_ID (or ORDER_ID) plus AUTH_TOKEN or AUTH_HEADER.

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

ORDER_ID="${SHIPMENT_ORDER_ID:-${ORDER_ID:-}}"

if [ -z "$ORDER_ID" ]; then
  ORDER_ID=$(uuidgen)
  echo -e "${YELLOW}No order id provided; using generated id=${ORDER_ID}.${NC}"
  echo -e "${YELLOW}The API will return an error unless this order exists.${NC}"
fi

TRACKING_NUMBER="${SHIPMENT_TRACKING_NUMBER:-SHIP-$(date +%s)}"
SHIPPING_METHOD="${SHIPMENT_METHOD:-standard}"
RECIPIENT_NAME="${SHIPMENT_RECIPIENT:-Demo Customer}"
SHIPPING_ADDRESS="${SHIPMENT_ADDRESS:-123 Demo Street, Springfield, NY 10001}"

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

print_step "Creating shipment"
SHIPMENT_RESPONSE=$(api_call POST "/api/v1/shipments" "$SHIPMENT_PAYLOAD")
echo "$SHIPMENT_RESPONSE" | jq '.' || echo "$SHIPMENT_RESPONSE"
echo

SHIPMENT_ID=$(echo "$SHIPMENT_RESPONSE" | jq -r '.data.id // empty')

if [ -n "$SHIPMENT_ID" ]; then
  echo -e "${GREEN}Shipment created with id=${SHIPMENT_ID}${NC}"
else
  echo -e "${YELLOW}Shipment identifier not present. Check response for validation or auth errors.${NC}"
fi

echo
echo -e "${BLUE}Demo complete.${NC}"
