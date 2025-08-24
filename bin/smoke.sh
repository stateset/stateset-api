#!/usr/bin/env bash
set -euo pipefail

# Simple smoke tests for the Stateset API
# - Verifies health/readiness, metrics, auth login, and key v1 endpoints
# - Attempts a write to demonstrate permission gate behavior

BASE_URL="${BASE_URL:-http://localhost:8080}"
API_KEY_HEADER="${API_KEY_HEADER:-sk_test_example123}"

print_hdr() {
  echo
  echo "=== $1 ==="
}

req() {
  local method="$1"; shift
  local url="$1"; shift
  local extra=("$@")
  curl -sS -w "\n<HTTP_STATUS:%{http_code}>\n" -i -X "$method" "$url" "${extra[@]}"
}

# 1) Health + readiness
print_hdr "Health"
req GET "$BASE_URL/health" | sed -n '1,80p'

print_hdr "Readiness"
req GET "$BASE_URL/health/ready" | sed -n '1,80p'

# 2) Metrics
print_hdr "Metrics"
req GET "$BASE_URL/metrics" | sed -n '1,40p'

# 3) Status
print_hdr "API v1 Status"
req GET "$BASE_URL/api/v1/status" | sed -n '1,120p'

# 4) Auth: login to get tokens (mock login)
print_hdr "Login"
LOGIN_BODY='{"email":"tester@example.com","password":"secret"}'
LOGIN_JSON=$(curl -sS -X POST "$BASE_URL/api/v1/auth/login" -H 'Content-Type: application/json' -d "$LOGIN_BODY")
echo "$LOGIN_JSON" | sed -n '1,10p' >/dev/null 2>&1 || true
echo "$LOGIN_JSON" | head -c 200 | sed -n '1,10p'

# Extract access_token using python (fallback if jq is not available)
if command -v jq >/dev/null 2>&1; then
  ACCESS_TOKEN=$(echo "$LOGIN_JSON" | jq -r '.access_token')
else
  ACCESS_TOKEN=$(python3 - "$LOGIN_JSON" <<'PY'
import sys, json
data=json.loads(sys.argv[1])
print(data.get('access_token',''))
PY
)
fi

if [ -z "${ACCESS_TOKEN:-}" ] || [ "$ACCESS_TOKEN" = "null" ]; then
  echo "Failed to extract access_token from login response" >&2
  exit 1
fi

AUTHZ=( -H "Authorization: Bearer $ACCESS_TOKEN" )

# 5) List orders (authorized via API key)
print_hdr "List Orders (API Key)"
req GET "$BASE_URL/api/v1/orders?page=1&limit=2" -H "X-API-Key: $API_KEY_HEADER" | sed -n '1,200p'

# 6) List orders (authorized via JWT)
print_hdr "List Orders (JWT)"
req GET "$BASE_URL/api/v1/orders?page=1&limit=2" "${AUTHZ[@]}" | sed -n '1,200p'

# 7) Attempt create order with JWT (expected 403 unless server grants orders:write)
print_hdr "Create Order (JWT; may 403 without orders:write)"
CREATE_BODY='{
  "customer_id": "cust_001",
  "items": [
    {"product_id": "sku_123", "quantity": 2, "unit_price": "19.99"},
    {"product_id": "sku_456", "quantity": 1}
  ]
}'
req POST "$BASE_URL/api/v1/orders" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$CREATE_BODY" | sed -n '1,200p'

echo
echo "Smoke tests complete. Note: write endpoints require 'orders:write' (or admin)."

