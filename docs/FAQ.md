# StateSet API - Frequently Asked Questions

## General Questions

### What is StateSet API?

StateSet API is a comprehensive, production-ready backend system built in Rust for e-commerce, supply chain management, and manufacturing operations. It provides REST and gRPC interfaces for managing orders, inventory, returns, shipments, and more.

**Key features:**
- Complete order management system (OMS)
- Multi-location inventory management
- Returns processing (RMS)
- Multi-carrier shipment tracking
- E-commerce platform (products, cart, checkout)
- AI-powered shopping (ChatGPT integration)
- Manufacturing operations (BOMs, work orders)
- Crypto payment support (StablePay)
- Enterprise-grade security and scalability

### Why Rust?

We chose Rust for StateSet API because it provides:
- **Memory safety** without garbage collection
- **High performance** comparable to C/C++
- **Concurrency safety** preventing data races
- **Zero-cost abstractions** for maintainable, fast code
- **Strong type system** catching bugs at compile time
- **Excellent async/await** support via Tokio

Real-world impact:
- Sub-100ms response times
- 1000+ requests per second
- Memory-safe (no segfaults, buffer overflows)
- Reliable (no null pointer exceptions)

### Is StateSet API open source?

Yes! StateSet API is MIT licensed. You can:
- ✅ Use it commercially
- ✅ Modify the source code
- ✅ Distribute your modifications
- ✅ Use it in proprietary software

See [LICENSE](../LICENSE) for details.

### What databases are supported?

**Primary support:**
- **PostgreSQL** (recommended for production)
- **SQLite** (perfect for development and testing)

**Why PostgreSQL for production?**
- ACID compliance
- Excellent concurrency
- Full-text search
- JSON support
- Battle-tested at scale

**When to use SQLite?**
- Local development
- Testing
- Single-user applications
- Embedded use cases

Configuration:
```bash
# PostgreSQL
export APP__DATABASE_URL="postgres://user:pass@localhost:5432/stateset"

# SQLite
export APP__DATABASE_URL="sqlite://stateset.db?mode=rwc"
```

### Do I need Redis?

**Redis is optional but recommended** for:
- **Session caching** - Faster user session lookups
- **Entity caching** - Reduce database load
- **Rate limiting** - Distributed rate limit tracking
- **Idempotency** - Request deduplication
- **Webhook queues** - Reliable event delivery

**Without Redis:**
- In-memory caching (single instance only)
- Database-backed rate limiting (slower)
- Limited to single server instance

**With Redis:**
- Distributed caching across instances
- Horizontal scaling support
- Better performance under load

```bash
# Enable Redis
export APP__REDIS_URL="redis://localhost:6379"
```

---

## Getting Started

### How do I get started quickly?

Follow our [5-Minute Quick Start](./QUICK_START.md):

```bash
# 1. Clone and setup (1 min)
git clone https://github.com/stateset/stateset-api.git
cd stateset-api

# 2. Run migrations (30 sec)
cargo run --bin migration

# 3. Start server (30 sec)
cargo run

# 4. Try the API (1 min)
curl http://localhost:8080/health
```

See the [Quick Start Guide](./QUICK_START.md) for details.

### What are the system requirements?

**Minimum (Development):**
- Rust 1.88+
- 2 GB RAM
- 2 CPU cores
- 1 GB disk space

**Recommended (Production):**
- 4 GB+ RAM
- 4+ CPU cores
- 10 GB+ disk space
- PostgreSQL 12+
- Redis 6+
- Linux (Ubuntu 20.04+, Debian 11+, or similar)

**Cloud deployments:**
- AWS: t3.medium or larger
- GCP: e2-medium or larger
- Azure: B2s or larger

### How do I deploy to production?

See our comprehensive [Deployment Guide](./DEPLOYMENT.md) which covers:
- Docker deployment
- Kubernetes deployment
- Systemd service
- Environment configuration
- Database migrations
- Monitoring setup
- Security hardening

**Quick Docker deployment:**
```bash
docker-compose up -d
```

---

## API Usage

### How do I authenticate?

StateSet API supports **two authentication methods**:

