#!/bin/bash

###############################################################################
# StablePay Crypto Demo - USDC & USDT Payments
# 
# Features:
# - Instant stablecoin payments (USDC, USDT)
# - Multi-blockchain support (Ethereum, Polygon, Arbitrum, Optimism, Base)
# - Crypto wallet management
# - Low fees (0.5% vs 2.9%)
# - Instant settlement
###############################################################################

set -e

BASE_URL="${BASE_URL:-http://localhost:8000}"
API_PREFIX="/api/v1/stablepay"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}"
echo "╔════════════════════════════════════════════════════════════════╗"
echo "║              StablePay Crypto Demo                            ║"
echo "║            USDC & USDT Payments                                ║"
echo "║                                                                ║"
echo "║  ✓ Instant Payments (seconds, not days)                        ║"
echo "║  ✓ Ultra-Low Fees (0.5% vs 2.9%)                              ║"
echo "║  ✓ Multi-Blockchain (ETH, Polygon, Arbitrum, Optimism, Base)  ║"
echo "║  ✓ Stablecoins (USDC, USDT)                                   ║"
echo "║  ✓ Global Access (no geographic restrictions)                  ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# Function to make API calls with pretty output
api_call() {
    local method=$1
    local endpoint=$2
    local data=$3
    local description=$4
    
    echo -e "\n${YELLOW}▶ ${description}${NC}"
    echo -e "${BLUE}${method} ${BASE_URL}${endpoint}${NC}"
    
    if [ -n "$data" ]; then
        echo -e "${BLUE}Request:${NC}"
        echo "$data" | jq '.'
    fi
    
    response=$(curl -s -X "${method}" \
        -H "Content-Type: application/json" \
        ${data:+-d "$data"} \
        "${BASE_URL}${endpoint}")
    
    echo -e "${GREEN}Response:${NC}"
    echo "$response" | jq '.'
    echo "$response"
}

