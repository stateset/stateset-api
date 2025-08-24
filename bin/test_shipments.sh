#!/usr/bin/env bash
set -euo pipefail

# End-to-end tests for shipments endpoints
BASE_URL="${BASE_URL:-http://localhost:8080}"

log() { echo "[shipments-test] $*"; }
curl_i() { curl -sS -w "\n<HTTP_STATUS:%{http_code}>\n" -i "$@"; }

# Login for JWT
JWT=""
json=$(curl -sS -X POST "$BASE_URL/api/v1/auth/login" -H 'Content-Type: application/json' -d '{"email":"tester@example.com","password":"secret"}')
if command -v jq >/dev/null 2>&1; then
  JWT=$(echo "$json" | jq -r '.access_token')
else
  JWT=$(python3 - "$json" <<'PY'
import sys, json
print(json.loads(sys.argv[1]).get('access_token',''))
PY
)
fi
[ -n "$JWT" ] && [ "$JWT" != null ] || { echo "login failed" >&2; exit 1; }
AUTHZ=( -H "Authorization: Bearer $JWT" )

echo
log "List shipments"
curl_i -X GET "$BASE_URL/api/v1/shipments?limit=2" "${AUTHZ[@]}" | sed -n '1,160p'

echo
log "Get shipment"
curl_i -X GET "$BASE_URL/api/v1/shipments/ship_001" "${AUTHZ[@]}" | sed -n '1,160p'

CREATE_BODY='{
  "order_id": "order_001",
  "carrier": "UPS",
  "service_type": "Ground",
  "shipping_address": {"street1":"123 Main St","street2":null,"city":"Anytown","state":"CA","postal_code":"90210","country":"US"},
  "items": [{"order_item_id":"order_item_001","quantity":1}],
  "weight": 2.5,
  "dimensions": {"length":12.0,"width":8.0,"height":4.0,"unit":"in"}
}'
echo
log "Create shipment"
curl_i -X POST "$BASE_URL/api/v1/shipments" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$CREATE_BODY" | sed -n '1,200p'

UPDATE_BODY='{ "status":"in_transit", "tracking_number":"1Z123456789" }'
echo
log "Update shipment"
curl_i -X PUT "$BASE_URL/api/v1/shipments/ship_001" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$UPDATE_BODY" | sed -n '1,200p'

STATUS_BODY='{ "status":"label_created" }'
echo
log "Update shipment status"
curl_i -X PUT "$BASE_URL/api/v1/shipments/ship_001/status" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$STATUS_BODY" | sed -n '1,160p'

echo
log "Mark shipped"
curl_i -X POST "$BASE_URL/api/v1/shipments/ship_001/ship" "${AUTHZ[@]}" | sed -n '1,160p'

echo
log "Mark delivered"
curl_i -X POST "$BASE_URL/api/v1/shipments/ship_001/deliver" "${AUTHZ[@]}" | sed -n '1,160p'

TRACK_BODY='{ "status":"in_transit","description":"Departed facility","location":"Distribution Center" }'
echo
log "Add tracking event"
curl_i -X POST "$BASE_URL/api/v1/shipments/ship_001/tracking" -H 'Content-Type: application/json' "${AUTHZ[@]}" -d "$TRACK_BODY" | sed -n '1,200p'

echo
log "Track by ID"
curl_i -X GET "$BASE_URL/api/v1/shipments/ship_001/track" "${AUTHZ[@]}" | sed -n '1,200p'

echo
log "Track by number"
curl_i -X GET "$BASE_URL/api/v1/shipments/track/1Z123456789" "${AUTHZ[@]}" | sed -n '1,200p'

echo
log "Delete shipment"
curl_i -X DELETE "$BASE_URL/api/v1/shipments/ship_001" "${AUTHZ[@]}" | sed -n '1,200p'

echo
log "Shipments tests complete."

