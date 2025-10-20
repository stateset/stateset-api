#!/usr/bin/env bash
set -euo pipefail

# Usage:
#   AUTH_TOKEN=<jwt> CUSTOMER_ID=<uuid> PRODUCT_ID=<uuid> ./scripts/create_order.sh
# Optional env vars:
#   API_BASE (default http://localhost:8080/api/v1)
#   ORDER_NUMBER (defaults to generated UUID v4)
#   CURRENCY (default USD)
#   UNIT_PRICE (default 19.99)
#   QUANTITY (default 1)
#   NOTES (default empty string)

API_BASE=${API_BASE:-http://localhost:8080/api/v1}
AUTH_TOKEN=${AUTH_TOKEN:-}
CUSTOMER_ID=${CUSTOMER_ID:-}
PRODUCT_ID=${PRODUCT_ID:-}

if [[ -z "${AUTH_TOKEN}" || -z "${CUSTOMER_ID}" || -z "${PRODUCT_ID}" ]]; then
  echo "AUTH_TOKEN, CUSTOMER_ID, and PRODUCT_ID environment variables are required." >&2
  exit 1
fi

ORDER_NUMBER=${ORDER_NUMBER:-$(uuidgen)}
CURRENCY=${CURRENCY:-USD}
UNIT_PRICE=${UNIT_PRICE:-19.99}
QUANTITY=${QUANTITY:-1}
NOTES=${NOTES:-""}

PAYLOAD=$(cat <<JSON
{
  "order_number": "${ORDER_NUMBER}",
  "customer_id": "${CUSTOMER_ID}",
  "currency": "${CURRENCY}",
  "items": [
    {
      "product_id": "${PRODUCT_ID}",
      "quantity": ${QUANTITY},
      "unit_price": "${UNIT_PRICE}"
    }
  ],
  "notes": ${NOTES:+\"$NOTES\"}
}
JSON
)

echo "Creating order ${ORDER_NUMBER} for customer ${CUSTOMER_ID}..."

curl --fail --silent --show-error \
  -X POST "${API_BASE}/orders" \
  -H "Authorization: Bearer ${AUTH_TOKEN}" \
  -H "Content-Type: application/json" \
  -d "${PAYLOAD}"

echo
echo "Order created."
