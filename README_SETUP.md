# StateSet API - Setup Guide

This guide will help you get the StateSet API up and running quickly.

## 🚀 Quick Start with Docker (Recommended)

### Prerequisites
- Docker and Docker Compose installed
- At least 2GB of available RAM

### 1. Clone and Setup
```bash
git clone <repository-url>
cd stateset-api
```

### 2. Environment Configuration
Create a `.env` file or set environment variables:
```bash
# Database
DATABASE_URL=postgres://postgres:postgres@localhost:5432/stateset_db

# JWT Configuration
JWT_SECRET=your_secure_jwt_secret_key_please_change_in_production
JWT_EXPIRATION=60
JWT_REFRESH_EXPIRATION=7

# Redis
REDIS_URL=redis://localhost:6379

# Application
RUST_LOG=info
APP_ENV=development
PORT=3000
```

### 3. Start the Services
```bash
# Start all services (PostgreSQL, Redis, API)
docker-compose up -d

# Run database migrations
docker-compose run --rm migrate

# Check logs
docker-compose logs -f stateset-api
```

### 4. Verify Installation
```bash
# Health check
curl http://localhost:3000/health

# API status
curl http://localhost:3000/status
```

## 🏗️ Manual Setup (Development)

### Prerequisites
- Rust 1.70+
- PostgreSQL 15+
- Redis 7+

### 1. Database Setup
```bash
# Create database
createdb stateset_db

# Run migrations
cargo run --bin migration
```

### 2. Install Dependencies
```bash
cargo build
```

### 3. Run the API Server
```bash
cargo run --bin api_server
```

## 📋 API Endpoints

### Authentication
```
POST /customers/register    - Register new customer
POST /customers/login       - Customer login
```

### Orders
```
GET    /orders              - List orders
POST   /orders              - Create order
GET    /orders/{id}         - Get order details
PUT    /orders/{id}         - Update order
DELETE /orders/{id}         - Delete order
POST   /orders/{id}/cancel  - Cancel order
```

### Inventory
```
GET    /inventory           - List inventory
POST   /inventory           - Create inventory item
GET    /inventory/{id}      - Get inventory item
PUT    /inventory/{id}      - Update inventory item
DELETE /inventory/{id}      - Delete inventory item
```

### Payments
```
POST   /payments            - Process payment
GET    /payments            - List payments
GET    /payments/{id}       - Get payment details
POST   /payments/refund     - Refund payment
GET    /payments/order/{id} - Get payments for order
```

### Advanced Shipping Notices (ASN)
```
GET    /asns                - List ASNs
POST   /asns                - Create ASN
GET    /asns/{id}           - Get ASN details
PUT    /asns/{id}           - Update ASN
DELETE /asns/{id}           - Delete ASN
POST   /asns/{id}/in-transit - Mark ASN in transit
POST   /asns/{id}/delivered - Mark ASN delivered
```

### Purchase Orders
```
GET    /purchase-orders             - List purchase orders
POST   /purchase-orders             - Create purchase order
GET    /purchase-orders/{id}        - Get purchase order
PUT    /purchase-orders/{id}        - Update purchase order
POST   /purchase-orders/{id}/approve - Approve purchase order
POST   /purchase-orders/{id}/cancel - Cancel purchase order
GET    /purchase-orders/supplier/{id} - Get POs by supplier
```

### Returns & Warranties
```
GET    /returns             - List returns
POST   /returns             - Create return
GET    /returns/{id}        - Get return details
PUT    /returns/{id}        - Update return
DELETE /returns/{id}        - Delete return

GET    /warranties          - List warranties
POST   /warranties          - Create warranty
GET    /warranties/{id}     - Get warranty details
PUT    /warranties/{id}     - Update warranty
DELETE /warranties/{id}     - Delete warranty
```

## 🧪 Testing

### Run Integration Tests
```bash
cargo test --test integration
```

### API Testing with Sample Data
```bash
# Register a customer
curl -X POST http://localhost:3000/customers/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "customer@example.com",
    "first_name": "John",
    "last_name": "Doe",
    "password": "securepassword123"
  }'

# Login
curl -X POST http://localhost:3000/customers/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "customer@example.com",
    "password": "securepassword123"
  }'
```

## 📊 Features Implemented

✅ **Core Services**
- Customer Management
- Order Processing
- Inventory Management
- Payment Processing
- Advanced Shipping Notices
- Purchase Orders
- Returns & Warranties
- Work Orders

✅ **Infrastructure**
- PostgreSQL Database
- Redis Caching
- Docker Support
- Authentication & Authorization
- Comprehensive Logging
- Health Checks

✅ **API Features**
- RESTful Endpoints
- JSON API Responses
- Input Validation
- Error Handling
- Pagination Support
- Event-Driven Architecture

## 🔧 Development

### Adding New Features
1. Define your protobuf messages in `proto/`
2. Implement models in `src/models/`
3. Create services in `src/services/`
4. Add handlers in `src/handlers/`
5. Update routes in `src/lib.rs`

### Database Changes
1. Create migration files in `migrations/`
2. Update models to reflect schema changes
3. Run migrations: `docker-compose run --rm migrate`

## 📚 Documentation

- **API Documentation**: Available at `http://localhost:3000/docs` (when running)
- **Health Check**: `http://localhost:3000/health`
- **Status**: `http://localhost:3000/status`

## 🤝 Support

For questions or issues:
1. Check the logs: `docker-compose logs stateset-api`
2. Verify database connectivity
3. Ensure all environment variables are set
4. Check the troubleshooting section below

## 🐛 Troubleshooting

### Common Issues

**Database Connection Failed**
```bash
# Check if PostgreSQL is running
docker-compose ps postgres

# View database logs
docker-compose logs postgres

# Reset database
docker-compose down -v
docker-compose up -d postgres
```

**API Server Won't Start**
```bash
# Check environment variables
docker-compose exec stateset-api env

# View application logs
docker-compose logs stateset-api
```

**Compilation Errors**
```bash
# Clean and rebuild
docker-compose exec stateset-api cargo clean
docker-compose exec stateset-api cargo build
```

---

🎉 **Congratulations!** Your StateSet API is now ready for development and production use.
