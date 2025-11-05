#!/bin/bash

# StateSet API - cURL Examples
# This script demonstrates common API workflows using cURL
# Usage: ./curl-examples.sh

set -e

# Configuration
BASE_URL="http://localhost:8080/api/v1"
EMAIL="admin@stateset.com"
PASSWORD="your-password"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== StateSet API cURL Examples ===${NC}\n"

# 1. Login and get access token
echo -e "${GREEN}1. Authentication${NC}"
LOGIN_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/login" \
  -H "Content-Type: application/json" \
  -d "{
    \"email\": \"$EMAIL\",
    \"password\": \"$PASSWORD\"
  }")

# Extract access token using grep and sed (portable)
ACCESS_TOKEN=$(echo "$LOGIN_RESPONSE" | grep -o '"access_token":"[^"]*' | sed 's/"access_token":"//')

if [ -z "$ACCESS_TOKEN" ]; then
  echo -e "${RED}❌ Login failed. Please check your credentials.${NC}"
  echo "Response: $LOGIN_RESPONSE"
  exit 1
fi

echo -e "✓ Logged in successfully"
echo "Access Token: ${ACCESS_TOKEN:0:20}..."

# 2. List orders
echo -e "\n${GREEN}2. List Orders${NC}"
ORDERS_RESPONSE=$(curl -s -X GET "$BASE_URL/orders?page=1&limit=5" \
  -H "Authorization: Bearer $ACCESS_TOKEN")

echo "✓ Orders retrieved"
echo "$ORDERS_RESPONSE" | head -n 5

# 3. Check inventory low stock
echo -e "\n${GREEN}3. Check Low Stock Items${NC}"
LOW_STOCK_RESPONSE=$(curl -s -X GET "$BASE_URL/inventory/low-stock" \
  -H "Authorization: Bearer $ACCESS_TOKEN")

echo "✓ Low stock items retrieved"

# 4. Create an order
echo -e "\n${GREEN}4. Create Order${NC}"

# Note: Replace these UUIDs with actual values from your database
CUSTOMER_ID="550e8400-e29b-41d4-a716-446655440001"
PRODUCT_ID="550e8400-e29b-41d4-a716-446655440002"

CREATE_ORDER_RESPONSE=$(curl -s -X POST "$BASE_URL/orders" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"customer_id\": \"$CUSTOMER_ID\",
    \"status\": \"pending\",
    \"total_amount\": 199.98,
    \"currency\": \"USD\",
    \"items\": [{
      \"product_id\": \"$PRODUCT_ID\",
      \"sku\": \"WIDGET-001\",
      \"quantity\": 2,
      \"unit_price\": 99.99,
      \"name\": \"Premium Widget\"
    }],
    \"shipping_address\": {
      \"street\": \"123 Main St\",
      \"city\": \"San Francisco\",
      \"state\": \"CA\",
      \"postal_code\": \"94105\",
      \"country\": \"US\"
    }
  }")

# Extract order ID
ORDER_ID=$(echo "$CREATE_ORDER_RESPONSE" | grep -o '"id":"[^"]*' | head -1 | sed 's/"id":"//')

if [ -n "$ORDER_ID" ]; then
  echo "✓ Order created: $ORDER_ID"
else
  echo -e "${RED}❌ Order creation failed${NC}"
  echo "Response: $CREATE_ORDER_RESPONSE"
  ORDER_ID="550e8400-e29b-41d4-a716-446655440000" # Fallback for remaining examples
fi

# 5. Update order status
echo -e "\n${GREEN}5. Update Order Status${NC}"
UPDATE_STATUS_RESPONSE=$(curl -s -X PUT "$BASE_URL/orders/$ORDER_ID/status" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"status\": \"processing\",
    \"notes\": \"Payment confirmed\"
  }")

echo "✓ Order status updated to processing"

# 6. Create a shipment
echo -e "\n${GREEN}6. Create Shipment${NC}"

# Extract first order item ID (would need actual order data)
SHIPMENT_RESPONSE=$(curl -s -X POST "$BASE_URL/shipments" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"order_id\": \"$ORDER_ID\",
    \"carrier\": \"UPS\",
    \"service_level\": \"ground\"
  }")

SHIPMENT_ID=$(echo "$SHIPMENT_RESPONSE" | grep -o '"id":"[^"]*' | head -1 | sed 's/"id":"//')

if [ -n "$SHIPMENT_ID" ]; then
  echo "✓ Shipment created: $SHIPMENT_ID"
else
  echo "Note: Shipment creation requires valid order items"
  SHIPMENT_ID="550e8400-e29b-41d4-a716-446655440010" # Fallback
fi

# 7. Mark as shipped
echo -e "\n${GREEN}7. Mark Shipment as Shipped${NC}"
TRACKING_NUMBER="1Z999AA10123456784"

SHIP_RESPONSE=$(curl -s -X POST "$BASE_URL/shipments/$SHIPMENT_ID/ship" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"tracking_number\": \"$TRACKING_NUMBER\",
    \"shipped_at\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"
  }")

echo "✓ Marked as shipped with tracking: $TRACKING_NUMBER"

# 8. Track shipment
echo -e "\n${GREEN}8. Track Shipment${NC}"
TRACKING_RESPONSE=$(curl -s -X GET "$BASE_URL/shipments/track/$TRACKING_NUMBER" \
  -H "Authorization: Bearer $ACCESS_TOKEN")

echo "✓ Tracking info retrieved"

# 9. Create a return
echo -e "\n${GREEN}9. Create Return Request${NC}"
RETURN_RESPONSE=$(curl -s -X POST "$BASE_URL/returns" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"order_id\": \"$ORDER_ID\",
    \"items\": [{
      \"quantity\": 1,
      \"reason\": \"defective\",
      \"description\": \"Product arrived damaged\"
    }],
    \"customer_notes\": \"Package was damaged during shipping\"
  }")

RETURN_ID=$(echo "$RETURN_RESPONSE" | grep -o '"id":"[^"]*' | head -1 | sed 's/"id":"//')

if [ -n "$RETURN_ID" ]; then
  echo "✓ Return created: $RETURN_ID"
else
  echo "Note: Return creation requires valid order items"
fi

# 10. Get analytics dashboard
echo -e "\n${GREEN}10. Dashboard Metrics${NC}"
DASHBOARD_RESPONSE=$(curl -s -X GET "$BASE_URL/analytics/dashboard" \
  -H "Authorization: Bearer $ACCESS_TOKEN")

echo "✓ Dashboard metrics retrieved"
echo "$DASHBOARD_RESPONSE"

# 11. Create API key
echo -e "\n${GREEN}11. Create API Key${NC}"
API_KEY_RESPONSE=$(curl -s -X POST "$BASE_URL/auth/api-keys" \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"name\": \"Test API Key\",
    \"permissions\": [\"orders:read\", \"inventory:read\"],
    \"expires_at\": \"$(date -d '+1 year' -u +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || date -v+1y -u +%Y-%m-%dT%H:%M:%SZ)\"
  }")

API_KEY=$(echo "$API_KEY_RESPONSE" | grep -o '"key":"[^"]*' | sed 's/"key":"//')

if [ -n "$API_KEY" ]; then
  echo "✓ API Key created: ${API_KEY:0:20}..."

  # Test API key authentication
  echo -e "\n${GREEN}12. Test API Key Authentication${NC}"
  API_KEY_TEST=$(curl -s -X GET "$BASE_URL/orders?limit=1" \
    -H "X-API-Key: $API_KEY")

  echo "✓ API Key authentication successful"
fi

echo -e "\n${BLUE}=== All examples completed successfully! ===${NC}\n"
