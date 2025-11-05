# StateSet API - 5-Minute Quick Start

Get up and running with StateSet API in 5 minutes or less.

## Prerequisites

- Rust 1.88+ installed
- 5 minutes of your time ⏱️

## Step 1: Clone & Setup (1 minute)

```bash
# Clone the repository
git clone https://github.com/stateset/stateset-api.git
cd stateset-api

# The API uses SQLite by default - no database setup needed!
```

## Step 2: Run Migrations (30 seconds)

```bash
# Create the database schema
cargo run --bin migration
```

Expected output:
```
✓ Database migrations completed successfully
```

## Step 3: Start the Server (30 seconds)

```bash
# Start the API server
cargo run
```

Expected output:
```
🚀 StateSet API starting...
📊 Database: Connected (SQLite)
🔧 Redis: Not configured (optional)
🌐 Server listening on http://0.0.0.0:8080
📚 API docs: http://localhost:8080/swagger-ui
✓ StateSet API ready!
```

**🎉 Your API is now running!**

## Step 4: Make Your First Request (1 minute)

Open a new terminal and try these commands:

### Check API Health

```bash
curl http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "version": "1.0.0",
  "database": "connected"
}
```

### Register a User

```bash
curl -X POST http://localhost:8080/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "demo@example.com",
    "password": "SecurePass123!",
    "name": "Demo User"
  }'
```

Response:
```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGc...",
    "refresh_token": "eyJhbGc...",
    "user": {
      "id": "...",
      "email": "demo@example.com",
      "name": "Demo User"
    }
  }
}
```

**Save your access token!** You'll need it for authenticated requests.

### Create Your First Order (2 minutes)

```bash
# Set your token
export TOKEN="your-access-token-here"

# Create a customer first
curl -X POST http://localhost:8080/api/v1/customers \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "email": "customer@example.com",
    "first_name": "Jane",
    "last_name": "Smith",
    "phone": "+1-555-0123"
  }'

# Save the customer ID from the response
export CUSTOMER_ID="customer-id-from-response"

# Create a product
curl -X POST http://localhost:8080/api/v1/products \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Sample Widget",
    "sku": "WIDGET-001",
    "price": 29.99,
    "currency": "USD",
    "inventory_quantity": 100
  }'

# Save the product ID from the response
export PRODUCT_ID="product-id-from-response"

# Create an order
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"customer_id\": \"$CUSTOMER_ID\",
    \"status\": \"pending\",
    \"items\": [{
      \"product_id\": \"$PRODUCT_ID\",
      \"sku\": \"WIDGET-001\",
      \"quantity\": 2,
      \"unit_price\": 29.99,
      \"name\": \"Sample Widget\"
    }],
    \"total_amount\": 59.98,
    \"currency\": \"USD\",
    \"shipping_address\": {
      \"street\": \"123 Main St\",
      \"city\": \"San Francisco\",
      \"state\": \"CA\",
      \"postal_code\": \"94105\",
      \"country\": \"US\"
    }
  }"
```

**🎊 Congratulations!** You just created your first order!

## Step 5: Explore the API (1 minute)

### Interactive Documentation

Open your browser and visit:
```
http://localhost:8080/swagger-ui
```

Here you can:
- ✅ Browse all 100+ endpoints
- ✅ Try API calls interactively
- ✅ See request/response schemas
- ✅ Test authentication

### Use the CLI

```bash
# Build the CLI tool
cargo build --bin stateset-cli

# Login
./target/debug/stateset-cli auth login \
  --email demo@example.com \
  --password SecurePass123! \
  --save

# List orders
./target/debug/stateset-cli orders list

# View order details
./target/debug/stateset-cli orders get --id $ORDER_ID --json
```

## Common Operations

### List Orders

```bash
curl "http://localhost:8080/api/v1/orders?page=1&limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

### Get Inventory

```bash
curl "http://localhost:8080/api/v1/inventory" \
  -H "Authorization: Bearer $TOKEN"
```

### Check Low Stock

```bash
curl "http://localhost:8080/api/v1/inventory/low-stock" \
  -H "Authorization: Bearer $TOKEN"
```

### View Analytics

```bash
curl "http://localhost:8080/api/v1/analytics/dashboard" \
  -H "Authorization: Bearer $TOKEN"
