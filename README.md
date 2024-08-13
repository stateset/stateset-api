# StateSet API

StateSet API is a comprehensive, scalable, and robust backend system for order management, inventory control, returns processing, warranty management, shipment tracking, and work order handling. It's built with Rust, leveraging modern web technologies and best practices.

## Features

- **Order Management**: Create, retrieve, update, and delete orders
- **Inventory Control**: Real-time inventory tracking and management
- **Returns Processing**: Handle product returns efficiently
- **Warranty Management**: Manage product warranties and claims
- **Shipment Tracking**: Track and manage product shipments
- **Work Order Handling**: Create and manage work orders for repairs or modifications

## Tech Stack

- **Language**: Rust
- **Web Framework**: Actix-web
- **Database**: PostgreSQL (with SQLx for async operations)
- **Caching**: Redis
- **Message Queue**: RabbitMQ
- **API Protocols**: REST, GraphQL, gRPC
- **Metrics**: Prometheus
- **Tracing**: OpenTelemetry with Jaeger
- **Logging**: slog

## Architecture

The StateSet API follows a modular architecture with the following key components:

- **Services**: Core business logic implementation
- **Handlers**: HTTP request handlers
- **Commands**: Command pattern for write operations
- **Queries**: Query pattern for read operations
- **Events**: Event-driven architecture for asynchronous processing
- **Models**: Data models representing the domain entities
- **Middleware**: Auth, rate limiting, circuit breaking, etc.

## Getting Started

### Prerequisites

- Rust (latest stable version)
- PostgreSQL
- Redis
- RabbitMQ
- Jaeger (for distributed tracing)

### Installation

1. Clone the repository:
   ```
   git clone https://github.com/yourusername/stateset-api.git
   cd stateset-api
   ```

2. Set up the environment variables:
   ```
   cp .env.example .env
   ```
   Edit the `.env` file with your configuration details.

3. Build the project:
   ```
   cargo build
   ```

4. Run the migrations:
   ```
   cargo run --bin migrate
   ```

5. Start the server:
   ```
   cargo run
   ```

The API will be available at `http://localhost:8080` by default.

## API Documentation

API documentation is available at `/docs` when running the server in development mode.

## Testing

Run the test suite with:

```
cargo test
```

## Deployment

The application is containerized and can be deployed using Docker:

```
docker build -t stateset-api .
docker run -p 8080:8080 stateset-api
```

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.

## Acknowledgments

- [Actix-web](https://actix.rs/) for the web framework
- [Diesel](https://diesel.rs) for ORM and Query Builder
- [Tonic](https://github.com/hyperium/tonic) for gRPC support
- All other open-source libraries used in this project