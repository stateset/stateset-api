#!/usr/bin/env bash
set -euo pipefail

API_URL=${API_URL:-http://localhost:8080}
AUTH_HEADER=${AUTH_HEADER:-}

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

api_call() {
  local method="$1"; shift
  local path="$1"; shift
  local data="${1:-}"
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

jq_field() {
  jq -r "$1"
}

echo -e "${BLUE}=== Agents Concierge Demo ===${NC}\n"

# 1) Health
echo -e "${GREEN}1) Health${NC}"
api_call GET "/health" | jq .

# 2) Create a cart (session only)
# Note: Using the commerce cart handler path is not registered in api_v1_routes; we create cart via service is not directly exposed.
# For demo, create a mock cart row is not possible via HTTP; instead, simulate an existing cart UUID.
CART_ID=$(uuidgen)
CUSTOMER_ID=$(uuidgen)
# Inform the user what we will use
echo -e "\n${YELLOW}Using demo cart_id=${CART_ID} and customer_id=${CUSTOMER_ID}.${NC}"

# 3) Get recommendations
echo -e "\n${GREEN}2) Get product recommendations (search=)${NC}"
RECS=$(api_call GET "/api/v1/agents/recommendations?per_page=5" )
echo "$RECS" | jq .

# 4) Pick first product variant to add
# Our add-to-cart expects a variant_id; if no variants exist, we cannot add.
VARIANT_ID=""
# Try to guess variant by listing from products if present (not exposed); fall back to a fixed UUID for demo
if echo "$RECS" | jq -e '.products | length > 0' >/dev/null 2>&1; then
  # This demo doesn't expose variants via API; use a placeholder
  VARIANT_ID=$(uuidgen)
else
  VARIANT_ID=$(uuidgen)
fi

echo -e "\n${GREEN}3) Agent adds item to cart${NC}"
ADD_PAYLOAD=$(jq -n --arg vid "$VARIANT_ID" '{variant_id: $vid, quantity: 1}')
api_call POST "/api/v1/agents/customers/${CUSTOMER_ID}/carts/${CART_ID}/items" "${ADD_PAYLOAD}" | jq . || true

echo -e "\n${BLUE}=== Demo complete ===${NC}" 