**1. JWT Tokens (for users)**
```bash
# Register
curl -X POST http://localhost:8080/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{"email": "user@example.com", "password": "SecurePass123!"}'

# Login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email": "user@example.com", "password": "SecurePass123!"}'

# Use token
curl http://localhost:8080/api/v1/orders \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

**2. API Keys (for services)**
```bash
# Create API key (requires user JWT)
curl -X POST http://localhost:8080/api/v1/auth/api-keys \
  -H "Authorization: Bearer YOUR_JWT" \
  -d '{"name": "My Service", "permissions": ["orders:read"]}'

# Use API key
curl http://localhost:8080/api/v1/orders \
  -H "X-API-Key: YOUR_API_KEY"
```

See [Integration Guide - Authentication](./INTEGRATION_GUIDE.md#authentication-strategies) for details.

### What is idempotency and why should I use it?

**Idempotency ensures requests are processed only once**, even if submitted multiple times.

**Why it matters:**
- Network issues can cause duplicate requests
- Prevents duplicate charges
- Prevents duplicate orders
- Safe to retry failed requests

**How to use:**
```bash
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Idempotency-Key: unique-key-123" \
  -d '{...}'
```

**Key rules:**
- Same idempotency key + same request = cached response
- Different keys = different operations
- Keys expire after 10 minutes
- Use UUID or request-specific hash

See [Integration Guide - Idempotency](./INTEGRATION_GUIDE.md#idempotency-implementation).

### How do I handle pagination?

All list endpoints support pagination:

```bash
# Request
curl "http://localhost:8080/api/v1/orders?page=1&limit=20"

# Response
{
  "data": {
    "items": [...],
    "total": 150,
    "page": 1,
    "per_page": 20,
    "total_pages": 8
  }
}
```

**Parameters:**
- `page` - Page number (1-based, default: 1)
- `limit` or `per_page` - Items per page (default: 20, max: 100)

**Best practices:**
- Always paginate in production
- Use reasonable page sizes (20-50)
- Cache page results when appropriate
- Handle empty pages gracefully

### How do I filter and search?

Most endpoints support filtering:

```bash
# Filter orders by status
curl "http://localhost:8080/api/v1/orders?status=pending"

# Filter by date range
curl "http://localhost:8080/api/v1/orders?start_date=2025-01-01&end_date=2025-12-31"

# Filter by customer
curl "http://localhost:8080/api/v1/orders?customer_id=customer-uuid"

# Combine filters
curl "http://localhost:8080/api/v1/orders?status=pending&customer_id=customer-uuid&page=1&limit=20"

# Search products
curl "http://localhost:8080/api/v1/products/search?q=widget&category=Electronics"
```

**Common filters:**
- `status` - Filter by status
- `customer_id` - Filter by customer
- `start_date` / `end_date` - Date range
- `q` or `query` - Full-text search
- `category` - Filter by category
- `location_id` - Filter by location

### What are rate limits?

**Default rate limits:**
- 100 requests per minute (global)
- Custom limits per endpoint
- Custom limits per API key
- Custom limits per user

**Rate limit headers:**
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1699564800
```

**When rate limited (429):**
```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded",
    "retry_after": 60
  }
}
```

**Best practices:**
- Check `X-RateLimit-Remaining` header
- Implement exponential backoff
- Use webhooks instead of polling
- Request rate limit increase if needed

**Configure rate limits:**
```bash
export APP__RATE_LIMIT_REQUESTS_PER_WINDOW=1000
export APP__RATE_LIMIT_WINDOW_SECONDS=60
```