```

## Next Steps

Now that you're up and running, explore these resources:

### Learn More

- **[API Overview](./API_OVERVIEW.md)** - Understand all capabilities
- **[Use Cases](./USE_CASES.md)** - See real-world implementation scenarios
- **[Integration Guide](./INTEGRATION_GUIDE.md)** - Build production integrations
- **[API Examples](../examples/api-examples.md)** - Code examples in multiple languages

### Try Advanced Features

#### E-Commerce Checkout
```bash
# 1. Create a cart
# 2. Add items
# 3. Start checkout
# 4. Complete purchase
```
See: [E-Commerce Use Case](./USE_CASES.md#e-commerce-store)

#### Inventory Management
```bash
# Reserve inventory
curl -X POST "http://localhost:8080/api/v1/inventory/$INVENTORY_ID/reserve" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "quantity": 5,
    "order_id": "'$ORDER_ID'"
  }'
```

#### Process Returns
```bash
curl -X POST http://localhost:8080/api/v1/returns \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": "'$ORDER_ID'",
    "items": [{
      "order_item_id": "'$ITEM_ID'",
      "quantity": 1,
      "reason": "defective"
    }]
  }'
```

#### Track Shipments
```bash
curl -X POST http://localhost:8080/api/v1/shipments \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": "'$ORDER_ID'",
    "carrier": "UPS",
    "service_level": "ground"
  }'
```

## Configuration Options

### Switch to PostgreSQL

Edit `config/default.toml` or set environment variable:

```bash
export APP__DATABASE_URL="postgres://user:pass@localhost:5432/stateset"
cargo run --bin migration
cargo run
```

### Enable Redis (Optional)

```bash
# Start Redis
docker run -d -p 6379:6379 redis:alpine

# Configure in config/default.toml or:
export APP__REDIS_URL="redis://localhost:6379"
cargo run
```

### Enable Features

```toml
# config/default.toml
[features]
webhooks = true
rate_limiting = true
idempotency = true
metrics = true
tracing = true
```

## Development Tips

### Hot Reload with cargo-watch

```bash
cargo install cargo-watch
cargo watch -x run
```

### Run Tests

```bash
# All tests
cargo test

# Specific test
cargo test test_create_order

# With output
cargo test -- --nocapture
```

### Check Code Quality

```bash
# Format code
cargo fmt

# Lint
cargo clippy

# Check compilation
cargo check
```

### View Logs

```bash
# Set log level
export RUST_LOG=debug
cargo run

# Structured JSON logs
export LOG_FORMAT=json
cargo run
```

## Troubleshooting

### Port Already in Use

```bash
# Change port
export APP__PORT=8081
cargo run
```

### Database Migration Issues

```bash
# Reset database
rm stateset.db
cargo run --bin migration
```

### Permission Errors

```bash
# Check file permissions
ls -la stateset.db

# Make writable
chmod 644 stateset.db
```

### Build Errors

```bash
# Clean and rebuild
cargo clean
cargo build
```

## Production Deployment

When you're ready to deploy:

1. **Read**: [Deployment Guide](./DEPLOYMENT.md)
2. **Review**: [Production Checklist](./INTEGRATION_GUIDE.md#production-checklist)
3. **Set up**: [Monitoring](./MONITORING.md)

## Quick Reference Card

```bash
# Start server
cargo run

# Run migrations
cargo run --bin migration

# Run tests
cargo test

# CLI login
./target/debug/stateset-cli auth login --email user@example.com --password pass --save

# Create order (CLI)
./target/debug/stateset-cli orders create --customer-id <uuid> --item sku=ABC,quantity=2,price=29.99

# API health check
curl http://localhost:8080/health

# Interactive docs
open http://localhost:8080/swagger-ui

# View metrics
curl http://localhost:8080/metrics
```

## Help & Support

- **Documentation**: Start at [DOCUMENTATION_INDEX.md](./DOCUMENTATION_INDEX.md)
- **Examples**: Browse [examples/](../examples/) directory
- **Issues**: [GitHub Issues](https://github.com/stateset/stateset-api/issues)
- **Community**: [GitHub Discussions](https://github.com/stateset/stateset-api/discussions)

## What's Next?

Choose your path:

**🛍️ Building an E-Commerce Store?**
→ Read [E-Commerce Use Case](./USE_CASES.md#e-commerce-store)

**🏭 Manufacturing Operations?**
→ Read [Manufacturing Use Case](./USE_CASES.md#manufacturing--production)

**🤖 AI-Powered Shopping?**
→ Read [Agentic Commerce Guide](./USE_CASES.md#ai-powered-shopping)

**💰 Crypto Payments?**
→ Read [Crypto Commerce Guide](./USE_CASES.md#crypto-commerce)

**🔧 Building an Integration?**
→ Read [Integration Guide](./INTEGRATION_GUIDE.md)

---

**You're all set!** 🚀 Start building amazing commerce experiences with StateSet API.

Need help? Check the [Documentation Index](./DOCUMENTATION_INDEX.md) or join our [community discussions](https://github.com/stateset/stateset-api/discussions).
