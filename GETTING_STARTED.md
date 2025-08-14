# Getting Started with StateSet API

Welcome to StateSet API! This guide will help you get up and running with our comprehensive backend system for order management, inventory control, returns processing, and more.

## Table of Contents
- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Configuration](#configuration)
- [Database Setup](#database-setup)
- [Building the Project](#building-the-project)
- [Running the API](#running-the-api)
- [Development Workflow](#development-workflow)
- [Testing](#testing)
- [API Documentation](#api-documentation)
- [Troubleshooting](#troubleshooting)

## Prerequisites

Before you begin, ensure you have the following installed on your development machine:

### Required Software
- **Rust** (latest stable version)
  ```bash
  # Install Rust via rustup
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  
  # Verify installation
  rustc --version
  cargo --version
  ```

- **Database** (Choose one):
  - **SQLite** (default for development)
    - Automatically created when running the API
  - **PostgreSQL 14+** (recommended for production)
    ```bash
    # macOS
    brew install postgresql@14
    
    # Ubuntu/Debian
    sudo apt-get install postgresql-14
    
    # Verify installation
    psql --version
    ```

### Optional Tools
- **Redis** (for caching, optional)
  ```bash
  # macOS
  brew install redis
  
  # Ubuntu/Debian
  sudo apt-get install redis-server
  ```

- **Protocol Buffer Compiler** (for gRPC development)
  ```bash
  # macOS
  brew install protobuf
  
  # Ubuntu/Debian
  sudo apt-get install protobuf-compiler
  ```

## Installation

1. **Clone the Repository**
   ```bash
   git clone https://github.com/stateset/stateset-api.git
   cd stateset-api
   ```

2. **Install Rust Dependencies**
   ```bash
   # This will download and compile all dependencies
   cargo fetch
   ```

## Configuration

StateSet API uses environment variables and configuration files for setup.

### 1. Environment Variables (.env)

Create a `.env` file in the project root:

```bash
cp .env.example .env
```

Edit `.env` with your configuration:

```env
# Database Configuration
DATABASE_URL=sqlite:stateset.db?mode=rwc  # For SQLite (development)
# DATABASE_URL=postgres://username:password@localhost/stateset_db  # For PostgreSQL

# JWT Configuration
JWT_SECRET=your_secure_jwt_secret_key_change_in_production
JWT_EXPIRATION=3600              # 1 hour in seconds
JWT_REFRESH_EXPIRATION=604800    # 7 days in seconds

# Server Configuration
PORT=8080
HOST=0.0.0.0
APP_ENV=development

# Logging
RUST_LOG=stateset_api=debug,tower_http=debug,sea_orm=debug,info

# Optional: Redis (if using caching)
# REDIS_URL=redis://localhost:6379
```

### 2. Configuration File (config/default.toml)

The TOML configuration file provides additional settings:

```toml
# Database
database_url = "sqlite:stateset.db?mode=rwc"
auto_migrate = true  # Automatically run migrations on startup

# Server
host = "0.0.0.0"
port = 8080

# Cache (optional)
[cache]
cache_type = "memory"  # or "redis" if Redis is configured
capacity = 1000
default_ttl_secs = 300
```

## Database Setup

### SQLite (Default for Development)

SQLite requires no additional setup. The database file will be created automatically when you first run the application.

### PostgreSQL (Production)

1. **Create Database**
   ```bash
   # Connect to PostgreSQL
   psql -U postgres
   
   # Create database and user
   CREATE DATABASE stateset_db;
   CREATE USER stateset_user WITH ENCRYPTED PASSWORD 'your_password';
   GRANT ALL PRIVILEGES ON DATABASE stateset_db TO stateset_user;
   \q
   ```

2. **Update DATABASE_URL in .env**
   ```env
   DATABASE_URL=postgres://stateset_user:your_password@localhost/stateset_db
   ```

### Running Migrations

Migrations are handled automatically on startup if `auto_migrate = true` in your config. To run them manually:

```bash
# Using the migration binary
cargo run --bin migration

# Or using SQLx CLI (if installed)
sqlx migrate run
```

## Building the Project

StateSet API provides several build options:

### Development Build
```bash
# Quick build for development
cargo build

# Or using the Makefile
make build
```

### Release Build (Optimized)
```bash
# Optimized build for production
cargo build --release

# Or using the Makefile
make build-release
```

### Build Specific Components
```bash
# Build only the main API server
cargo build --bin stateset-api

# Build the minimal server (lightweight version)
cargo build --bin minimal-server

# Build the gRPC server
cargo build --bin grpc-server

# Build performance testing tools
cargo build --bin orders-bench
cargo build --bin orders-mock-server
```

## Running the API

### Main API Server
```bash
# Run in development mode
cargo run

# Or using the Makefile
make run

# Run with specific binary
cargo run --bin stateset-api

# Run release build
./target/release/stateset-api
```

### Alternative Servers
```bash
# Run minimal server (lightweight, fewer features)
cargo run --bin minimal-server

# Run gRPC server
cargo run --bin grpc-server

# Run mock server for testing
cargo run --bin orders-mock-server
```

### Verify the Server is Running
```bash
# Health check
curl http://localhost:8080/health

# Should return:
# {"status":"healthy"}
```

## Development Workflow

### 1. Code Organization

```
src/
├── commands/        # Write operations (Command pattern)
├── queries/         # Read operations (Query pattern)
├── handlers/        # HTTP request handlers
├── services/        # Business logic
├── entities/        # Database entities (SeaORM)
├── models/          # Domain models
├── repositories/    # Data access layer
└── main.rs         # Application entry point
```

### 2. Adding New Features

1. **Create Command/Query**
   ```rust
   // src/commands/orders/create_order_command.rs
   pub struct CreateOrderCommand {
       pub customer_id: Uuid,
       pub items: Vec<OrderItem>,
   }
   ```

2. **Implement Service**
   ```rust
   // src/services/orders.rs
   impl OrderService {
       pub async fn create_order(&self, cmd: CreateOrderCommand) -> Result<Order> {
           // Business logic here
       }
   }
   ```

3. **Add Handler**
   ```rust
   // src/handlers/orders.rs
   pub async fn create_order(
       State(state): State<AppState>,
       Json(payload): Json<CreateOrderRequest>,
   ) -> Result<Json<OrderResponse>> {
       // Handle HTTP request
   }
   ```

### 3. Hot Reloading (Development)

Use `cargo-watch` for automatic recompilation:

```bash
# Install cargo-watch
cargo install cargo-watch

# Run with auto-reload
cargo watch -x run
```

### 4. Code Quality Tools

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check for security issues
cargo audit

# Generate documentation
cargo doc --open
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_create_order

# Run tests with output
cargo test -- --nocapture

# Run tests with backtrace
RUST_BACKTRACE=1 cargo test

# Run integration tests only
cargo test --test '*'

# Using Makefile
make test
```

### Writing Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_create_order() {
        // Test implementation
        let service = OrderService::new(db_pool);
        let order = service.create_order(cmd).await.unwrap();
        assert_eq!(order.status, "pending");
    }
}
```

### Performance Testing

```bash
# Run order throughput benchmark
cargo run --bin orders-bench

# Start mock server for load testing
cargo run --bin orders-mock-server

# In another terminal, run benchmarks
ab -n 1000 -c 10 http://localhost:8080/orders
```

## API Documentation

### REST API Endpoints

The API provides comprehensive endpoints for all operations:

#### Authentication
- `POST /auth/register` - Register new user
- `POST /auth/login` - Login and receive JWT token
- `POST /auth/refresh` - Refresh JWT token

#### Orders
- `GET /orders` - List all orders
- `GET /orders/:id` - Get order by ID
- `POST /orders` - Create new order
- `PUT /orders/:id` - Update order
- `DELETE /orders/:id` - Delete order
- `POST /orders/:id/cancel` - Cancel order
- `POST /orders/:id/hold` - Put order on hold

#### Inventory
- `GET /inventory` - Get inventory levels
- `POST /inventory/adjust` - Adjust inventory
- `POST /inventory/allocate` - Allocate inventory
- `POST /inventory/reserve` - Reserve inventory

#### Returns
- `POST /returns` - Create return
- `GET /returns/:id` - Get return details
- `POST /returns/:id/approve` - Approve return
- `POST /returns/:id/reject` - Reject return

### OpenAPI/Swagger Documentation

When running in development mode, interactive API documentation is available at:

```
http://localhost:8080/swagger-ui
```

### gRPC API

For gRPC clients, proto files are located in `proto/` directory. Generate client code:

```bash
# Generate Rust code from proto files
cargo build  # Automatically generates during build

# For other languages, use protoc directly
protoc --go_out=. --go-grpc_out=. proto/*.proto
```

## Troubleshooting

### Common Issues and Solutions

#### 1. Build Errors

```bash
# Clear build cache
cargo clean

# Update dependencies
cargo update

# Check for specific errors
cargo build 2>&1 | tee build_errors.log
```

#### 2. Database Connection Issues

```bash
# Test database connection
psql $DATABASE_URL -c "SELECT 1"

# For SQLite, check file permissions
ls -la stateset.db

# Reset database
rm stateset.db  # For SQLite
# OR
DROP DATABASE stateset_db; CREATE DATABASE stateset_db;  # For PostgreSQL
```

#### 3. Port Already in Use

```bash
# Find process using port 8080
lsof -i :8080

# Kill the process
kill -9 <PID>

# Or use a different port
PORT=3000 cargo run
```

#### 4. Migration Failures

```bash
# Reset migrations
cargo run --bin migration -- reset

# Run migrations step by step
cargo run --bin migration -- up
```

### Debugging

```bash
# Enable debug logging
RUST_LOG=debug cargo run

# Enable backtrace for errors
RUST_BACKTRACE=1 cargo run

# Use debugger (VS Code or IntelliJ Rust)
# Add breakpoints and run in debug mode
```

### Performance Issues

```bash
# Profile the application
cargo build --release
valgrind --tool=callgrind ./target/release/stateset-api

# Check database queries
RUST_LOG=sea_orm=debug cargo run

# Monitor resource usage
htop  # While application is running
```

## Next Steps

1. **Explore the Codebase**
   - Review `src/handlers/` for API endpoints
   - Check `src/services/` for business logic
   - Look at `tests/` for usage examples

2. **Customize Configuration**
   - Modify `config/default.toml` for your needs
   - Set up proper JWT secrets for security
   - Configure database connections

3. **Set Up Development Environment**
   - Install VS Code with rust-analyzer extension
   - Configure your IDE for Rust development
   - Set up pre-commit hooks for code quality

4. **Learn the Architecture**
   - Read about CQRS pattern used in commands/queries
   - Understand the event-driven architecture
   - Review the domain models and entities

5. **Contribute**
   - Check out open issues on GitHub
   - Read CONTRIBUTING.md for guidelines
   - Join our community discussions

## Getting Help

- **Documentation**: Full docs at [docs.stateset.com](https://docs.stateset.com)
- **GitHub Issues**: [github.com/stateset/stateset-api/issues](https://github.com/stateset/stateset-api/issues)
- **Discord Community**: Join our Discord for real-time help
- **API Reference**: Check `/swagger-ui` when running locally

## License

StateSet API is licensed under the BSL License. See [LICENSE](LICENSE) for details.

---

Happy coding! Welcome to the StateSet community! 🚀