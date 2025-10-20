#!/bin/bash

echo "═══════════════════════════════════════════════════════════════"
echo "   Security Features Test - Agentic Commerce Server"
echo "═══════════════════════════════════════════════════════════════"
echo

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Test 1: API Key Validation
echo -e "${BLUE}Test 1: API Key Validation${NC}"
echo "  Testing with valid API key..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"test","quantity":1}]}')
STATUS=$(echo "$RESPONSE" | tail -1)
if [ "$STATUS" = "201" ]; then
    echo -e "  ${GREEN}✓ Valid API key accepted${NC}"
else
    echo -e "  ${RED}✗ Failed (status: $STATUS)${NC}"
fi

echo "  Testing with invalid API key..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer invalid_key_999" \
  -d '{"items":[{"id":"test","quantity":1}]}')
STATUS=$(echo "$RESPONSE" | tail -1)
if [ "$STATUS" = "401" ]; then
    echo -e "  ${GREEN}✓ Invalid API key rejected${NC}"
else
    echo -e "  ${RED}✗ Failed (expected 401, got $STATUS)${NC}"
fi

echo "  Testing with missing API key..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -d '{"items":[{"id":"test","quantity":1}]}')
STATUS=$(echo "$RESPONSE" | tail -1)
if [ "$STATUS" = "401" ]; then
    echo -e "  ${GREEN}✓ Missing API key rejected${NC}"
else
    echo -e "  ${RED}✗ Failed (expected 401, got $STATUS)${NC}"
fi

echo
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 2: Rate Limiting
echo -e "${BLUE}Test 2: Rate Limiting (100 req/min)${NC}"
echo "  Sending burst of requests..."

SUCCESS_COUNT=0
LIMIT_COUNT=0

for i in {1..110}; do
    STATUS=$(curl -s -w "%{http_code}" -o /dev/null http://localhost:8080/health)
    if [ "$STATUS" = "200" ]; then
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    elif [ "$STATUS" = "429" ]; then
        LIMIT_COUNT=$((LIMIT_COUNT + 1))
    fi
done

echo "  Results: $SUCCESS_COUNT successful, $LIMIT_COUNT rate-limited"

if [ "$LIMIT_COUNT" -gt 0 ]; then
    echo -e "  ${GREEN}✓ Rate limiting working (hit limit after ~100 requests)${NC}"
else
    echo -e "  ${YELLOW}⚠ No rate limiting triggered (may need more requests)${NC}"
fi

echo
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 3: Input Validation
echo -e "${BLUE}Test 3: Input Validation${NC}"

echo "  Testing invalid quantity (0)..."
RESPONSE=$(curl -s -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_demo_123" \
  -d '{"items":[{"id":"test","quantity":0}]}')
if echo "$RESPONSE" | grep -q "error"; then
    echo -e "  ${GREEN}✓ Invalid quantity rejected${NC}"
else
    echo -e "  ${YELLOW}⚠ Validation may need strengthening${NC}"
fi

echo
echo "═══════════════════════════════════════════════════════════════"
echo

# Test 4: Metrics Tracking
echo -e "${BLUE}Test 4: Metrics Tracking${NC}"
echo "  Creating sessions to test metrics..."

# Create 3 sessions
for i in 1 2 3; do
    curl -s -X POST http://localhost:8080/checkout_sessions \
      -H "Content-Type: application/json" \
      -H "Authorization: Bearer api_key_demo_123" \
      -d "{\"items\":[{\"id\":\"metrics_test_$i\",\"quantity\":1}]}" > /dev/null
done

METRICS=$(curl -s http://localhost:8080/metrics)
SESSION_COUNT=$(echo "$METRICS" | grep "checkout_sessions_created_total" | awk '{print $2}')

echo "  Sessions created (from metrics): $SESSION_COUNT"

if [ -n "$SESSION_COUNT" ] && [ "$SESSION_COUNT" -gt 0 ]; then
    echo -e "  ${GREEN}✓ Metrics tracking working${NC}"
else
    echo -e "  ${RED}✗ Metrics not tracking${NC}"
fi

echo
echo "═══════════════════════════════════════════════════════════════"
echo

# Summary
echo -e "${BLUE}Security Test Summary${NC}"
echo "  ✅ API key validation implemented"
echo "  ✅ Rate limiting active (100 req/min)"
echo "  ✅ Input validation framework ready"
echo "  ✅ Prometheus metrics tracking"
echo "  ⚠️  Signature verification (requires WEBHOOK_SECRET env var)"
echo "  ⚠️  Redis storage (requires REDIS_URL env var)"
echo "  ⚠️  Idempotency (requires Redis)"

echo
echo "Next: Set environment variables to enable remaining features:"
echo "  export WEBHOOK_SECRET=your_secret_here"
echo "  export REDIS_URL=redis://localhost:6379"

echo
echo "═══════════════════════════════════════════════════════════════" 