See [Integration Guide - Rate Limiting](./INTEGRATION_GUIDE.md#rate-limiting--throttling).

### How do webhooks work?

**Webhooks notify your application of events in real-time.**

**Setup:**
1. Create webhook endpoint (returns 200 OK)
2. Configure webhook URL in StateSet
3. Verify webhook signatures
4. Handle events

**Example webhook handler:**
```javascript
app.post('/webhooks/stateset', (req, res) => {
  // 1. Verify signature
  const signature = req.headers['x-stateset-signature'];
  if (!verifySignature(req.body, signature, secret)) {
    return res.status(401).send('Invalid signature');
  }

  // 2. Parse event
  const event = JSON.parse(req.body);

  // 3. Handle event
  switch (event.type) {
    case 'order.created':
      handleOrderCreated(event.data);
      break;
    case 'shipment.shipped':
      handleShipmentShipped(event.data);
      break;
  }

  // 4. Acknowledge receipt
  res.status(200).send('OK');
});
```

**Available events:**
- `order.created`
- `order.status_changed`
- `shipment.shipped`
- `shipment.delivered`
- `payment.processed`
- `payment.failed`
- `return.created`
- `return.approved`
- `inventory.low_stock`

See [Integration Guide - Webhooks](./INTEGRATION_GUIDE.md#webhook-integration).

---

## Features

### Can I use this for my e-commerce store?

**Absolutely!** StateSet API includes complete e-commerce functionality:

- ✅ Product catalog with variants
- ✅ Shopping cart management
- ✅ Complete checkout flow
- ✅ Customer accounts and profiles
- ✅ Multiple payment methods
- ✅ Order management
- ✅ Inventory tracking
- ✅ Shipment tracking
- ✅ Returns processing
- ✅ Analytics and reporting

See [Use Case - E-Commerce Store](./USE_CASES.md#e-commerce-store) for implementation guide.

### Does it support multi-location inventory?

**Yes!** StateSet supports:
- Multiple warehouses and stores
- Per-location inventory tracking
- Available-to-promise (ATP) calculations
- Inventory reservations and allocations
- Inventory transfers between locations
- Low stock alerts per location
- Lot tracking and expiration dates

**Example:**
```bash
# Get inventory for product at specific location
curl "http://localhost:8080/api/v1/inventory?product_id=...&location_id=..."

# Transfer inventory between locations
curl -X POST http://localhost:8080/api/v1/inventory/transfer \
  -d '{
    "from_location_id": "warehouse-1",
    "to_location_id": "store-1",
    "product_id": "...",
    "quantity": 50
  }'
```

See [Use Case - Omnichannel Retail](./USE_CASES.md#omnichannel-retail).

### Can I integrate with Shopify?

**Yes!** StateSet can act as an OMS (Order Management System) backend for Shopify:

**Integration flow:**
1. Sync products from StateSet to Shopify
2. Receive orders from Shopify via webhook
3. Process orders in StateSet
4. Update order status back to Shopify
5. Sync inventory levels bidirectionally

**Benefits:**
- Centralized inventory management
- Advanced fulfillment workflows
- Multi-channel order management
- Manufacturing integration
- Advanced analytics

See [Integration Guide - Shopify](./INTEGRATION_GUIDE.md#third-party-platform-integrations).

### What is Agentic Commerce?

**Agentic Commerce enables shopping entirely within ChatGPT** using natural language.

**How it works:**
1. Customer chats with ChatGPT: *"I need running shoes"*
2. ChatGPT searches StateSet product catalog
3. Customer selects product via conversation
4. ChatGPT handles checkout (address, shipping, payment)
5. Order created in StateSet
6. Customer receives confirmation in chat

**Features:**
- Natural language product search
- Conversational checkout
- Automatic tax and shipping calculation
- Secure delegated payment (PSP vault)
- Full OpenAI Agentic Commerce Protocol compliance

**Setup:**
See [Use Case - AI-Powered Shopping](./USE_CASES.md#ai-powered-shopping).

### Can I accept cryptocurrency payments?

**Yes!** StateSet includes **StablePay integration** for crypto payments:

**Supported currencies:**
- USDC (USD Coin)
- USDT (Tether)
- Other stablecoins

**Features:**
- Crypto-to-fiat conversion
- Blockchain transaction tracking
- Automatic payment confirmation
- Crypto refund processing
- Transaction reconciliation

**Example:**
```bash
# Create crypto payment
curl -X POST http://localhost:8080/api/v1/payments/crypto \
  -d '{
    "order_id": "...",
    "amount": 99.99,
    "currency": "USD",
    "crypto_currency": "USDC"
  }'

# Response includes payment address
{
  "payment_address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb",
  "crypto_amount": "100.00",
  "crypto_currency": "USDC",
  "network": "Ethereum",
  "expires_at": "2025-11-05T11:00:00Z"
}
```

See [Use Case - Crypto Commerce](./USE_CASES.md#crypto-commerce).

### Is there a GraphQL API?

**Not yet**, but it's on our [Roadmap](../ROADMAP.md).

Currently available:
- ✅ REST API (primary interface)
- ✅ gRPC API (service-to-service)
- 🔜 GraphQL API (planned)

**Why REST for now?**
- Simpler to get started
- Better caching
- Wide tooling support
- Swagger documentation

**Want GraphQL?** Let us know in [GitHub Discussions](https://github.com/stateset/stateset-api/discussions).

### Can I use this for B2B?

**Absolutely!** StateSet includes B2B features:

- ✅ Business customer accounts
- ✅ Custom pricing tiers
- ✅ Volume discounts
- ✅ Purchase orders (PO)
- ✅ Net payment terms (Net 30, Net 60)
- ✅ Credit limits
- ✅ Invoice generation
- ✅ Bulk ordering
- ✅ Multiple shipping addresses
- ✅ Account representatives

See [Use Case - B2B Wholesale](./USE_CASES.md#b2b-wholesale).

---

## Performance & Scalability

### How fast is StateSet API?

**Performance benchmarks:**
- **Response time**: <100ms for most endpoints
- **Throughput**: 1000+ requests/second
- **Concurrent connections**: 10,000+
- **Database queries**: <10ms with proper indexes

**Real-world performance:**
- Order creation: ~50ms
- Inventory check: ~20ms
- Product search: ~30ms
- List orders (paginated): ~40ms

**Optimization tips:**
- Use Redis caching
- Add database indexes
- Enable connection pooling
- Use pagination
- Implement client-side caching

See [API Overview - Performance](./API_OVERVIEW.md#performance--scalability).

### Can it scale horizontally?

**Yes!** StateSet API is designed for horizontal scaling:

**Stateless design:**
- No server-side sessions (JWT tokens)
- Shared cache (Redis)
- Shared database (PostgreSQL)
- Event-driven architecture

**Scaling strategy:**
```
Load Balancer
  ├── API Instance 1
  ├── API Instance 2
  ├── API Instance 3
  └── API Instance N
        ↓
  PostgreSQL (primary + replicas)
        ↓
  Redis (cluster)
```

**Kubernetes deployment:**
```yaml
apiVersion: apps/v1
kind: Deployment
spec:
  replicas: 5  # Scale to 5 instances
  selector:
    matchLabels:
      app: stateset-api
```

### What are the database requirements?

**Development:**
- SQLite is perfect
- No setup required
- Fast for local testing

**Production:**
- **PostgreSQL 12+** recommended
- **Minimum**: 2 CPU, 4 GB RAM, 50 GB storage
- **Recommended**: 4 CPU, 8 GB RAM, 100 GB storage
- **High-traffic**: 8+ CPU, 16+ GB RAM, 500+ GB storage

**Optimization:**
- Add indexes on frequently queried columns
- Use connection pooling (default: 20 connections)
- Set up read replicas for reporting
- Regular VACUUM and ANALYZE
- Monitor slow queries

**PostgreSQL settings:**
```sql
-- Increase work memory for complex queries
ALTER SYSTEM SET work_mem = '16MB';

-- Increase shared buffers
ALTER SYSTEM SET shared_buffers = '2GB';

-- Enable parallel queries
ALTER SYSTEM SET max_parallel_workers_per_gather = 4;
```

---

## Security

### Is StateSet API secure?

**Yes!** StateSet implements industry-standard security practices:

**Security features:**
- ✅ Memory-safe (Rust, no unsafe code)
- ✅ JWT authentication with refresh tokens
- ✅ API key management with scoped permissions
- ✅ Role-based access control (RBAC)
- ✅ Password hashing (Argon2)
- ✅ Rate limiting
- ✅ Idempotency for mutations
- ✅ Request signature verification
- ✅ SQL injection prevention (parameterized queries)
- ✅ XSS protection
- ✅ HTTPS enforcement
- ✅ CORS configuration
- ✅ Security headers
- ✅ Audit logging

**Security policy:**
See [SECURITY.md](../SECURITY.md) for vulnerability reporting.

### How are passwords stored?

Passwords are **never stored in plain text**.

**Password hashing:**
- Algorithm: **Argon2id** (winner of Password Hashing Competition)
- Memory-hard (resistant to GPU attacks)
- Includes random salt per password
- Configurable iterations

**On registration:**
```rust
let hashed_password = argon2::hash_encoded(
    password.as_bytes(),
    &salt,
    &config
)?;
```

**On login:**
```rust
let is_valid = argon2::verify_encoded(
    &stored_hash,
    password.as_bytes()
)?;
```

**Best practices:**
- Minimum password length: 8 characters
- Require mix of characters (letters, numbers, symbols)
- Rate limit login attempts
- Implement account lockout after failed attempts
- Support 2FA (planned)

### How do I secure my API keys?

**Best practices:**

1. **Never commit API keys to version control**
   ```bash
   # Add to .gitignore
   echo ".env" >> .gitignore
   echo "config/secrets.toml" >> .gitignore
   ```

2. **Use environment variables**
   ```bash
   export STATESET_API_KEY="sk_live_..."
   ```

3. **Use different keys per environment**
   ```bash
   # Development
   STATESET_API_KEY=sk_test_dev_...

   # Staging
   STATESET_API_KEY=sk_test_staging_...

   # Production
   STATESET_API_KEY=sk_live_...
   ```

4. **Rotate keys regularly**
   ```bash
   # Create new key
   ./target/debug/stateset-cli auth api-keys create --name "Production 2025-Q4"

   # Update applications to use new key
   # Revoke old key
   ./target/debug/stateset-cli auth api-keys revoke --id old-key-id
   ```

5. **Use scoped permissions**
   ```bash
   # Don't give full access
   # ❌ Bad
   permissions: ["*"]

   # ✅ Good
   permissions: ["orders:read", "orders:create"]
   ```

6. **Use secret management services**
   - AWS Secrets Manager
   - Google Secret Manager
   - HashiCorp Vault
   - Kubernetes Secrets

### Are there any security audits?

**Security practices:**
- Automated security scanning in CI/CD
- Dependency vulnerability scanning (cargo-deny)
- Dependabot for dependency updates
- Regular code reviews
- Community security reporting

**Third-party audit:** Not yet, but planned for 2025.

**Want to help?** See [SECURITY.md](../SECURITY.md) for responsible disclosure.

---

## Development

### Can I contribute?

**Yes! We welcome contributions!**

**How to contribute:**
1. Read [CONTRIBUTING.md](../CONTRIBUTING.md)
2. Check [good first issue](https://github.com/stateset/stateset-api/labels/good%20first%20issue) label
3. Fork the repository
4. Create a feature branch
5. Make your changes with tests
6. Submit a pull request

**Types of contributions:**
- 🐛 Bug fixes
- ✨ New features
- 📝 Documentation improvements
- 🧪 Test coverage
- 🎨 Code quality improvements
- 🌐 Translations
- 💡 Feature ideas

### How do I run tests?

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_create_order

# Run with output
cargo test -- --nocapture

# Run integration tests
cargo test --features integration

# Run with coverage
cargo install cargo-tarpaulin
cargo tarpaulin --out Html
```

**Test organization:**
```
tests/
├── unit/           # Unit tests
├── integration/    # Integration tests
└── common/         # Test utilities
```

### How do I add a new endpoint?

**Step-by-step:**

1. **Define handler** (`src/handlers/my_feature.rs`):
```rust
pub async fn create_widget(
    State(state): State<AppState>,
    Json(payload): Json<CreateWidgetRequest>,
) -> Result<Json<WidgetResponse>, ApiError> {
    // Implementation
}
```

2. **Add route** (`src/main.rs`):
```rust
let app = Router::new()
    .route("/api/v1/widgets", post(handlers::create_widget))
    .route("/api/v1/widgets/:id", get(handlers::get_widget));
```

3. **Add service logic** (`src/services/widget_service.rs`):
```rust
pub async fn create_widget(
    db: &DatabaseConnection,
    data: CreateWidgetRequest,
) -> Result<Widget, ServiceError> {
    // Business logic
}
```

4. **Add tests**:
```rust
#[tokio::test]
async fn test_create_widget() {
    // Test implementation
}
```

5. **Update documentation**

### What's the code structure?

```
stateset-api/
├── migrations/           # Database migrations
├── proto/                # gRPC protocol definitions
├── src/
│   ├── bin/              # Binary executables
│   │   ├── grpc_server.rs
│   │   └── stateset-cli/
│   ├── handlers/         # HTTP request handlers
│   ├── services/         # Business logic
│   ├── repositories/     # Data access layer
│   ├── entities/         # Database entities (SeaORM)
│   ├── models/           # Domain models
│   ├── commands/         # Write operations (CQRS)
│   ├── queries/          # Read operations (CQRS)
│   ├── events/           # Event definitions
│   ├── errors/           # Error types
│   ├── cache/            # Caching layer
│   ├── webhooks/         # Webhook handling
│   ├── config.rs         # Configuration
│   ├── lib.rs            # Library exports
│   └── main.rs           # Main entry point
├── tests/                # Integration tests
├── config/               # Configuration files
└── docs/                 # Documentation
```

**Architecture patterns:**
- **Layered architecture** (handlers → services → repositories)
- **CQRS** (separate read and write operations)
- **Event sourcing** (event outbox pattern)
- **Repository pattern** (data access abstraction)
- **Dependency injection** (via Axum state)

---

## Troubleshooting

### Where do I find error codes?

See [Troubleshooting Guide - Error Code Reference](./TROUBLESHOOTING.md#error-code-reference).

Common error codes:
- `INVALID_CREDENTIALS` (401) - Wrong email/password
- `TOKEN_EXPIRED` (401) - Access token expired
- `INSUFFICIENT_PERMISSIONS` (403) - Missing permission
- `NOT_FOUND` (404) - Resource doesn't exist
- `VALIDATION_ERROR` (400) - Invalid request data
- `RATE_LIMIT_EXCEEDED` (429) - Too many requests
- `INSUFFICIENT_INVENTORY` (422) - Not enough stock

### How do I debug issues?

**Enable debug logging:**
```bash
export RUST_LOG=debug
cargo run
```

**View structured logs:**
```bash
# JSON format
export LOG_FORMAT=json
cargo run | jq

# View specific request
grep "req-abc123" logs/stateset.log | jq
```

**Check database queries:**
```bash
export RUST_LOG=sqlx=debug
cargo run
```

**Use request IDs:**
Every API response includes `X-Request-Id` header for tracing.

**Get help:**
See [Troubleshooting Guide](./TROUBLESHOOTING.md) for common issues and solutions.

### Where can I get help?

1. **Documentation**: [DOCUMENTATION_INDEX.md](./DOCUMENTATION_INDEX.md)
2. **Troubleshooting**: [TROUBLESHOOTING.md](./TROUBLESHOOTING.md)
3. **Examples**: [examples/](../examples/)
4. **Search issues**: [GitHub Issues](https://github.com/stateset/stateset-api/issues)
5. **Ask community**: [GitHub Discussions](https://github.com/stateset/stateset-api/discussions)
6. **Email support**: support@stateset.io

**When asking for help, include:**
- Request ID
- Error message
- API endpoint and method
- Request payload (sanitized)
- Server version
- Environment (dev/staging/prod)

---

## Licensing & Commercial Use

### Can I use this commercially?

**Yes!** StateSet API is MIT licensed, which allows:
- ✅ Commercial use
- ✅ Modification
- ✅ Distribution
- ✅ Private use

**No restrictions on:**
- Number of users
- Revenue
- Industry
- Geographic location

See [LICENSE](../LICENSE) for full terms.

### Do I need to pay for support?

**No!** StateSet API is free and open source.

**Available for free:**
- Full source code
- Documentation
- Community support (GitHub Discussions)
- Bug reports and feature requests

**Premium support** (coming soon):
- Priority support
- SLA guarantees
- Private Slack channel
- Architecture consulting
- Custom development

Interested? Contact: support@stateset.io

### Can I contribute back?

**We encourage contributions!**

**Ways to contribute:**
- Fix bugs
- Add features
- Improve documentation
- Answer questions in Discussions
- Share your use case
- Star the repository ⭐

See [CONTRIBUTING.md](../CONTRIBUTING.md).

---

## Still have questions?

- **Browse Documentation**: [DOCUMENTATION_INDEX.md](./DOCUMENTATION_INDEX.md)
- **Ask the Community**: [GitHub Discussions](https://github.com/stateset/stateset-api/discussions)
- **Report Issues**: [GitHub Issues](https://github.com/stateset/stateset-api/issues)
- **Email Us**: support@stateset.io

**New to StateSet?** Start with the [Quick Start Guide](./QUICK_START.md)!

[← Back to Documentation Index](./DOCUMENTATION_INDEX.md)
