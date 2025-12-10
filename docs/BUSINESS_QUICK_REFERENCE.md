# Stateset API - Business Quick Reference Card

**Your Ecommerce Operating System - Command Cheat Sheet**

---

## Quick Start Commands

```bash
# Clone and setup
git clone https://github.com/stateset/stateset-api.git
cd stateset-api
cp .env.example .env

# Run migrations
cargo run --bin migration

# Start server
cargo run --bin stateset-api

# Access Swagger UI
open http://localhost:8080/swagger-ui
```

---

## Essential API Endpoints

### Authentication

```bash
# Register
POST /api/v1/auth/register
{
  "email": "user@example.com",
  "password": "SecurePass123!",
  "full_name": "John Doe"
}

# Login
POST /api/v1/auth/login
{
  "email": "user@example.com",
  "password": "SecurePass123!"
}

# Create API Key
POST /api/v1/auth/api-keys
{
  "name": "Production API Key",
  "permissions": ["orders:read", "orders:create"]
}
```

### Products

```bash
# Create Product
POST /api/v1/products
{
  "name": "Product Name",
  "sku": "PROD-001",
  "price": 29.99,
  "description": "Product description",
  "active": true
}

# List Products
GET /api/v1/products?page=1&limit=20

# Get Product
GET /api/v1/products/{product_id}

# Update Product
PATCH /api/v1/products/{product_id}
{
  "price": 34.99,
  "active": true
}
```

### Orders

```bash
# Create Order
POST /api/v1/orders
{
  "customer_email": "customer@example.com",
  "items": [
    {
      "product_id": "prod_123",
      "quantity": 2,
      "price": 29.99
    }
  ],
  "shipping_address": {
    "name": "John Doe",
    "street": "123 Main St",
    "city": "San Francisco",
    "state": "CA",
    "postal_code": "94102",
    "country": "US"
  }
}

# Get Order
GET /api/v1/orders/{order_id}

# List Orders
GET /api/v1/orders?status=pending&limit=50

# Update Order Status
PATCH /api/v1/orders/{order_id}/status
{
  "status": "processing"
}

# Cancel Order
POST /api/v1/orders/{order_id}/cancel
{
  "reason": "Customer requested cancellation"
}
```

### Inventory

```bash
# Check Inventory
GET /api/v1/inventory?product_id=prod_123&location=WAREHOUSE_A

# Reserve Inventory
POST /api/v1/inventory/reserve
{
  "product_id": "prod_123",
  "quantity": 5,
  "location": "WAREHOUSE_A",
  "order_id": "ord_456"
}

# Adjust Inventory
POST /api/v1/inventory/adjust
{
  "product_id": "prod_123",
  "location": "WAREHOUSE_A",
  "quantity_change": 100,
  "reason": "restock",
  "notes": "Weekly restock"
}

# Low Stock Alert
GET /api/v1/inventory/low-stock?threshold=10
```

### Shopping Cart

```bash
# Create Cart
POST /api/v1/carts
{
  "session_id": "sess_abc123",
  "customer_id": "cust_456"
}

# Add Item to Cart
POST /api/v1/carts/{cart_id}/items
{
  "product_id": "prod_123",
  "quantity": 2
}

# Get Cart
GET /api/v1/carts/{cart_id}

# Update Cart Item
PATCH /api/v1/carts/{cart_id}/items/{item_id}
{
  "quantity": 3
}

# Remove Item
DELETE /api/v1/carts/{cart_id}/items/{item_id}
```

### Checkout

```bash
# Initiate Checkout
POST /api/v1/checkout/initiate
{
  "cart_id": "cart_123"
}

# Set Shipping Address
POST /api/v1/checkout/{checkout_id}/shipping-address
{
  "name": "John Doe",
  "street": "123 Main St",
  "city": "San Francisco",
  "state": "CA",
  "postal_code": "94102",
  "country": "US"
}

# Select Shipping Method
POST /api/v1/checkout/{checkout_id}/shipping-method
{
  "carrier": "UPS",
  "service_level": "ground"
}

# Complete Checkout
POST /api/v1/checkout/{checkout_id}/complete
{
  "payment_method": "credit_card",
  "stripe_token": "tok_visa",
  "billing_address": {...}
}
```

### Payments

```bash
# Process Payment
POST /api/v1/payments
{
  "order_id": "ord_123",
  "amount": 59.98,
  "currency": "USD",
  "payment_method": "credit_card",
  "stripe_token": "tok_visa"
}

# Process Crypto Payment
POST /api/v1/payments
{
  "order_id": "ord_123",
  "amount": 59.98,
  "currency": "USD",
  "payment_method": "crypto",
  "crypto_currency": "USDC",
  "customer_wallet": "0x742d35..."
}

# Get Payment
GET /api/v1/payments/{payment_id}

# Process Refund
POST /api/v1/payments/{payment_id}/refund
{
  "amount": 59.98,
  "reason": "Customer requested refund"
}
```

