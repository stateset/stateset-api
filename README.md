# StateSet API

[![Production Ready](https://img.shields.io/badge/Production%20Ready-10%2F10-brightgreen.svg)](PRODUCTION_READY_REPORT.md)
[![Rust CI](https://github.com/stateset/stateset-api/actions/workflows/rust.yml/badge.svg)](https://github.com/stateset/stateset-api/actions/workflows/rust.yml)
[![Security Scan](https://github.com/stateset/stateset-api/actions/workflows/security.yml/badge.svg)](https://github.com/stateset/stateset-api/actions/workflows/security.yml)
[![codecov](https://codecov.io/gh/stateset/stateset-api/branch/master/graph/badge.svg)](https://codecov.io/gh/stateset/stateset-api)
[![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)
[![GitHub release](https://img.shields.io/github/v/release/stateset/stateset-api)](https://github.com/stateset/stateset-api/releases)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
[![code style: rustfmt](https://img.shields.io/badge/code%20style-rustfmt-orange.svg)](https://github.com/rust-lang/rustfmt)

StateSet API is a comprehensive, scalable, and robust backend system for order management, inventory control, returns processing, warranty management, shipment tracking, and work order handling. Built with Rust, it leverages modern web technologies and best practices to provide a high-performance, reliable solution for e-commerce and manufacturing businesses.

**Stats**: ~590 Rust source files • ~113,000 lines of code • 100% safe Rust

**Quick Links**: [Getting Started](#getting-started) | [Documentation](#documentation) | [API Endpoints](#api-endpoints) | [Deployment](docs/DEPLOYMENT.md) | [Contributing](CONTRIBUTING.md) | [Roadmap](ROADMAP.md)

## Production Readiness: 10/10

StateSet API has achieved **full production readiness** with comprehensive fixes and improvements:

- **Zero Compilation Errors** - Clean build with no warnings or errors
- **Zero Panic Risks** - All `.unwrap()` calls eliminated with proper error handling
- **Complete Security** - HMAC verification, input validation, RBAC, no SQL injection, constant-time comparisons
- **Configurable Deployment** - All hardcoded values externalized to configuration
- **Event-Driven Architecture** - Outbox pattern for reliable event processing
- **Comprehensive Testing** - Core business logic and security features validated
- **Full Observability** - Structured logging, Prometheus metrics, health checks
- **Production-Grade Error Handling** - Proper transaction boundaries and error propagation

**See**: [API Overview](API_OVERVIEW.md) | [Production Ready Report](PRODUCTION_READY_REPORT.md) | [Recent Improvements](IMPROVEMENTS_SUMMARY.md)

## Features

- **Order Management**:
  - Create, retrieve, update, and delete orders
  - Support for complex order workflows (hold, cancel, archive, merge)
  - Order item management and tracking
  - Fulfillment order creation and status updates

- **Inventory Control**: 
  - Real-time inventory tracking across multiple locations
  - Allocation, reservation, and release workflows
  - Lot tracking and cycle counting
  - Safety stock and reorder alerts

- **Returns Processing**: 
  - Streamlined return authorization and processing
  - Approval, rejection, and restocking workflows
  - Refund integration

- **Warranty Management**: 
  - Track and manage product warranties
  - Warranty claim processing with approval/rejection flows

- **Shipment Tracking**: 
  - Carrier assignment and tracking integration
  - Advanced shipping notice (ASN) creation and management
  - Delivery confirmation workflows

- **Manufacturing & Production**:
  - Bill of materials (BOM) creation and management
  - Work order scheduling and tracking
  - Component and raw material management
- **Financial Operations**:
  - Cash sale creation and tracking
  - Invoice generation with persistent storage
  - Payment processing with stored records
  - Item receipt recording for purchase orders

- **Supply Chain Management**:
  - Purchase order creation and tracking
  - Supplier management and relationships
  - Advanced shipping notice (ASN) processing
  - Quality control and maintenance workflows

- **Crypto Payments (StablePay)**:
  - Stablecoin payment processing
  - Crypto reconciliation and tracking
  - Multi-currency support

- **Analytics & Reporting**:
  - Business intelligence and metrics
  - Order and inventory analytics
  - Custom report generation

- **AI-Powered Commerce**:
  - Agentic Commerce Protocol support (separate server)
  - ChatGPT Instant Checkout integration
  - AI agents for commerce operations

## Tech Stack

Our carefully selected tech stack ensures high performance, scalability, and maintainability:

### Core Technologies
- **Language**: Rust (for performance, safety, and concurrency)
- **Web Framework**: [Axum](https://github.com/tokio-rs/axum/) (async web framework from the Tokio team)
- **Database**: SQLite (dev default) and PostgreSQL (prod) via [SeaORM](https://www.sea-ql.org/SeaORM) (async ORM)
- **Async Runtime**: Tokio (efficient async runtime for Rust)

### API Protocols
- **REST API**: Primary interface for client applications
- **gRPC**: Interface for service-to-service communication with Protocol Buffers

### Observability
- **Tracing**: OpenTelemetry integration for distributed request tracing
- **Health Checks**: Comprehensive service health monitoring
- **Error Handling**: Structured error system with detailed context

#### Metrics

The API exposes metrics at `/metrics` (text) and `/metrics/json` (JSON).

- Route-level metrics (via `metrics` crate):
  - `http_requests_total{method,route,status}`: Request counts per route.
  - `http_request_duration_ms{method,route,status}`: Request latency histogram (ms).
  - `rate_limit_denied_total{key_type,path}` / `rate_limit_allowed_total{key_type,path}`.
  - `auth_failures_total{code,status}`.

- Aggregated metrics (custom registry also visible at `/metrics`):
  - Counters: `http_requests_total`, `errors_total`, `cache_hits_total`, `cache_misses_total`.
  - Histograms: `http_request_duration_seconds_count/sum`.
  - Business: `orders_created_total`, `returns_processed_total`, etc.

Standard response headers:
- `X-Request-Id`: Unique id for tracing.
- Rate limit: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset` (and RFC `RateLimit-*`).

## Project Structure

This is a Cargo workspace with multiple members:

```
stateset-api/
├── agentic_server/       # Standalone Agentic Commerce Protocol server
├── migrations/           # Database migrations (SeaORM)
├── simple_api/           # Lightweight API variant
├── proto/                # Protocol Buffer definitions (gRPC)
├── src/
│   ├── main.rs           # Server entry point
│   ├── lib.rs            # Library exports & route composition
│   ├── config.rs         # Application configuration
│   ├── bin/              # Binary executables (15+ targets)
│   │   ├── stateset-api  # Main HTTP/REST API server
│   │   ├── stateset-cli  # Command-line interface
│   │   ├── grpc-server   # gRPC service endpoint
│   │   └── ...           # Additional utilities and servers
│   ├── auth/             # Authentication & authorization (JWT, API keys, RBAC, MFA)
│   ├── commands/         # Command handlers (write operations, 27 modules)
│   ├── entities/         # Database entity definitions (SeaORM, 30+ entities)
│   ├── errors/           # Error types and handling
│   ├── events/           # Event definitions and processing (outbox pattern)
│   ├── handlers/         # HTTP request handlers (35+ modules)
│   │   ├── orders.rs     # Order CRUD & operations
│   │   ├── inventory.rs  # Inventory management
│   │   ├── commerce/     # E-commerce (products, carts, checkout, customers)
│   │   └── ...           # Returns, shipments, warranties, work orders, etc.
│   ├── models/           # Domain models (90+ types)
│   ├── queries/          # Query handlers (read operations)
│   ├── services/         # Business logic services (55+ services)
│   │   ├── commerce/     # Cart, checkout, product catalog, pricing
│   │   └── ...           # Orders, inventory, payments, analytics, etc.
│   ├── cache/            # Caching layer (Redis)
│   ├── rate_limiter/     # Rate limiting middleware
│   ├── webhooks/         # Webhook management
│   ├── health/           # Health check endpoints
│   ├── metrics/          # Prometheus metrics
│   └── tracing/          # Distributed tracing (OpenTelemetry)
├── tests/                # Integration tests
├── benches/              # Performance benchmarks
├── docs/                 # Comprehensive documentation
├── config/               # Configuration files (default.toml)
└── examples/             # Usage examples (cURL, JavaScript, Python)
```

## Getting Started

### Prerequisites

Ensure you have the following installed:
- Rust 1.75+ (latest stable recommended)
- Protocol Buffer compiler (for gRPC)

Note: The app defaults to SQLite for local development (via SeaORM). PostgreSQL is optional and can be enabled by changing configuration.

### Quick Install

1. Clone the repository:
   ```sh
   git clone https://github.com/stateset/stateset-api.git
   cd stateset-api
   ```

2. Configure the app (choose one):
   - Using config files (recommended): edit `config/default.toml` (already set to SQLite by default).
   - Using env overrides: set environment variables with the `APP__` prefix (e.g., `APP__DATABASE_URL`, `APP__HOST`, `APP__PORT`).

   Examples:
   - SQLite (default): `APP__DATABASE_URL=sqlite://stateset.db?mode=rwc`
   - PostgreSQL: `APP__DATABASE_URL=postgres://user:pass@localhost:5432/stateset`

3. Run database migrations:
   ```sh
   cargo run --bin migration
   ```

4. Build and run the project:
   ```sh
   cargo run
   ```

The API will be available at `http://localhost:8080`.
Requests to unknown routes return a JSON 404 response.

Docker: `docker-compose up -d` starts the API and Redis. Compose reads values from `.env` for container env, which is separate from the app's `APP__*` variables used by the config system.

### Multiple Binary Targets

The project includes 15+ binary targets for different use cases:

- **`stateset-api`** - Main HTTP/REST API server (default)
- **`stateset-cli`** - Command-line interface for orders, products, customers, etc.
- **`grpc-server`** - gRPC service endpoint
- **`simple-server`** - Minimal server variant
- **`migration`** - Database migration runner
- **`openapi-export`** - Export OpenAPI specification
- **`orders-bench`** - Performance benchmarking tool
- And more...

Run any binary with `cargo run --bin <binary-name>`. For example:
```sh
cargo run --bin grpc-server     # Start gRPC server
cargo run --bin stateset-cli -- --help  # CLI help
```

## Stateset CLI

This repository also provides a CLI binary (`stateset-cli`) that reuses the same service layer as the HTTP API. It is ideal for local development, quick smoke tests, or scripting operational tasks without crafting raw REST requests.

### Build or Run the CLI

```sh
# Build once
cargo build --bin stateset-cli

# Or run ad hoc with arguments
cargo run --bin stateset-cli -- --help
```

By default the CLI reads configuration exactly like the server (via `AppConfig`). Set the usual `APP__*` environment variables (e.g. `APP__DATABASE_URL`) before running it.

### Authentication Helpers

```sh
# Login and persist tokens to ~/.stateset/session.json
stateset-cli auth login \
  --email admin@stateset.com \
  --password "secret" \
  --save

# Refresh saved tokens or inspect claims
stateset-cli auth refresh --save
stateset-cli auth whoami --include-refresh

# Revoke tokens (and optionally clear the session file)
stateset-cli auth logout --clear
```

The session file defaults to `~/.stateset/session.json`. Override with `STATESET_CLI_HOME=/custom/path.json`.

### Orders

```sh
# Create an order with two line items
stateset-cli orders create \
  --customer-id 2ddfeabd-0e6b-47f1-b63b-e1d755d094d6 \
  --item sku=SKU-123,quantity=2,price=19.99 \
  --item sku=SKU-456,quantity=1,price=49.00

# Explore and manage orders
stateset-cli orders list --page 1 --per-page 20 --status pending
stateset-cli orders update-status --order-id <uuid> --status shipped --notes "Label printed"
stateset-cli orders delete --order-id <uuid>
stateset-cli orders items --id <uuid> --json
```

### Products

```sh
# Create or update catalog entries
stateset-cli products create \
  --name "Widget" \
  --sku WIDGET-001 \
  --price 25.00 \
  --brand "Stateset"

stateset-cli products update \
  --id <uuid> \
  --price 27.50 \
  --deactivate

# Add variants and search the catalog
stateset-cli products create-variant \
  --product-id <uuid> \
  --sku WIDGET-001-RED-S \
  --name "Widget / Red / Small" \
  --price 27.50 \
  --option color=Red \
  --option size=Small

stateset-cli products search --query widget --limit 10 --json
stateset-cli products variants --product-id <uuid>
```

### Customers

```sh
# Register, authenticate, and browse customers
stateset-cli customers create \
  --email customer@example.com \
  --password "P@ssw0rd!" \
  --first-name Ada \
  --last-name Lovelace

stateset-cli customers login \
  --email customer@example.com \
  --password "P@ssw0rd!" \
  --save

stateset-cli customers list --search "@stateset.com" --limit 25
stateset-cli customers get --id <uuid> --json

# Manage addresses
stateset-cli customers add-address \
  --customer-id <uuid> \
  --first-name Ada \
  --last-name Lovelace \
  --address-line-1 "1 Infinite Loop" \
  --city Cupertino \
  --province CA \
  --country-code US \
  --postal-code 95014 \
  --default-shipping

stateset-cli customers addresses --id <uuid> --json
```

### Tips

- Append `--json` to any subcommand for machine-readable pretty output.
- Validation mirrors the API: expect the same error messages and permission requirements.
- Because the CLI instantiates the full service stack, ensure Redis/database dependencies are reachable when commands require them (orders, inventory, etc.).

## Agentic Commerce Server

The repository includes a standalone **Agentic Commerce Server** (`agentic_server/`) that implements OpenAI's Agentic Commerce Protocol for ChatGPT Instant Checkout.

### Features
- ✅ Full Agentic Checkout Spec compliance (5 required endpoints)
- ✅ Delegated Payment Spec support with mock PSP
- ✅ Lightweight and fast (~1,700 lines, <100ms response times)
- ✅ No database required - fully standalone with in-memory sessions
- ✅ Production-ready with structured logging, CORS, and compression
- ✅ Single-use vault tokens with allowance validation

### Quick Start

```sh
cd agentic_server
cargo build --release
cargo run --release
# Server starts on http://0.0.0.0:8080
```

The Agentic Commerce Server enables end-to-end checkout flows inside ChatGPT while keeping orders, payments, and compliance on your existing commerce stack. See [agentic_server/README.md](agentic_server/README.md) for full documentation.

## Continuous Integration & Quality Gates

StateSet API ships with GitHub Actions workflows that enforce quality gates:

- `Rust CI` enforces formatting (`cargo fmt --all -- --check`), linting (`cargo clippy -- -D warnings`), compilation, and tests for every push or pull request targeting `main` or `master`.
- `Build with Error Logging` captures detailed logs while re-running the same checks, keeping artifacts for quick diagnosis and commenting on pull requests when failures occur.
- `Dependency Audit` runs [`cargo deny`](https://github.com/EmbarkStudios/cargo-deny) on pushes, pull requests, and a weekly schedule to flag vulnerable, unlicensed, or banned dependencies.
- Dependabot opens weekly update PRs for Cargo crates and GitHub Actions to keep the stack current and secure.

Before pushing changes run the same commands locally:

```sh
cargo fmt --all -- --check
cargo clippy -- -D warnings
cargo test
# Optional: cargo deny check advisories licenses bans sources
```

## API Endpoints

StateSet API provides a rich set of RESTful endpoints:

### Authentication (`/auth`)
- `POST /auth/login` - Authenticate user and get JWT access + refresh tokens
- `POST /auth/refresh` - Refresh JWT tokens
- `POST /auth/logout` - Revoke the current access token and associated refresh tokens
- `POST /auth/api-keys` - Issue a new API key (requires `api-keys:create`)

#### Auth and Permissions
- Use `POST /auth/login` with `{ email, password }` to obtain a JWT `access_token` and `refresh_token`.
- Use `POST /auth/refresh` with `{ refresh_token }` to rotate tokens when the access token expires.
- Send `Authorization: Bearer <access_token>` on protected routes. API keys are also supported via `X-API-Key`.
- Endpoints are permission-gated (e.g., `orders:read`, `orders:create`). Admins have full access.
- Standard error responses include JSON with `error.code` and HTTP status; `X-Request-Id` is included on responses for tracing.

### Orders (`/api/v1/orders`)
- `GET /orders` - List all orders (paginated, filterable)
- `GET /orders/{id}` - Get order details by ID
- `GET /orders/by-number/{order_number}` - Get order by order number
- `GET /orders/{id}/items` - Get order line items
- `POST /orders` - Create a new order
- `PUT /orders/{id}` - Update an order
- `POST /orders/{id}/items` - Add item to order
- `PUT /orders/{id}/status` - Update order status
- `POST /orders/{id}/cancel` - Cancel an order
- `POST /orders/{id}/archive` - Archive an order
- `DELETE /orders/{id}` - Delete an order

### Inventory (`/api/v1/inventory`)
- `GET /inventory` - List all inventory items (paginated)
- `GET /inventory/{id}` - Get inventory item details
- `GET /inventory/low-stock` - Get low-stock items
- `POST /inventory` - Create inventory item
- `PUT /inventory/{id}` - Update inventory item
- `DELETE /inventory/{id}` - Delete inventory item
- `POST /inventory/{id}/reserve` - Reserve inventory for order
- `POST /inventory/{id}/release` - Release inventory reservation
- `POST /inventory/bulk-adjust` - Bulk adjust quantities (recount, damage, loss, etc.)
- `GET /inventory/reservations` - List all reservations
- `GET /inventory/reservations/{id}` - Get reservation details
- `GET /inventory/reservations/stats` - Get reservation statistics
- `POST /inventory/reservations/{id}/cancel` - Cancel a reservation
- `POST /inventory/reservations/cleanup` - Cleanup expired reservations

### Returns (`/api/v1/returns`)
- `GET /returns` - List all returns (paginated)
- `GET /returns/{id}` - Get return details
- `POST /returns` - Create a return request
- `POST /returns/{id}/approve` - Approve a return
- `POST /returns/{id}/restock` - Mark returned items as restocked

### Shipments (`/api/v1/shipments`)
- `GET /shipments` - List all shipments
- `GET /shipments/{id}` - Get shipment details
- `GET /shipments/{id}/track` - Track shipment by ID
- `GET /shipments/track/{tracking_number}` - Track by tracking number
- `POST /shipments` - Create new shipment
- `POST /shipments/{id}/ship` - Mark shipment as shipped
- `POST /shipments/{id}/deliver` - Mark shipment as delivered

### Warranties (`/api/v1/warranties`)
- `GET /warranties` - List all warranties
- `GET /warranties/{id}` - Get warranty details
- `POST /warranties` - Create a warranty
- `POST /warranties/{id}/extend` - Extend warranty period
- `POST /warranties/claims` - Submit a warranty claim
- `POST /warranties/claims/{id}/approve` - Approve a warranty claim

### Work Orders (`/api/v1/work-orders`)
- `GET /work-orders` - List all work orders
- `GET /work-orders/{id}` - Get work order details
- `POST /work-orders` - Create a work order
- `PUT /work-orders/{id}` - Update a work order
- `POST /work-orders/{id}/assign` - Assign work order to technician
- `POST /work-orders/{id}/complete` - Mark work order complete
- `PUT /work-orders/{id}/status` - Update work order status
- `DELETE /work-orders/{id}` - Delete a work order

### Bill of Materials (`/api/v1/manufacturing/boms`)
- `GET /manufacturing/boms` - List all BOMs
- `GET /manufacturing/boms/{id}` - Get BOM details
- `POST /manufacturing/boms` - Create a bill of materials
- `PUT /manufacturing/boms/{id}` - Update a BOM
- `POST /manufacturing/boms/{id}/components` - Add components to BOM
- `DELETE /manufacturing/boms/{id}/components` - Remove components from BOM
- `GET /manufacturing/boms/{id}/components` - Get BOM components
- `POST /manufacturing/boms/{id}/audit` - Audit BOM changes

### Purchase Orders (`/api/v1/purchase-orders`)
- `GET /purchase-orders` - List purchase orders
- `GET /purchase-orders/{id}` - Get purchase order details
- `POST /purchase-orders` - Create a purchase order
- `PUT /purchase-orders/{id}` - Update a purchase order
- `POST /purchase-orders/{id}/receive` - Record receipt of goods

### Advanced Shipping Notice (`/api/v1/asns`)
- `GET /asns` - List all ASNs
- `GET /asns/{id}` - Get ASN details
- `POST /asns` - Create an ASN
- `PUT /asns/{id}` - Update an ASN
- `POST /asns/{id}/mark-in-transit` - Mark ASN as in transit
- `POST /asns/{id}/mark-delivered` - Mark ASN as delivered
- `POST /asns/{id}/hold` - Place ASN on hold
- `POST /asns/{id}/release` - Release ASN from hold
- `DELETE /asns/{id}` - Cancel an ASN
- `POST /asns/{id}/items` - Add item to ASN
- `DELETE /asns/{id}/items/{item_id}` - Remove item from ASN
- `GET /asns/by-supplier/{supplier_id}` - Get ASNs by supplier
- `GET /asns/by-status/{status}` - Get ASNs by status
- `GET /asns/by-delivery-date` - Get ASNs by delivery date

### Products (`/api/v1/products`)
- `GET /products` - List products (searchable, filterable, paginated)
- `GET /products/{id}` - Get product details
- `POST /products` - Create a product
- `PUT /products/{id}` - Update a product
- `GET /products/{id}/variants` - Get product variants
- `POST /products/{id}/variants` - Add product variant

### Shopping Carts (`/api/v1/carts`)
- `POST /carts` - Create a shopping cart
- `GET /carts/{id}` - Get cart details
- `POST /carts/{id}/items` - Add item to cart
- `PUT /carts/{id}/items/{item_id}` - Update cart item quantity
- `DELETE /carts/{id}/items/{item_id}` - Remove item from cart

### Checkout (`/api/v1/checkout`)
- `POST /checkout` - Initiate checkout process

### Customers (`/api/v1/customers`)
- `GET /customers` - List customers (searchable)
- `GET /customers/{id}` - Get customer details
- `POST /customers` - Create customer account
- `PUT /customers/{id}` - Update customer
- `POST /customers/{id}/addresses` - Add customer address
- `GET /customers/{id}/addresses` - List customer addresses

### Payments (`/api/v1/payments`)
- `POST /payments` - Create a payment
- `GET /payments/{id}` - Get payment status
- `POST /payments/refund` - Refund a payment
- `POST /payments/webhook` - Process payment webhook (signature-verified, no auth required)

### Agents API (`/api/v1/agents`)
- `GET /agents/recommendations` - Get product recommendations
- `POST /agents/customers/{customer_id}/carts/{cart_id}/items` - Agent adds item to customer's cart

### Agentic Checkout (`/api/v1/agentic-checkout`)
- `POST /agentic-checkout/sessions` - Create checkout session (ChatGPT integration)
- `GET /agentic-checkout/sessions/{id}` - Get session details
- `POST /agentic-checkout/sessions/{id}/submit` - Submit order from checkout
- `POST /agentic-checkout/sessions/{id}/payment` - Handle payment
- `GET /agentic-checkout/order-status/{id}` - Get order status

### Analytics (`/api/v1/analytics`)
- `GET /analytics/dashboard` - Get dashboard metrics
- `GET /analytics/sales/trends` - Get sales trends
- `GET /analytics/sales/metrics` - Get sales metrics
- `GET /analytics/inventory` - Get inventory analytics
- `GET /analytics/shipments` - Get shipment metrics
- `GET /analytics/carts` - Get cart analytics

### Admin (`/api/v1/admin`)
- `GET /admin/outbox` - List outbox events (requires `admin:outbox`)
- `POST /admin/outbox/{id}/retry` - Retry failed event
- `DELETE /admin/outbox/{id}` - Delete outbox event

### Health & Metrics
- `GET /api/v1/status` - API status and version information
- `GET /api/v1/health` - Health check (DB, Redis, cache status)
- `GET /metrics` - Prometheus metrics (text format)
- `GET /metrics/json` - Metrics in JSON format

## Testing

Run the test suite with:

```sh
# Run all tests
cargo test

# Run integration tests
cargo test --features integration

# Run a specific test with backtrace
RUST_BACKTRACE=1 cargo test test_name
```

## Development Tools

- **Linting**: `cargo clippy`
- **Formatting**: `cargo fmt`
- **Documentation**: `cargo doc --open`

## Error Handling

StateSet API uses a structured error system with detailed context. API errors are returned as:

```json
{
  "error": {
    "code": "ORDER_NOT_FOUND",
    "message": "The requested order could not be found",
    "status": 404,
    "details": { "order_id": "123" }
  }
}
```

## Performance Considerations

- The API is designed for high throughput and low latency
- Connection pooling is used for database operations
- Async/await patterns are used throughout for non-blocking I/O
- Entity caching is implemented for frequently accessed data

## Idempotency and Rate Limiting

- Idempotency: Mutating endpoints (POST/PUT/PATCH/DELETE) support `Idempotency-Key` headers. When provided, the API ensures each unique key is processed once per route+method and caches the response for 10 minutes (Redis-backed).
- Rate Limiting: A global rate limiter is enforced with optional per-path, per‑API key, and per‑user policies. Standard headers (`X-RateLimit-*`, `RateLimit-*`) are included when enabled.

Environment variables:
- `APP__RATE_LIMIT_REQUESTS_PER_WINDOW` / `APP__RATE_LIMIT_WINDOW_SECONDS`
- `APP__RATE_LIMIT_ENABLE_HEADERS=true|false`
- `APP__RATE_LIMIT_PATH_POLICIES="/api/v1/orders:60:60,/api/v1/inventory:120:60"`
- `APP__RATE_LIMIT_API_KEY_POLICIES="sk_live_abc:200:60"`
- `APP__RATE_LIMIT_USER_POLICIES="user-123:500:60"`

## Documentation

### 📚 Complete Documentation Suite

**Public docs**: The hosted, end‑user API docs live at https://docs.stateset.com (this repo’s `/docs` folder contains deeper implementation and ops material).

**Start Here:**
- **[Business Getting Started Guide](docs/GETTING_STARTED_BUSINESS.md)** 🚀 - Complete guide for businesses launching ecommerce on Stateset
- **[Business Quick Reference Card](docs/BUSINESS_QUICK_REFERENCE.md)** ⚡ - One-page cheat sheet with all essential commands and endpoints
- **[API Overview](API_OVERVIEW.md)** 🎯 - Comprehensive platform overview with architecture, capabilities, and production readiness details
- **[Production Ready Report](PRODUCTION_READY_REPORT.md)** ✅ - 10/10 production readiness achievements and deployment checklist
- **[Quick Start Guide](docs/QUICK_START.md)** ⚡ - Technical quick start - get running in 5 minutes
- **[Documentation Index](docs/DOCUMENTATION_INDEX.md)** 🗂️ - Complete documentation map
- **[FAQ](docs/FAQ.md)** ❓ - Frequently asked questions

**Core Guides:**
- **[Architecture Documentation](docs/ARCHITECTURE.md)** 🏗️ - Comprehensive system architecture with 6 detailed diagrams
- **[API Overview](docs/API_OVERVIEW.md)** 📖 - Complete API reference with architecture, capabilities, and data models
- **[Use Cases](docs/USE_CASES.md)** 💡 - Real-world scenarios (e-commerce, B2B, manufacturing, AI shopping, crypto)
- **[Integration Guide](docs/INTEGRATION_GUIDE.md)** 🔧 - Production-ready integration patterns
- **[Best Practices](docs/BEST_PRACTICES.md)** ✨ - Patterns and anti-patterns for production use
- **[Troubleshooting](docs/TROUBLESHOOTING.md)** 🔍 - Common issues, solutions, and error codes
- **[Performance Tuning](docs/PERFORMANCE_TUNING.md)** 🚀 - Optimization guide for scale
- **[2025 Improvements](docs/API_IMPROVEMENTS_2025.md)** 📈 - Recent improvements: test coverage, code quality, architecture diagrams
- **[Recent Improvements Summary](IMPROVEMENTS_SUMMARY.md)** 📊 - Latest fixes and enhancements to reach 10/10

**Deployment & Operations:**
- **[Getting Started](GETTING_STARTED.md)** - Initial setup guide
- **[Deployment Guide](docs/DEPLOYMENT.md)** - Production deployment instructions
- **[Database Guide](docs/DATABASE.md)** - Database management and migrations
- **[Monitoring Guide](docs/MONITORING.md)** - Observability and alerting

**Additional Resources:**
- **[API Examples](examples/)** 💻 - Code examples in cURL, JavaScript, and Python
- **[API Versioning](API_VERSIONING.md)** - API versioning strategy
- **[Security Policy](SECURITY.md)** - Security guidelines and reporting
- **[Roadmap](ROADMAP.md)** - Feature roadmap and planning
- **[Changelog](CHANGELOG.md)** - Release history and changes

### Related Projects

- [stateset-sync-server](https://github.com/stateset/stateset-sync-server) - Multi‑tenant orchestration and integrations.
- [stateset-agents](https://github.com/stateset/stateset-agents) - RL framework for training multi‑turn agents.
- [stateset-nsr](https://github.com/stateset/stateset-nsr) - Neuro‑symbolic reasoning engine for explainable automation.
- [core](https://github.com/stateset/core) - Cosmos SDK blockchain for on‑chain commerce.

## Performance

- **Benchmarks**: Run `cargo bench` to see performance benchmarks
- **Load Testing**: Comprehensive load tests included in `tests/load_test.rs`
- **Metrics**: Prometheus metrics available at `/metrics`
- **Tracing**: OpenTelemetry support for distributed tracing

## Community

- **GitHub Discussions**: [Ask questions and share ideas](https://github.com/stateset/stateset-api/discussions)
- **Issue Tracker**: [Report bugs and request features](https://github.com/stateset/stateset-api/issues)
- **Code of Conduct**: [Our commitment to a welcoming community](CODE_OF_CONDUCT.md)

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

Before contributing:
1. Check existing issues or create a new one
2. Fork the repository
3. Create a feature branch
4. Make your changes with tests
5. Submit a pull request

## Security

Security is a top priority. If you discover a security vulnerability, please follow our [Security Policy](SECURITY.md).

**Security Features**:
- `#![forbid(unsafe_code)]` - Memory safe by design
- JWT authentication with refresh tokens
- Role-based access control (RBAC)
- API key management
- Rate limiting
- Automated security scanning
- Dependency auditing

## License

This project is source-available under the Business Source License 1.1 (BSL 1.1). See the [LICENSE](LICENSE) file for terms and the Change Date when it converts to Apache 2.0.

## Acknowledgments

Built with these amazing open-source projects:
- [Rust](https://www.rust-lang.org/) - Systems programming language
- [Axum](https://github.com/tokio-rs/axum) - Web framework
- [SeaORM](https://www.sea-ql.org/SeaORM/) - Async ORM
- [Tokio](https://tokio.rs/) - Async runtime
- [Tower](https://github.com/tower-rs/tower) - Service middleware

## Support

- **Documentation**: https://docs.stateset.com
- **Email**: support@stateset.io
- **GitHub Issues**: https://github.com/stateset/stateset-api/issues

---

**⭐ Star us on GitHub** — it motivates us a lot!

Made with ❤️ by the StateSet team
