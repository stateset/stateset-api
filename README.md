# StateSet API

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
- Rust (latest stable version)
- PostgreSQL 14+
- Protocol Buffer compiler (for gRPC)

### Quick Install

1. Clone the repository:
   ```sh
   git clone https://github.com/stateset/stateset-api.git
   cd stateset-api
   ```

2. Create a `.env` file with your configuration:
   ```sh
   DATABASE_URL=postgres://username:password@localhost/stateset
   SERVER_HOST=0.0.0.0
   SERVER_PORT=8080
   JWT_SECRET=your_jwt_secret
   ```

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

## API Endpoints

StateSet API provides a rich set of RESTful endpoints:

### Authentication
- `POST /auth/login` - Authenticate user and get JWT token
- `POST /auth/register` - Register a new user

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

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