### Returns

```bash
# Create Return
POST /api/v1/returns
{
  "order_id": "ord_123",
  "items": [
    {
      "order_item_id": "item_456",
      "quantity": 1,
      "reason": "size_too_small"
    }
  ],
  "customer_note": "Need larger size"
}

# Get Return
GET /api/v1/returns/{return_id}

# Approve Return
POST /api/v1/returns/{return_id}/approve
{
  "refund_amount": 29.99,
  "notes": "Approved for full refund"
}

# Reject Return
POST /api/v1/returns/{return_id}/reject
{
  "reason": "Outside return window"
}

# Restock Returned Items
POST /api/v1/returns/{return_id}/restock
{
  "condition": "like_new",
  "location": "WAREHOUSE_A"
}
```

### Shipments

```bash
# Create Shipment
POST /api/v1/shipments
{
  "order_id": "ord_123",
  "carrier": "UPS",
  "service_level": "ground",
  "tracking_number": "1Z999AA10123456784",
  "items": [
    {
      "order_item_id": "item_456",
      "quantity": 2
    }
  ]
}

# Get Shipment
GET /api/v1/shipments/{shipment_id}

# Track by Tracking Number
GET /api/v1/shipments/track/{tracking_number}

# Mark as Shipped
PATCH /api/v1/shipments/{shipment_id}/ship
{
  "shipped_at": "2024-01-15T10:30:00Z"
}

# Mark as Delivered
PATCH /api/v1/shipments/{shipment_id}/deliver
{
  "delivered_at": "2024-01-18T14:20:00Z"
}
```

### Customers

```bash
# Create Customer
POST /api/v1/customers
{
  "email": "customer@example.com",
  "first_name": "John",
  "last_name": "Doe",
  "phone": "+1234567890"
}

# Get Customer
GET /api/v1/customers/{customer_id}

# List Customers
GET /api/v1/customers?page=1&limit=50

# Add Customer Address
POST /api/v1/customers/{customer_id}/addresses
{
  "label": "Home",
  "street": "123 Main St",
  "city": "San Francisco",
  "state": "CA",
  "postal_code": "94102",
  "country": "US",
  "is_default": true
}

# Get Customer Orders
GET /api/v1/customers/{customer_id}/orders
```

### Analytics

```bash
# Dashboard Metrics
GET /api/v1/analytics/dashboard?period=30days

# Sales Trends
GET /api/v1/analytics/sales-trends?start_date=2024-01-01&end_date=2024-01-31

# Inventory Analytics
GET /api/v1/analytics/inventory?location=WAREHOUSE_A

# Top Products
GET /api/v1/analytics/top-products?limit=10&period=7days

# Customer Lifetime Value
GET /api/v1/analytics/customer-ltv/{customer_id}
```

### Manufacturing (Optional)

```bash
# Create Bill of Materials
POST /api/v1/manufacturing/boms
{
  "product_id": "prod_123",
  "components": [
    {
      "material_id": "mat_456",
      "quantity": 2.5,
      "unit": "kg"
    }
  ]
}

# Create Work Order
POST /api/v1/work-orders
{
  "product_id": "prod_123",
  "quantity": 100,
  "due_date": "2024-02-01",
  "bom_id": "bom_789"
}

# Update Work Order Status
PATCH /api/v1/work-orders/{work_order_id}
{
  "status": "in_progress",
  "quantity_completed": 25
}
```

---

## Common HTTP Headers

```bash
# Authentication (JWT Token)
Authorization: Bearer eyJhbGciOiJIUzI1NiIs...

# Authentication (API Key)
X-API-Key: sk_live_abc123...

# Content Type
Content-Type: application/json

# Idempotency Key (for safe retries)
Idempotency-Key: unique-request-id-123
```

---

## Response Codes

| Code | Meaning | Action |
|------|---------|--------|
| 200 | Success | Request completed successfully |
| 201 | Created | Resource created successfully |
| 400 | Bad Request | Check request body/parameters |
| 401 | Unauthorized | Check authentication token |
| 403 | Forbidden | Check permissions |
| 404 | Not Found | Resource doesn't exist |
| 409 | Conflict | Resource already exists |
| 422 | Validation Error | Check validation errors in response |
| 429 | Rate Limited | Slow down requests |
| 500 | Server Error | Contact support if persistent |

---

## Environment Variables

