# StateSet API

[![Rust CI](https://github.com/stateset/stateset-api/actions/workflows/rust.yml/badge.svg)](https://github.com/stateset/stateset-api/actions/workflows/rust.yml)
[![Build with Error Logging](https://github.com/stateset/stateset-api/actions/workflows/build-with-logging.yml/badge.svg)](https://github.com/stateset/stateset-api/actions/workflows/build-with-logging.yml)

StateSet API is a comprehensive, scalable, and robust backend system for order management, inventory control, returns processing, warranty management, shipment tracking, and work order handling. Built with Rust, it leverages modern web technologies and best practices to provide a high-performance, reliable solution for e-commerce and manufacturing businesses.

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

## Tech Stack

Our carefully selected tech stack ensures high performance, scalability, and maintainability:

### Core Technologies
- **Language**: Rust (for performance, safety, and concurrency)
- **Web Framework**: [Axum](https://github.com/tokio-rs/axum/) (async web framework from the Tokio team)
- **Database**: PostgreSQL with [SeaORM](https://www.sea-ql.org/SeaORM) (async ORM)
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

```
stateset-api/
├── migrations/           # Database migrations
├── proto/                # Protocol Buffer definitions
├── src/
│   ├── bin/              # Binary executables
│   ├── commands/         # Command handlers (write operations)
│   ├── entities/         # Database entity definitions
│   ├── errors/           # Error types and handling
│   ├── events/           # Event definitions and processing
│   ├── handlers/         # HTTP request handlers
│   ├── models/           # Domain models
│   ├── queries/          # Query handlers (read operations)
│   ├── repositories/     # Data access layer
│   ├── services/         # Business logic services
│   └── config.rs         # Application configuration
└── tests/                # Integration tests
```

## Getting Started

### Prerequisites

Ensure you have the following installed:
- Rust (latest stable)
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

Docker: `docker-compose up -d` starts the API and Redis. Compose reads values from `.env` for container env, which is separate from the app’s `APP__*` variables used by the config system.

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

### Authentication
- `POST /auth/login` - Authenticate user and get JWT token
- `POST /auth/register` - Register a new user
- `POST /auth/logout` - Revoke the current access token and all associated refresh tokens
- `POST /auth/password/change` - Change the password for the authenticated user
- `POST /auth/password/reset/request` - Request a password reset token (delivered via email in production)
- `POST /auth/password/reset/confirm` - Complete a password reset with the issued token
- `GET /auth/api-keys` - List API keys for the authenticated user (requires `api-keys:read`)
- `POST /auth/api-keys` - Issue a new API key (requires `api-keys:create`)
- `DELETE /auth/api-keys/{id}` - Revoke an API key (requires `api-keys:delete`)

#### Auth and Permissions
- Use `POST /api/v1/auth/login` with `{ email, password }` to obtain a JWT `access_token` and `refresh_token`.
- Send `Authorization: Bearer <access_token>` on protected routes. API keys are also supported via `X-API-Key`.
- Endpoints are permission-gated (e.g., `orders:read`, `orders:create`). Admins have full access.
- Standard error responses include JSON with `error.code` and HTTP status; `X-Request-Id` is included on responses for tracing.

### Orders
- `GET /orders` - List all orders
- `GET /orders/:id` - Get order details
- `POST /orders` - Create a new order
- `PUT /orders/:id` - Update an order
- `POST /orders/:id/hold` - Place an order on hold
- `POST /orders/:id/cancel` - Cancel an order
- `POST /orders/:id/archive` - Archive an order

### Inventory
- `GET /inventory` - Get current inventory levels
- `POST /inventory/adjust` - Adjust inventory quantity
- `POST /inventory/allocate` - Allocate inventory
- `POST /inventory/reserve` - Reserve inventory
- `POST /inventory/release` - Release reserved inventory

### Returns
- `POST /returns` - Create a return request
- `GET /returns/:id` - Get return details
- `POST /returns/:id/approve` - Approve a return
- `POST /returns/:id/reject` - Reject a return
- `POST /returns/:id/restock` - Restock returned items

### Warranties
- `POST /warranties` - Create a warranty
- `POST /warranties/claim` - Submit a warranty claim
- `POST /warranties/claims/:id/approve` - Approve a warranty claim
- `POST /warranties/claims/:id/reject` - Reject a warranty claim

### Work Orders
- `POST /work-orders` - Create a work order
- `GET /work-orders/:id` - Get work order details
- `POST /work-orders/:id/start` - Start a work order
- `POST /work-orders/:id/complete` - Complete a work order

### Health
- `GET /health` - Basic health check
- `GET /health/readiness` - Database readiness check
- `GET /health/version` - Build and version information

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

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
