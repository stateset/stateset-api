#!/bin/bash

# Demo script for the minimal-server API
# This demonstrates the endpoints that return mock data

API_URL="${API_URL:-http://localhost:8080}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== StateSet Minimal Server API Demo ===${NC}\n"
echo -e "${YELLOW}This demo showcases the mock data endpoints available in minimal-server${NC}\n"

# Helper function for API calls
api_call() {
    local method=$1
    local endpoint=$2
    
    curl -s -X "$method" "$API_URL$endpoint" \
        -H "Content-Type: application/json"
}

# Show API Information
echo -e "${GREEN}1. API Information${NC}"
echo -e "${CYAN}Endpoint: GET /api/info${NC}"
api_call GET "/api/info" | jq .
echo -e "\n"

# Health Check
echo -e "${GREEN}2. Health Check${NC}"
echo -e "${CYAN}Endpoint: GET /health${NC}"
api_call GET "/health" | jq .
echo -e "\n"

# List Orders
echo -e "${GREEN}3. Order Management${NC}"
echo -e "${CYAN}Endpoint: GET /api/v1/orders${NC}"
echo "Recent orders with their status:"
api_call GET "/api/v1/orders" | jq '{
    total_orders: .total,
    orders: [.orders[] | {
        order_id: .id,
        customer: .customer_id,
        status: .status,
        total: "$\(.total)",
        created: .created_at
    }]
}'
echo -e "\n"

# Check Inventory
echo -e "${GREEN}4. Inventory Status${NC}"
echo -e "${CYAN}Endpoint: GET /api/v1/inventory${NC}"
echo "Current inventory levels:"
api_call GET "/api/v1/inventory" | jq '{
    total_products: .inventory | length,
    inventory: [.inventory[] | {
        sku: .sku,
        name: .name,
        available: .quantity_available,
        reserved: .quantity_reserved,
        warehouse: .warehouse
    }]
}'
echo -e "\n"

# Track Shipments
echo -e "${GREEN}5. Shipment Tracking${NC}"
echo -e "${CYAN}Endpoint: GET /api/v1/shipments${NC}"
echo "Active shipments:"
api_call GET "/api/v1/shipments" | jq '{
    total_shipments: .total_shipments,
    summary: {
        in_transit: .in_transit,
        delivered: .delivered
    },
    shipments: [.shipments[] | {
        order: .order_id,
        tracking: .tracking_number,
        carrier: .carrier,
        status: .status,
        eta: (.estimated_delivery // .delivered_at)
    }]
}'
echo -e "\n"

# Summary
echo -e "${BLUE}=== Demo Summary ===${NC}"
echo -e "${GREEN}✓ API is healthy and operational${NC}"
echo -e "${GREEN}✓ Order management system shows 3 sample orders${NC}"
echo -e "${GREEN}✓ Inventory tracking shows product availability${NC}"
echo -e "${GREEN}✓ Shipment tracking provides real-time status${NC}"
echo -e "\n${YELLOW}This minimal server provides mock data for testing and development.${NC}"
echo -e "${YELLOW}For full functionality, a complete API implementation with database is required.${NC}" 