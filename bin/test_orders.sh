#!/usr/bin/env bash
set -euo pipefail

# Exhaustive order endpoint tests against a running server
# This script exercises: list, get, get-items, create, update, add-item,
# update-status, cancel, archive, delete. Write routes may return 403 unless
# your auth mock grants orders:write/orders:update/orders:delete.

BASE_URL="${BASE_URL:-http://localhost:8080}"

log() { echo "[orders-test] $*"; }

curl_i() { curl -sS -w "\n<HTTP_STATUS:%{http_code}>\n" -i "$@"; }

# Login to get JWT (mock login)
JWT=""
login() {
  local body='{"email":"tester@example.com","password":"secret"}'
  local json
  json=$(curl -sS -X POST "$BASE_URL/api/v1/auth/login" -H 'Content-Type: application/json' -d "$body")
  if command -v jq >/dev/null 2>&1; then
    JWT=$(echo "$json" | jq -r '.access_token')
  else
    JWT=$(python3 - "$json" <<'PY'
import sys, json
print(json.loads(sys.argv[1]).get('access_token',''))
PY
)
  fi
  if [ -z "$JWT" ] || [ "$JWT" = "null" ]; then
    echo "Failed to obtain JWT" >&2; exit 1
  fi
}

login

AUTHZ=( -H "Authorization: Bearer $JWT" )

echo
log "List orders (GET /api/v1/orders)"
curl_i -X GET "$BASE_URL/api/v1/orders?page=1&limit=2" "${AUTHZ[@]}" | sed -n '1,120p'

echo
log "Get order existing (GET /api/v1/orders/order_123)"
curl_i -X GET "$BASE_URL/api/v1/orders/order_123" "${AUTHZ[@]}" | sed -n '1,120p'

echo
log "Get order missing (GET /api/v1/orders/missing_999)"
curl_i -X GET "$BASE_URL/api/v1/orders/missing_999" "${AUTHZ[@]}" | sed -n '1,120p'

echo
log "Get order items (GET /api/v1/orders/order_123/items)"
curl_i -X GET "$BASE_URL/api/v1/orders/order_123/items" "${AUTHZ[@]}" | sed -n '1,160p'

# Create order (may 403 without orders:write)
CREATE_BODY='{
  "customer_id": "cust_001",
  "items": [
    {"product_id": "sku_123", "quantity": 2, "unit_price": "19.99"},
    {"product_id": "sku_456", "quantity": 1}
  ]
}'

echo
log "Create order (POST /api/v1/orders) [expect 201 if write perms; else 403]"
curl_i -X POST "$BASE_URL/api/v1/orders" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$CREATE_BODY" | sed -n '1,200p'

# Update order (may 403 without orders:write)
UPDATE_BODY='{ "notes": "Updated by test script" }'
echo
log "Update order (PUT /api/v1/orders/order_123) [expect 200 with perms; else 403]"
curl_i -X PUT "$BASE_URL/api/v1/orders/order_123" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$UPDATE_BODY" | sed -n '1,160p'

# Add order item (may 403 without orders:write)
ADD_ITEM_BODY='{ "product_id": "sku_extra", "quantity": 1, "unit_price": "9.99" }'
echo
log "Add order item (POST /api/v1/orders/order_123/items) [expect 201 with perms; else 403]"
curl_i -X POST "$BASE_URL/api/v1/orders/order_123/items" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$ADD_ITEM_BODY" | sed -n '1,160p'

# Update status (may 403 without orders:write)
STATUS_BODY='{ "status": "shipped", "reason": "Test update" }'
echo
log "Update order status (PUT /api/v1/orders/order_123/status) [expect 200 with perms; else 403]"
curl_i -X PUT "$BASE_URL/api/v1/orders/order_123/status" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$STATUS_BODY" | sed -n '1,160p'

# Cancel (may 403 without orders:write)
CANCEL_BODY='{ "reason": "Customer request" }'
echo
log "Cancel order (POST /api/v1/orders/order_123/cancel) [expect 200 with perms; else 403]"
curl_i -X POST "$BASE_URL/api/v1/orders/order_123/cancel" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$CANCEL_BODY" | sed -n '1,160p'

# Archive (may 403 without orders:write)
echo
log "Archive order (POST /api/v1/orders/order_123/archive) [expect 200 with perms; else 403]"
curl_i -X POST "$BASE_URL/api/v1/orders/order_123/archive" "${AUTHZ[@]}" | sed -n '1,160p'

# Delete (may 403 without orders:delete)
echo
log "Delete order (DELETE /api/v1/orders/order_123) [expect 200 with perms; else 403]"
curl_i -X DELETE "$BASE_URL/api/v1/orders/order_123" "${AUTHZ[@]}" | sed -n '1,160p'

echo
log "Order command tests complete. Note expected 403 on writes without proper permissions."

