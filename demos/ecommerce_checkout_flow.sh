#!/bin/bash
# Demo script: Complete eCommerce checkout flow
# This demonstrates the entire flow from customer registration to order placement

API_BASE="http://localhost:8080/api/v1"
AUTH_TOKEN=""
CUSTOMER_ID=""
PRODUCT_ID=""
VARIANT_ID=""
CART_ID=""
SESSION_ID=""
ORDER_ID=""

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Stateset eCommerce OS Demo: Complete Checkout Flow ===${NC}\n"

# Step 1: Register a new customer
echo -e "${GREEN}Step 1: Registering new customer...${NC}"
CUSTOMER_RESPONSE=$(curl -s -X POST "$API_BASE/customers/register" \
  -H "Content-Type: application/json" \
  -d '{
    "email": "john.doe@example.com",
    "password": "SecurePass123!",
    "first_name": "John",
    "last_name": "Doe",
    "phone": "+1-555-0123",
    "accepts_marketing": true
  }')

echo "Customer registered:"
echo "$CUSTOMER_RESPONSE" | jq '.'
CUSTOMER_ID=$(echo "$CUSTOMER_RESPONSE" | jq -r '.id')
echo -e "\n"

# Step 2: Login customer
echo -e "${GREEN}Step 2: Logging in customer...${NC}"
LOGIN_RESPONSE=$(curl -s -X POST "$API_BASE/customers/login" \
  -H "Content-Type: application/json" \
  -d '{
    "email": "john.doe@example.com",
    "password": "SecurePass123!"
  }')

echo "Login successful:"
echo "$LOGIN_RESPONSE" | jq '.'
AUTH_TOKEN=$(echo "$LOGIN_RESPONSE" | jq -r '.tokens.access_token')
echo -e "\n"

# Step 3: Add customer address
echo -e "${GREEN}Step 3: Adding customer address...${NC}"
ADDRESS_RESPONSE=$(curl -s -X POST "$API_BASE/customers/me/addresses" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d '{
    "first_name": "John",
    "last_name": "Doe",
    "address_line_1": "123 Main Street",
    "address_line_2": "Apt 4B",
    "city": "New York",
    "province": "NY",
    "country_code": "US",
    "postal_code": "10001",
    "phone": "+1-555-0123",
    "is_default_shipping": true,
    "is_default_billing": true
  }')

echo "Address added:"
echo "$ADDRESS_RESPONSE" | jq '.'
echo -e "\n"

# Step 4: Browse products
echo -e "${GREEN}Step 4: Browsing products...${NC}"
PRODUCTS_RESPONSE=$(curl -s -X GET "$API_BASE/products?limit=5" \
  -H "Authorization: Bearer $AUTH_TOKEN")

echo "Available products:"
echo "$PRODUCTS_RESPONSE" | jq '.'
PRODUCT_ID=$(echo "$PRODUCTS_RESPONSE" | jq -r '.data[0].id')
echo -e "\n"

# Step 5: Get product details with variants
echo -e "${GREEN}Step 5: Getting product details...${NC}"
PRODUCT_DETAILS=$(curl -s -X GET "$API_BASE/products/$PRODUCT_ID" \
  -H "Authorization: Bearer $AUTH_TOKEN")

echo "Product details:"
echo "$PRODUCT_DETAILS" | jq '.'
VARIANT_ID=$(echo "$PRODUCT_DETAILS" | jq -r '.variants[0].id')
echo -e "\n"

# Step 6: Create cart
echo -e "${GREEN}Step 6: Creating shopping cart...${NC}"
CART_RESPONSE=$(curl -s -X POST "$API_BASE/carts" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d "{
    \"customer_id\": \"$CUSTOMER_ID\",
    \"currency\": \"USD\"
  }")

echo "Cart created:"
echo "$CART_RESPONSE" | jq '.'
CART_ID=$(echo "$CART_RESPONSE" | jq -r '.id')
echo -e "\n"

