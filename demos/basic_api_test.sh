#!/bin/bash

# Basic API test for currently available endpoints
# Tests the endpoints that are actually implemented in the main.rs file

API_URL="${API_URL:-http://localhost:8080}"
AUTH_TOKEN="${AUTH_TOKEN:-test-token}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== StateSet API Basic Functionality Test ===${NC}\n"

# Helper function for API calls
api_call() {
    local method=$1
    local endpoint=$2
    local data=$3
    
    if [ -z "$data" ]; then
        curl -s -X "$method" "$API_URL$endpoint" \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json"
    else
        curl -s -X "$method" "$API_URL$endpoint" \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            -d "$data"
    fi
}

# Test 1: Health Check
echo -e "${GREEN}Test 1: Health Check${NC}"
api_call GET "/health" | jq .
echo -e "\n"

# Test 2: API Status
echo -e "${GREEN}Test 2: API Status${NC}"
api_call GET "/api/v1/status" | jq .
echo -e "\n"

# Test 3: List Orders (Empty)
echo -e "${GREEN}Test 3: List Orders${NC}"
api_call GET "/api/v1/orders" | jq .
echo -e "\n"

# Test 4: Create Order (Test POST endpoint)
echo -e "${GREEN}Test 4: Create Order${NC}"
ORDER_DATA='{
  "customer_id": "12345",
  "items": [
    {
      "product_id": "prod_001",
      "quantity": 2,
      "price": 29.99
    }
  ]
}'
api_call POST "/api/v1/orders" "$ORDER_DATA" | jq .
echo -e "\n"

# Test 5: List Inventory
echo -e "${GREEN}Test 5: List Inventory${NC}"
api_call GET "/api/v1/inventory" | jq .
echo -e "\n"

# Test 6: List Shipments
echo -e "${GREEN}Test 6: List Shipments${NC}"
api_call GET "/api/v1/shipments" | jq .
echo -e "\n"

# Test 7: List Returns
echo -e "${GREEN}Test 7: List Returns${NC}"
api_call GET "/api/v1/returns" | jq .
echo -e "\n"

# Test 8: List Warranties
echo -e "${GREEN}Test 8: List Warranties${NC}"
api_call GET "/api/v1/warranties" | jq .
echo -e "\n"

# Summary
echo -e "${BLUE}=== Test Summary ===${NC}"
echo -e "${YELLOW}Note: The current API implementation has basic endpoints that return empty data.${NC}"
echo -e "${YELLOW}The full commerce functionality shown in other demos is not yet implemented.${NC}"
echo -e "\n${GREEN}Available endpoints:${NC}"
echo "  - GET  /health           - Health check"
echo "  - GET  /api/v1/status    - API status"
echo "  - GET  /api/v1/orders    - List orders (returns empty list)"
echo "  - POST /api/v1/orders    - Create order (returns mock response)"
echo "  - GET  /api/v1/inventory - List inventory (returns empty list)"
echo "  - GET  /api/v1/shipments - List shipments (returns empty list)"
echo "  - GET  /api/v1/returns   - List returns (returns empty list)"
echo "  - GET  /api/v1/warranties- List warranties (returns empty list)" 