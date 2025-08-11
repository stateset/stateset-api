#!/bin/bash

# Customer Lifecycle Management Demo Script
# Demonstrates customer onboarding, segmentation, loyalty programs, and analytics

set -e

API_URL="http://localhost:8080"
AUTH_TOKEN="${AUTH_TOKEN:-test-token}"

echo "=============================================="
echo "StateSet API - Customer Lifecycle Management Demo"
echo "=============================================="
echo ""

# Helper function for API calls
api_call() {
    local method=$1
    local endpoint=$2
    local data=$3
    
    echo "→ $method $endpoint"
    
    if [ -z "$data" ]; then
        response=$(curl -s -X "$method" \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            "$API_URL$endpoint")
    else
        response=$(curl -s -X "$method" \
            -H "Authorization: Bearer $AUTH_TOKEN" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "$API_URL$endpoint")
    fi
    
    echo "$response" | jq '.' 2>/dev/null || echo "$response"
    echo ""
}

# Step 1: Customer onboarding
echo "Step 1: Customer onboarding process..."
echo "--------------------------------------"

# Create a new retail customer
CUSTOMER_DATA='{
  "name": "Jennifer Martinez",
  "email": "jennifer.martinez@example.com",
  "phone": "+1-555-234-5678",
  "date_of_birth": "1985-03-15",
  "type": "RETAIL",
  "source": "WEBSITE",
  "referral_code": "FRIEND2024",
  "preferences": {
    "communication": {
      "email_marketing": true,
      "sms_notifications": true,
      "push_notifications": false
    },
    "shopping": {
      "preferred_categories": ["Electronics", "Home & Garden"],
      "price_sensitivity": "MEDIUM"
    }
  },
  "address": {
    "street": "1234 Maple Avenue",
    "city": "Denver",
    "state": "CO",
    "zip": "80202",
    "country": "US"
  }
}'
CUSTOMER_ID=$(api_call POST "/api/customers" "$CUSTOMER_DATA" | jq -r '.id')
echo "✓ Customer created: $CUSTOMER_ID"

# Verify email address
VERIFY_EMAIL='{
  "customer_id": "'$CUSTOMER_ID'",
  "verification_code": "123456",
  "verified_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'"
}'
api_call POST "/api/customers/$CUSTOMER_ID/verify-email" "$VERIFY_EMAIL"
echo "✓ Email verified"

# Send welcome email
WELCOME_EMAIL='{
  "type": "WELCOME",
  "recipient": "'$CUSTOMER_ID'",
  "template": "new_customer_welcome",
  "data": {
    "customer_name": "Jennifer",
    "welcome_discount": "15%",
    "discount_code": "WELCOME15"
  }
}'
api_call POST "/api/notifications/send" "$WELCOME_EMAIL"
echo "✓ Welcome email sent"
echo ""

# Step 2: Create B2B customer
echo "Step 2: Creating B2B customer..."
echo "--------------------------------"
B2B_CUSTOMER='{
  "name": "Innovate Tech Solutions",
  "email": "purchasing@innovatetech.com",
  "phone": "+1-555-876-5432",
  "type": "B2B",
  "tax_id": "87-1234567",
  "credit_limit": 50000.00,
  "payment_terms": "NET30",
  "account_manager": "sales-rep-001",
  "contacts": [
    {
      "name": "David Chen",
      "role": "Purchasing Manager",
      "email": "david.chen@innovatetech.com",
      "phone": "+1-555-876-5433",
      "primary": true
    },
    {
      "name": "Lisa Wang",
      "role": "Accounts Payable",
      "email": "lisa.wang@innovatetech.com",
      "phone": "+1-555-876-5434"
    }
  ],
  "billing_address": {
    "street": "888 Corporate Plaza",
    "city": "San Jose",
    "state": "CA",
    "zip": "95110",
    "country": "US"
  },
  "shipping_addresses": [
    {
      "name": "Main Warehouse",
      "street": "999 Industrial Park",
      "city": "San Jose",
      "state": "CA",
      "zip": "95112",
      "country": "US",
      "default": true
    }
  ]
}'
B2B_ID=$(api_call POST "/api/customers" "$B2B_CUSTOMER" | jq -r '.id')
echo "✓ B2B customer created: $B2B_ID"
echo ""

