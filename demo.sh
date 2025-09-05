#!/bin/bash

# StateSet API Demo Script
# This script demonstrates the main API endpoints

BASE_URL="http://localhost:3000"
CUSTOMER_TOKEN=""

echo "🚀 StateSet API Demo"
echo "===================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to make API calls
api_call() {
    local method=$1
    local endpoint=$2
    local data=$3
    local auth_header=""
    
    if [ -n "$CUSTOMER_TOKEN" ]; then
        auth_header="-H \"Authorization: Bearer $CUSTOMER_TOKEN\""
    fi
    
    echo -e "\n${YELLOW}[$method] $endpoint${NC}"
    
    if [ -n "$data" ]; then
        echo "Request: $data"
    fi
    
    if [ "$method" = "GET" ]; then
        response=$(curl -s -X $method "$BASE_URL$endpoint" $auth_header)
    else
        response=$(curl -s -X $method "$BASE_URL$endpoint" \
            -H "Content-Type: application/json" \
            $auth_header \
            -d "$data")
    fi
    
    echo "Response: $response"
    echo "---"
}

# Health check
echo -e "\n${GREEN}1. Health Check${NC}"
api_call "GET" "/health" ""

# API status
echo -e "\n${GREEN}2. API Status${NC}"
api_call "GET" "/status" ""

# Register customer
echo -e "\n${GREEN}3. Customer Registration${NC}"
api_call "POST" "/customers/register" '{
    "email": "demo@example.com",
    "first_name": "Demo",
    "last_name": "User",
    "password": "demopassword123",
    "phone": "+1-555-0123",
    "accepts_marketing": true
}'

# Login customer
echo -e "\n${GREEN}4. Customer Login${NC}"
login_response=$(curl -s -X POST "$BASE_URL/customers/login" \
    -H "Content-Type: application/json" \
    -d '{
        "email": "demo@example.com",
        "password": "demopassword123"
    }')

echo "Login Response: $login_response"

# Extract token from login response (this is simplified - in reality you'd parse JSON)
CUSTOMER_TOKEN="demo_token_123"

# Get customer profile
echo -e "\n${GREEN}5. Get Customer Profile${NC}"
api_call "GET" "/customers/$(uuidgen)" ""

# List orders
echo -e "\n${GREEN}6. List Orders${NC}"
api_call "GET" "/orders" ""

# Create order (would require a real customer ID)
echo -e "\n${GREEN}7. Create Order Example${NC}"
echo "Note: This would require a real customer ID and product setup"
echo "Example payload:"
echo '{
    "customer_id": "customer-uuid-here",
    "items": [
        {
            "product_id": "product-uuid-here",
            "quantity": 2
        }
    ],
    "shipping_address": {
        "street": "123 Main St",
        "city": "Anytown",
        "state": "CA",
        "zip_code": "12345",
        "country": "US"
    }
}'

# List inventory
echo -e "\n${GREEN}8. List Inventory${NC}"
api_call "GET" "/inventory" ""

# List ASNs
echo -e "\n${GREEN}9. List ASNs${NC}"
api_call "GET" "/asns" ""

# List purchase orders
echo -e "\n${GREEN}10. List Purchase Orders${NC}"
api_call "GET" "/purchase-orders" ""

# List returns
echo -e "\n${GREEN}11. List Returns${NC}"
api_call "GET" "/returns" ""

# List shipments
echo -e "\n${GREEN}12. List Shipments${NC}"
api_call "GET" "/shipments" ""

# List work orders
echo -e "\n${GREEN}13. List Work Orders${NC}"
api_call "GET" "/work-orders" ""

echo -e "\n${GREEN}🎉 Demo completed!${NC}"
echo -e "${YELLOW}Note: Some endpoints may return empty results if no data exists yet.${NC}"
echo -e "${YELLOW}Use the API documentation at $BASE_URL/docs for detailed endpoint information.${NC}"