# Step 7: Add items to cart
echo -e "${GREEN}Step 7: Adding items to cart...${NC}"
ADD_ITEM_RESPONSE=$(curl -s -X POST "$API_BASE/carts/$CART_ID/items" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d "{
    \"variant_id\": \"$VARIANT_ID\",
    \"quantity\": 2
  }")

echo "Item added to cart:"
echo "$ADD_ITEM_RESPONSE" | jq '.'
echo -e "\n"

# Step 8: View cart
echo -e "${GREEN}Step 8: Viewing cart contents...${NC}"
CART_VIEW=$(curl -s -X GET "$API_BASE/carts/$CART_ID" \
  -H "Authorization: Bearer $AUTH_TOKEN")

echo "Current cart:"
echo "$CART_VIEW" | jq '.'
echo -e "\n"

# Step 9: Start checkout
echo -e "${GREEN}Step 9: Starting checkout...${NC}"
CHECKOUT_START=$(curl -s -X POST "$API_BASE/checkout" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d "{
    \"cart_id\": \"$CART_ID\"
  }")

echo "Checkout session started:"
echo "$CHECKOUT_START" | jq '.'
SESSION_ID=$(echo "$CHECKOUT_START" | jq -r '.id')
echo -e "\n"

# Step 10: Set customer info
echo -e "${GREEN}Step 10: Setting customer info...${NC}"
curl -s -X PUT "$API_BASE/checkout/$SESSION_ID/customer" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d '{
    "email": "john.doe@example.com",
    "subscribe_newsletter": true
  }' | jq '.'
echo -e "\n"

# Step 11: Set shipping address
echo -e "${GREEN}Step 11: Setting shipping address...${NC}"
curl -s -X PUT "$API_BASE/checkout/$SESSION_ID/shipping-address" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d '{
    "first_name": "John",
    "last_name": "Doe",
    "address_line_1": "123 Main Street",
    "address_line_2": "Apt 4B",
    "city": "New York",
    "province": "NY",
    "country_code": "US",
    "postal_code": "10001",
    "phone": "+1-555-0123"
  }' | jq '.'
echo -e "\n"

# Step 12: Select shipping method
echo -e "${GREEN}Step 12: Selecting shipping method...${NC}"
SHIPPING_RESPONSE=$(curl -s -X PUT "$API_BASE/checkout/$SESSION_ID/shipping-method" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d '{
    "method": "Standard"
  }')

echo "Shipping method selected:"
echo "$SHIPPING_RESPONSE" | jq '.'
echo -e "\n"

# Step 13: Complete checkout
echo -e "${GREEN}Step 13: Completing checkout with payment...${NC}"
ORDER_RESPONSE=$(curl -s -X POST "$API_BASE/checkout/$SESSION_ID/complete" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $AUTH_TOKEN" \
  -d '{
    "payment_method": "CreditCard",
    "payment_token": "tok_test_visa_4242"
  }')

echo "Order created:"
echo "$ORDER_RESPONSE" | jq '.'
ORDER_ID=$(echo "$ORDER_RESPONSE" | jq -r '.order_id')
echo -e "\n"

# Step 14: View order
echo -e "${GREEN}Step 14: Viewing order details...${NC}"
ORDER_DETAILS=$(curl -s -X GET "$API_BASE/orders/$ORDER_ID" \
  -H "Authorization: Bearer $AUTH_TOKEN")

echo "Order details:"
echo "$ORDER_DETAILS" | jq '.'
echo -e "\n"

echo -e "${BLUE}=== Demo Complete! ===${NC}"
echo -e "Customer ID: $CUSTOMER_ID"
echo -e "Cart ID: $CART_ID"
echo -e "Order ID: $ORDER_ID"
echo -e "\n${GREEN}Successfully demonstrated the complete eCommerce checkout flow!${NC}" 