# Step 3: Customer segmentation
echo "Step 3: Applying customer segmentation..."
echo "-----------------------------------------"

# Add customer to segments
SEGMENT_DATA='{
  "customer_id": "'$CUSTOMER_ID'",
  "segments": [
    {
      "name": "NEW_CUSTOMERS",
      "added_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
      "auto_assigned": true
    },
    {
      "name": "ELECTRONICS_ENTHUSIAST",
      "added_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
      "criteria": "preferred_category"
    }
  ]
}'
api_call POST "/api/customers/$CUSTOMER_ID/segments" "$SEGMENT_DATA"
echo "✓ Customer segments assigned"

# Create custom segment
CUSTOM_SEGMENT='{
  "name": "HIGH_VALUE_PROSPECTS",
  "description": "New customers with high engagement potential",
  "criteria": {
    "customer_type": "RETAIL",
    "days_since_signup": {"max": 30},
    "email_verified": true,
    "preferences.communication.email_marketing": true
  },
  "auto_update": true
}'
SEGMENT_ID=$(api_call POST "/api/segments" "$CUSTOM_SEGMENT" | jq -r '.id')
echo "✓ Custom segment created: $SEGMENT_ID"
echo ""

# Step 4: First purchase
echo "Step 4: Processing first purchase..."
echo "------------------------------------"
FIRST_ORDER='{
  "customer_id": "'$CUSTOMER_ID'",
  "items": [
    {
      "product_id": "prod-laptop-002",
      "product_name": "UltraBook Pro 13\"",
      "quantity": 1,
      "price": 1199.99
    },
    {
      "product_id": "prod-bag-001",
      "product_name": "Premium Laptop Bag",
      "quantity": 1,
      "price": 79.99
    }
  ],
  "discount_code": "WELCOME15",
  "discount_amount": 191.99,
  "subtotal": 1279.98,
  "total": 1087.99,
  "payment_method": "CREDIT_CARD",
  "shipping_address": {
    "street": "1234 Maple Avenue",
    "city": "Denver",
    "state": "CO",
    "zip": "80202",
    "country": "US"
  }
}'
ORDER_ID=$(api_call POST "/api/orders" "$FIRST_ORDER" | jq -r '.id')
echo "✓ First order created: $ORDER_ID"

# Update customer metrics
UPDATE_METRICS='{
  "first_purchase_date": "'$(date -u +"%Y-%m-%d")'",
  "total_orders": 1,
  "total_spent": 1087.99,
  "average_order_value": 1087.99,
  "last_order_date": "'$(date -u +"%Y-%m-%d")'"
}'
api_call PATCH "/api/customers/$CUSTOMER_ID/metrics" "$UPDATE_METRICS"
echo "✓ Customer metrics updated"
echo ""

# Step 5: Loyalty program enrollment
echo "Step 5: Enrolling in loyalty program..."
echo "---------------------------------------"
LOYALTY_ENROLLMENT='{
  "customer_id": "'$CUSTOMER_ID'",
  "program": "STATESET_REWARDS",
  "tier": "BRONZE",
  "points_balance": 1088,
  "enrollment_date": "'$(date -u +"%Y-%m-%d")'",
  "enrollment_source": "FIRST_PURCHASE",
  "member_number": "SSR-2024-'$CUSTOMER_ID'"
}'
LOYALTY_ID=$(api_call POST "/api/loyalty/enroll" "$LOYALTY_ENROLLMENT" | jq -r '.id')
echo "✓ Enrolled in loyalty program: $LOYALTY_ID"

# Award bonus points
BONUS_POINTS='{
  "customer_id": "'$CUSTOMER_ID'",
  "points": 500,
  "reason": "WELCOME_BONUS",
  "description": "Welcome bonus for joining StateSet Rewards",
  "expires_at": "'$(date -u -d "+1 year" +"%Y-%m-%d")'"
}'
api_call POST "/api/loyalty/points/add" "$BONUS_POINTS"
echo "✓ Welcome bonus points awarded"
echo ""

# Step 6: Customer engagement activities
echo "Step 6: Tracking customer engagement..."
echo "---------------------------------------"

