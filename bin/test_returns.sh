#!/usr/bin/env bash
set -euo pipefail

# Exhaustive returns endpoint tests against a running server
# Exercises: list, get, create, update, update-status, process, approve,
# reject, restock, refund, delete. Write routes are permission-gated;
# run server with make run-admin for local success.

BASE_URL="${BASE_URL:-http://localhost:8080}"

log() { echo "[returns-test] $*"; }
curl_i() { curl -sS -w "\n<HTTP_STATUS:%{http_code}>\n" -i "$@"; }

# Login (mock) to get JWT
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
log "List returns (GET /api/v1/returns)"
curl_i -X GET "$BASE_URL/api/v1/returns?limit=2" "${AUTHZ[@]}" | sed -n '1,160p'

echo
log "Get return (GET /api/v1/returns/ret_001)"
curl_i -X GET "$BASE_URL/api/v1/returns/ret_001" "${AUTHZ[@]}" | sed -n '1,160p'

CREATE_BODY='{
  "order_id": "order_001",
  "reason": "defective",
  "description": "Screen cracked",
  "return_type": "refund",
  "items": [
    {"order_item_id": "order_item_001", "quantity": 1}
  ]
}'
echo
log "Create return (POST /api/v1/returns)"
curl_i -X POST "$BASE_URL/api/v1/returns" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$CREATE_BODY" | sed -n '1,200p'

UPDATE_BODY='{"status":"approved","inspection_notes":"Approved after inspection","reason":"defective"}'
echo
log "Update return (PUT /api/v1/returns/ret_001)"
curl_i -X PUT "$BASE_URL/api/v1/returns/ret_001" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$UPDATE_BODY" | sed -n '1,200p'

STATUS_BODY='{"status":"approved"}'
echo
log "Update return status (PUT /api/v1/returns/ret_001/status)"
curl_i -X PUT "$BASE_URL/api/v1/returns/ret_001/status" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$STATUS_BODY" | sed -n '1,200p'

PROCESS_BODY='{"action":"approve","inspection_notes":"Looks good","items":[]}'
echo
log "Process return (POST /api/v1/returns/ret_001/process)"
curl_i -X POST "$BASE_URL/api/v1/returns/ret_001/process" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$PROCESS_BODY" | sed -n '1,200p'

echo
log "Approve return (POST /api/v1/returns/ret_001/approve)"
curl_i -X POST "$BASE_URL/api/v1/returns/ret_001/approve" "${AUTHZ[@]}" | sed -n '1,200p'

echo
log "Reject return (POST /api/v1/returns/ret_001/reject)"
curl_i -X POST "$BASE_URL/api/v1/returns/ret_001/reject" "${AUTHZ[@]}" | sed -n '1,200p'

RESTOCK_BODY='{"return_id":"ret_001","location_id":"loc_A","items":[{"return_item_id":"ret_item_001","quantity":1,"condition":"like_new"}]}'
echo
log "Restock return (POST /api/v1/returns/ret_001/restock)"
curl_i -X POST "$BASE_URL/api/v1/returns/ret_001/restock" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$RESTOCK_BODY" | sed -n '1,200p'

echo
log "Issue refund (POST /api/v1/returns/ret_001/refund)"
curl_i -X POST "$BASE_URL/api/v1/returns/ret_001/refund" "${AUTHZ[@]}" | sed -n '1,200p'

echo
log "Delete return (DELETE /api/v1/returns/ret_001)"
curl_i -X DELETE "$BASE_URL/api/v1/returns/ret_001" "${AUTHZ[@]}" | sed -n '1,200p'

echo
log "Returns command tests complete."