# Step 1: Health Check
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 1: Health Check${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

health_response=$(api_call "GET" "${API_PREFIX}/crypto/health" "" "Checking StablePay Crypto service")

# Step 2: Get Supported Blockchains
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 2: Get Supported Blockchains${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

blockchains_response=$(api_call "GET" "${API_PREFIX}/crypto/blockchains" "" \
    "Retrieving supported blockchains")

echo -e "\n${CYAN}Supported Blockchains:${NC}"
echo "$blockchains_response" | jq -r '.data[] | "  • \(.full_name) (\(.blockchain)) - Chain ID: \(.chain_id // "N/A"), Confirmations: ~\(.estimated_confirmation_time_minutes // "N/A") min"'

sleep 1

# Generate IDs for the demo
CUSTOMER_ID=$(uuidgen | tr '[:upper:]' '[:lower:]')
ORDER_ID=$(uuidgen | tr '[:upper:]' '[:lower:]')

echo -e "\n${BLUE}Generated Demo IDs:${NC}"
echo -e "Customer ID: ${CUSTOMER_ID}"
echo -e "Order ID: ${ORDER_ID}"

# Step 3: Add Crypto Wallet
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 3: Add Customer Crypto Wallet${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Example customer wallet address (Ethereum format)
CUSTOMER_WALLET="0x1234567890123456789012345678901234567890"

wallet_request=$(cat <<EOF
{
  "customer_id": "$CUSTOMER_ID",
  "wallet_address": "$CUSTOMER_WALLET",
  "blockchain": "ethereum",
  "label": "My MetaMask Wallet",
  "set_as_default": true
}
EOF
)

wallet_response=$(api_call "POST" "${API_PREFIX}/crypto/wallets" "$wallet_request" \
    "Adding customer crypto wallet")

WALLET_ID=$(echo "$wallet_response" | jq -r '.data.id // empty')

if [ -n "$WALLET_ID" ]; then
    echo -e "\n${GREEN}✓ Wallet added successfully!${NC}"
    echo -e "Wallet ID: ${WALLET_ID}"
    echo -e "Address: $(echo "$wallet_response" | jq -r '.data.wallet_address')"
    echo -e "Short Address: $(echo "$wallet_response" | jq -r '.data.short_address')"
    echo -e "Blockchain: $(echo "$wallet_response" | jq -r '.data.blockchain')"
fi

sleep 1

# Step 4: Create USDC Payment on Polygon (fast & cheap)
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 4: Create USDC Payment on Polygon${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo -e "\n${CYAN}Why Polygon?${NC}"
echo -e "  • Ultra-fast: ~2 second blocks"
echo -e "  • Ultra-cheap: ~$0.01 gas fees"
echo -e "  • Ethereum-compatible"
echo -e "  • Perfect for retail payments"

payment_usdc_request=$(cat <<EOF
{
  "customer_id": "$CUSTOMER_ID",
  "order_id": "$ORDER_ID",
  "amount": "299.99",
  "token_symbol": "USDC",
  "blockchain": "polygon",
  "from_address": "$CUSTOMER_WALLET",
  "description": "Premium Product Purchase",
  "metadata": {
    "product": "Enterprise License",
    "payment_method": "crypto"
  }
}
EOF
)

payment_usdc_response=$(api_call "POST" "${API_PREFIX}/crypto/payments" "$payment_usdc_request" \
    "Creating USDC payment on Polygon for $299.99")

PAYMENT_USDC_ID=$(echo "$payment_usdc_response" | jq -r '.data.payment_id // empty')

if [ -n "$PAYMENT_USDC_ID" ]; then
    echo -e "\n${GREEN}✓ USDC payment created successfully!${NC}"
    echo -e "Payment ID: ${PAYMENT_USDC_ID}"
    echo -e "Transaction Number: $(echo "$payment_usdc_response" | jq -r '.data.transaction_number')"
    echo -e "Amount: $(echo "$payment_usdc_response" | jq -r '.data.amount') USDC"
    echo -e "Blockchain: $(echo "$payment_usdc_response" | jq -r '.data.blockchain | ascii_upcase')"
    echo -e "Merchant Address: $(echo "$payment_usdc_response" | jq -r '.data.to_address')"
    echo -e "Status: $(echo "$payment_usdc_response" | jq -r '.data.status')"
    echo -e "Gas Estimate: ~$(echo "$payment_usdc_response" | jq -r '.data.gas_estimate_usd') USD"
    echo -e "Total Cost: $(echo "$payment_usdc_response" | jq -r '.data.total_cost_usd') USD"
    echo -e "Confirmations Required: $(echo "$payment_usdc_response" | jq -r '.data.required_confirmations')"
    echo -e "Est. Confirmation Time: ~$(echo "$payment_usdc_response" | jq -r '.data.estimated_confirmation_time_minutes') minutes"
fi

sleep 1

# Step 5: Create USDT Payment on Arbitrum (ultra-fast L2)
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 5: Create USDT Payment on Arbitrum${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo -e "\n${CYAN}Why Arbitrum?${NC}"
echo -e "  • Instant finality: 1 confirmation"
echo -e "  • Very low fees: ~$0.10"
echo -e "  • Ethereum security"
echo -e "  • Growing DeFi ecosystem"

payment_usdt_request=$(cat <<EOF
{
  "customer_id": "$CUSTOMER_ID",
  "amount": "1499.00",
  "token_symbol": "USDT",
  "blockchain": "arbitrum",
  "from_address": "$CUSTOMER_WALLET",
  "description": "Bulk Order Payment"
}
EOF
)

payment_usdt_response=$(api_call "POST" "${API_PREFIX}/crypto/payments" "$payment_usdt_request" \
    "Creating USDT payment on Arbitrum for $1,499.00")

PAYMENT_USDT_ID=$(echo "$payment_usdt_response" | jq -r '.data.payment_id // empty')

if [ -n "$PAYMENT_USDT_ID" ]; then
    echo -e "\n${GREEN}✓ USDT payment created successfully!${NC}"
    echo -e "Payment ID: ${PAYMENT_USDT_ID}"
    echo -e "Amount: $(echo "$payment_usdt_response" | jq -r '.data.amount') USDT"
    echo -e "Blockchain: $(echo "$payment_usdt_response" | jq -r '.data.blockchain | ascii_upcase')"
    echo -e "Confirmations Required: $(echo "$payment_usdt_response" | jq -r '.data.required_confirmations') (instant!)"
fi

sleep 1

# Step 6: Create USDC Payment on Base (Coinbase L2)
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 6: Create USDC Payment on Base${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo -e "\n${CYAN}Why Base?${NC}"
echo -e "  • Built by Coinbase"
echo -e "  • Instant finality"
echo -e "  • Native USDC support"
echo -e "  • Easy fiat on/off ramps"

payment_base_request=$(cat <<EOF
{
  "customer_id": "$CUSTOMER_ID",
  "amount": "599.99",
  "token_symbol": "USDC",
  "blockchain": "base",
  "from_address": "$CUSTOMER_WALLET",
  "description": "Subscription Renewal"
}
EOF
)

payment_base_response=$(api_call "POST" "${API_PREFIX}/crypto/payments" "$payment_base_request" \
    "Creating USDC payment on Base for $599.99")

sleep 1

# Step 7: List Customer Wallets
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 7: List Customer Wallets${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

wallets_response=$(api_call "GET" "${API_PREFIX}/crypto/customers/${CUSTOMER_ID}/wallets" "" \
    "Listing customer's crypto wallets")

wallet_count=$(echo "$wallets_response" | jq -r '.data | length // 0')
echo -e "\n${GREEN}✓ Found ${wallet_count} wallet(s)${NC}"

# Step 8: Cost Comparison
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 8: Cost Comparison - Crypto vs Traditional${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo -e "\n${YELLOW}Transaction: \$500.00${NC}"
echo ""
echo -e "${RED}Traditional Payment Processors:${NC}"
echo -e "  Stripe: 2.9% + \$0.30 = \$14.80"
echo -e "  PayPal: 3.49% + \$0.49 = \$17.94"
echo ""
echo -e "${GREEN}StablePay Crypto:${NC}"
echo -e "  Fee: 0.5% + \$0.00 = \$2.50"
echo -e "  Gas: ~\$0.01 (Polygon)"
echo -e "  Total: ~\$2.51"
echo ""
echo -e "${GREEN}Savings: \$12.29 per transaction (83% reduction!)${NC}"
echo ""
echo -e "${CYAN}Additional Benefits:${NC}"
echo -e "  ✓ Instant settlement (minutes vs days)"
echo -e "  ✓ No chargebacks"
echo -e "  ✓ Global access"
echo -e "  ✓ 24/7 availability"
echo -e "  ✓ No currency conversion fees"

echo -e "\n${BLUE}Annual Savings (10,000 transactions @ \$500):${NC}"
echo -e "${GREEN}\$122,900 saved per year!${NC}"

# Step 9: Feature Comparison
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Step 9: Crypto vs Traditional Comparison${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo -e "\n| Feature | Traditional | StablePay Crypto |"
echo -e "|---------|------------|------------------|"
echo -e "| ${CYAN}Fees${NC} | 2.9% - 3.49% | ${GREEN}0.5%${NC} |"
echo -e "| ${CYAN}Settlement${NC} | 3-7 days | ${GREEN}Minutes${NC} |"
echo -e "| ${CYAN}Chargebacks${NC} | Yes | ${GREEN}No${NC} |"
echo -e "| ${CYAN}Global Access${NC} | Limited | ${GREEN}Unlimited${NC} |"
echo -e "| ${CYAN}Currency Conversion${NC} | 2-3% | ${GREEN}0%${NC} |"
echo -e "| ${CYAN}Availability${NC} | Business hours | ${GREEN}24/7${NC} |"
echo -e "| ${CYAN}Minimum Amount${NC} | \$0.50 | ${GREEN}\$0.01${NC} |"
echo -e "| ${CYAN}Transaction Limits${NC} | Yes | ${GREEN}No${NC} |"

# Summary
echo -e "\n${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}Demo Summary${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

echo -e "\n${GREEN}✓ Successfully demonstrated:${NC}"
echo -e "  1. USDC & USDT payment support"
echo -e "  2. Multi-blockchain (Polygon, Arbitrum, Base)"
echo -e "  3. Crypto wallet management"
echo -e "  4. Instant settlements"
echo -e "  5. Ultra-low fees (0.5%)"
echo -e "  6. 83% cost savings vs traditional"

echo -e "\n${BLUE}Supported Networks:${NC}"
echo -e "  • Ethereum Mainnet"
echo -e "  • Polygon (recommended for retail)"
echo -e "  • Arbitrum One"
echo -e "  • Optimism"
echo -e "  • Base (Coinbase L2)"

echo -e "\n${BLUE}Supported Stablecoins:${NC}"
echo -e "  • USDC (USD Coin)"
echo -e "  • USDT (Tether)"

echo -e "\n${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}StablePay Crypto Demo Complete!${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""