```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost/stateset
DATABASE_MAX_CONNECTIONS=100

# Redis (Optional)
REDIS_URL=redis://localhost:6379
REDIS_MAX_CONNECTIONS=50

# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# Security
JWT_SECRET=your-secret-key-min-32-chars
JWT_EXPIRATION=3600
ALLOWED_ORIGINS=https://yourstore.com

# Stripe
STRIPE_SECRET_KEY=sk_live_xxx
STRIPE_WEBHOOK_SECRET=whsec_xxx

# StablePay (Crypto)
STABLEPAY_API_KEY=sp_live_xxx
STABLEPAY_WEBHOOK_SECRET=whsec_yyy

# Shopify Integration
SHOPIFY_API_KEY=your-key
SHOPIFY_API_SECRET=your-secret
SHOPIFY_STORE_URL=yourstore.myshopify.com

# Email
SMTP_HOST=smtp.sendgrid.net
SMTP_PORT=587
SMTP_USERNAME=apikey
SMTP_PASSWORD=your-sendgrid-key
```

---

## CLI Tool Commands

```bash
# Install CLI
cargo install --path . --bin stateset-cli

# Test order creation
stateset-cli orders create \
  --customer-email customer@example.com \
  --product-id prod_123 \
  --quantity 2

# Check inventory
stateset-cli inventory check --product-id prod_123

# Process return
stateset-cli returns create --order-id ord_123

# Generate test data
stateset-cli test-data generate --orders 100

# Export data
stateset-cli export orders --format csv --output orders.csv
```

---

## Database Migrations

```bash
# Run all migrations
cargo run --bin migration -- up

# Rollback last migration
cargo run --bin migration -- down

# Check migration status
cargo run --bin migration -- status

# Create new migration
cargo run --bin migration -- create add_new_table
```

---

## Monitoring & Health

```bash
# Health Check
GET /health

# API Status
GET /status

# Prometheus Metrics
GET /metrics

# JSON Metrics
GET /metrics/json
```

---

## Rate Limits

| Tier | Requests/Minute | Burst |
|------|----------------|-------|
| Development | 60 | 10 |
| Production | 1000 | 100 |
| Enterprise | Unlimited | Unlimited |

---

## Webhook Events

Subscribe to these events:

- `order.created`
- `order.updated`
- `order.shipped`
- `order.delivered`
- `order.cancelled`
- `payment.succeeded`
- `payment.failed`
- `return.created`
- `return.approved`
- `return.received`
- `inventory.low_stock`
- `shipment.delivered`

Configure webhook URL in settings:
```bash
POST /api/v1/webhooks
{
  "url": "https://yourapp.com/webhooks/stateset",
  "events": ["order.created", "payment.succeeded"],
  "secret": "your-webhook-secret"
}
```

---

## Testing Tips

```bash
# Use test credit cards (Stripe)
4242 4242 4242 4242  # Visa
5555 5555 5555 4444  # Mastercard

# Use test mode API keys
STRIPE_SECRET_KEY=sk_test_xxx

# Enable debug logging
LOG_LEVEL=debug cargo run --bin stateset-api

# Run with test database
DATABASE_URL=sqlite::memory: cargo run --bin stateset-api
```

---

## Performance Benchmarks

Expected performance (per API instance):

- **Throughput**: 1000+ requests/second
- **Response Time**: < 100ms (median)
- **Order Processing**: 10,000+ orders/hour
- **Database Queries**: < 10ms (indexed)

Run benchmarks:
```bash
cargo run --bin orders-bench -- --duration 60s --concurrency 10
```

---

## Troubleshooting

```bash
# Check logs
docker logs stateset-api

# Verify database connection
psql $DATABASE_URL -c "SELECT 1"

# Test Redis connection
redis-cli -u $REDIS_URL ping

# Verify API is running
curl http://localhost:8080/health

# Check Stripe webhook
stripe listen --forward-to localhost:8080/api/v1/webhooks/stripe
```

---

## Support Resources

- **Swagger UI**: http://localhost:8080/swagger-ui
- **Documentation**: `/docs/` directory
- **Examples**: `/examples/` directory
- **Issues**: https://github.com/stateset/stateset-api/issues
- **Email**: support@stateset.io

---

## Quick Business Scenarios

### E-Commerce Checkout Flow
1. Create cart → 2. Add items → 3. Initiate checkout → 4. Set shipping → 5. Process payment → 6. Create order

### B2B Order Flow
1. Create customer → 2. Create purchase order → 3. Set Net 30 terms → 4. Generate invoice → 5. Track payment

### Returns Flow
1. Customer creates return → 2. Approve return → 3. Customer ships back → 4. Receive & inspect → 5. Restock → 6. Process refund

### Manufacturing Flow
1. Create BOM → 2. Create work order → 3. Allocate materials → 4. Track production → 5. Complete order → 6. Update inventory

---

**Stateset API v0.2.1** | Built with Rust | Production Ready

For complete documentation, see `/docs/GETTING_STARTED_BUSINESS.md`