# Log website activity
ACTIVITY1='{
  "customer_id": "'$CUSTOMER_ID'",
  "type": "PRODUCT_VIEW",
  "timestamp": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
  "data": {
    "product_id": "prod-headphones-001",
    "product_name": "Wireless ANC Headphones",
    "category": "Electronics",
    "price": 299.99,
    "time_spent_seconds": 45
  }
}'
api_call POST "/api/customers/$CUSTOMER_ID/activities" "$ACTIVITY1"

# Add to wishlist
WISHLIST_ADD='{
  "customer_id": "'$CUSTOMER_ID'",
  "items": [
    {
      "product_id": "prod-headphones-001",
      "added_at": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'",
      "priority": "HIGH",
      "notes": "Birthday gift idea"
    }
  ]
}'
api_call POST "/api/customers/$CUSTOMER_ID/wishlist" "$WISHLIST_ADD"
echo "✓ Customer activities tracked"
echo ""

# Step 7: Customer service interaction
echo "Step 7: Customer service interaction..."
echo "--------------------------------------"
SUPPORT_TICKET='{
  "customer_id": "'$CUSTOMER_ID'",
  "subject": "Question about warranty coverage",
  "description": "I wanted to know if accidental damage is covered under the standard warranty for the laptop I purchased.",
  "priority": "MEDIUM",
  "category": "WARRANTY_INQUIRY",
  "channel": "EMAIL"
}'
TICKET_ID=$(api_call POST "/api/support/tickets" "$SUPPORT_TICKET" | jq -r '.id')
echo "✓ Support ticket created: $TICKET_ID"

# Add interaction note
INTERACTION_NOTE='{
  "customer_id": "'$CUSTOMER_ID'",
  "type": "SUPPORT",
  "channel": "EMAIL",
  "agent_id": "support-agent-003",
  "summary": "Customer inquired about warranty coverage",
  "outcome": "RESOLVED",
  "satisfaction_score": 5,
  "notes": "Explained standard vs extended warranty. Customer satisfied with response."
}'
api_call POST "/api/customers/$CUSTOMER_ID/interactions" "$INTERACTION_NOTE"
echo "✓ Interaction logged"
echo ""

# Step 8: Targeted marketing campaign
echo "Step 8: Creating targeted campaign..."
echo "------------------------------------"
CAMPAIGN_DATA='{
  "name": "Electronics Spring Sale",
  "segment_id": "'$SEGMENT_ID'",
  "type": "EMAIL",
  "start_date": "'$(date -u +"%Y-%m-%d")'",
  "end_date": "'$(date -u -d "+7 days" +"%Y-%m-%d")'",
  "content": {
    "subject": "Exclusive 20% off Electronics for You!",
    "preview": "As a valued customer, enjoy special savings",
    "template": "spring_sale_electronics"
  },
  "personalization": {
    "use_customer_name": true,
    "show_wishlist_items": true,
    "include_loyalty_points": true
  }
}'
CAMPAIGN_ID=$(api_call POST "/api/marketing/campaigns" "$CAMPAIGN_DATA" | jq -r '.id')
echo "✓ Marketing campaign created: $CAMPAIGN_ID"

# Send campaign to customer
SEND_CAMPAIGN='{
  "campaign_id": "'$CAMPAIGN_ID'",
  "customer_ids": ["'$CUSTOMER_ID'"],
  "test_mode": false
}'
api_call POST "/api/marketing/campaigns/$CAMPAIGN_ID/send" "$SEND_CAMPAIGN"
echo "✓ Campaign sent to customer"
echo ""

# Step 9: Customer lifetime value calculation
echo "Step 9: Calculating customer lifetime value..."
echo "----------------------------------------------"
CLV_PARAMS='{
  "customer_id": "'$CUSTOMER_ID'",
  "calculation_method": "PREDICTIVE",
  "time_horizon_months": 24,
  "include_factors": [
    "purchase_history",
    "engagement_score",
    "loyalty_tier",
    "return_rate"
  ]
}'
CLV_RESULT=$(api_call POST "/api/analytics/customer-lifetime-value" "$CLV_PARAMS")
echo "✓ CLV calculated"
echo ""

# Step 10: Generate customer 360 view
echo "Step 10: Generating customer 360° view..."
echo "-----------------------------------------"
api_call GET "/api/customers/$CUSTOMER_ID/360-view"
echo ""

# Step 11: B2B account management
echo "Step 11: B2B account management..."
echo "----------------------------------"

# Create quote for B2B customer
QUOTE_DATA='{
  "customer_id": "'$B2B_ID'",
  "valid_until": "'$(date -u -d "+30 days" +"%Y-%m-%d")'",
  "items": [
    {
      "product_id": "prod-server-002",
      "product_name": "Enterprise Server Rack",
      "quantity": 5,
      "unit_price": 3999.99,
      "discount_percent": 15
    }
  ],
  "payment_terms": "NET30",
  "notes": "Volume discount applied. Installation services available."
}'
QUOTE_ID=$(api_call POST "/api/quotes" "$QUOTE_DATA" | jq -r '.id')
echo "✓ B2B quote created: $QUOTE_ID"

# Set credit terms
CREDIT_TERMS='{
  "credit_limit": 50000.00,
  "payment_terms": "NET30",
  "credit_status": "APPROVED",
  "reviewed_by": "finance-manager-001",
  "next_review_date": "'$(date -u -d "+6 months" +"%Y-%m-%d")'"
}'
api_call PUT "/api/customers/$B2B_ID/credit-terms" "$CREDIT_TERMS"
echo "✓ B2B credit terms set"
echo ""

# Step 12: Customer retention analysis
echo "Step 12: Running retention analysis..."
echo "--------------------------------------"
RETENTION_ANALYSIS='{
  "segment": "ALL_CUSTOMERS",
  "period": "LAST_QUARTER",
  "metrics": [
    "churn_rate",
    "retention_rate",
    "repeat_purchase_rate",
    "average_time_between_purchases"
  ]
}'
api_call POST "/api/analytics/retention" "$RETENTION_ANALYSIS"
echo ""

# Step 13: Customer feedback
echo "Step 13: Collecting customer feedback..."
echo "----------------------------------------"
FEEDBACK_DATA='{
  "customer_id": "'$CUSTOMER_ID'",
  "type": "NPS",
  "score": 9,
  "comment": "Great experience! Fast shipping and excellent product quality.",
  "categories": ["SHIPPING", "PRODUCT_QUALITY"],
  "would_recommend": true,
  "survey_id": "NPS-2024-Q1"
}'
api_call POST "/api/feedback" "$FEEDBACK_DATA"
echo "✓ Customer feedback recorded"
echo ""

# Step 14: Export customer data (GDPR compliance)
echo "Step 14: Demonstrating data privacy compliance..."
echo "-------------------------------------------------"
EXPORT_REQUEST='{
  "customer_id": "'$CUSTOMER_ID'",
  "format": "JSON",
  "include_data": [
    "personal_information",
    "order_history",
    "communication_preferences",
    "loyalty_data",
    "support_interactions"
  ],
  "purpose": "CUSTOMER_REQUEST",
  "requested_by": "'$CUSTOMER_ID'"
}'
EXPORT_ID=$(api_call POST "/api/customers/$CUSTOMER_ID/data-export" "$EXPORT_REQUEST" | jq -r '.export_id')
echo "✓ Data export requested: $EXPORT_ID"
echo ""

# Step 15: Generate customer analytics dashboard
echo "Step 15: Generating customer analytics..."
echo "-----------------------------------------"
ANALYTICS_REQUEST='{
  "metrics": [
    "total_customers",
    "new_customers_this_month",
    "average_order_value",
    "customer_lifetime_value",
    "churn_rate",
    "nps_score"
  ],
  "group_by": "customer_type",
  "period": "LAST_90_DAYS"
}'
api_call POST "/api/analytics/customers/dashboard" "$ANALYTICS_REQUEST"
echo ""

echo "=============================================="
echo "Customer Lifecycle Management Demo Complete!"
echo "=============================================="
echo ""
echo "Summary of operations:"
echo "- Created retail customer: $CUSTOMER_ID"
echo "- Created B2B customer: $B2B_ID"
echo "- Enrolled in loyalty program: $LOYALTY_ID"
echo "- Processed first order: $ORDER_ID"
echo "- Created support ticket: $TICKET_ID"
echo "- Launched marketing campaign: $CAMPAIGN_ID"
echo "- Generated B2B quote: $QUOTE_ID"
echo "- Demonstrated GDPR compliance"
echo "- Generated customer analytics"
echo "" 