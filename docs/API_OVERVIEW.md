# StateSet API - Complete Overview

## Introduction

StateSet API is a comprehensive, enterprise-grade backend system built in Rust for modern e-commerce, supply chain management, and manufacturing operations. It provides a unified platform for managing the complete lifecycle of orders, inventory, returns, shipments, and manufacturing processes.

## Architecture Overview

### Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                     Client Applications                      │
│        (Web, Mobile, CLI, ChatGPT, Third-party)             │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                      API Gateway Layer                       │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  REST API   │  │   gRPC API   │  │  Agentic API  │      │
│  │  (Port 8080)│  │  (Port 50051)│  │  (ChatGPT)   │      │
│  └─────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   Middleware & Security                      │
│  • Authentication (JWT, API Keys)                           │
│  • Authorization (RBAC)                                     │
│  • Rate Limiting                                            │
│  • Idempotency                                              │
│  • Request Tracing                                          │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                     Business Services                        │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │ Orders   │ │Inventory │ │ Returns  │ │Shipments │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │ Products │ │  Cart    │ │ Checkout │ │Customers │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │Payments  │ │Manufacturing│Warehousing│Analytics │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                    Data & Event Layer                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │  PostgreSQL  │  │    Redis     │  │Event Outbox  │     │
│  │  (Primary)   │  │   (Cache)    │  │  (Events)    │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│                   External Integrations                      │
│  • Payment Processors (Stripe, StablePay)                   │
│  • Shipping Carriers (UPS, FedEx, USPS)                     │
│  • E-commerce Platforms (Shopify)                           │
│  • AI Services (OpenAI ChatGPT)                             │
│  • Notification Services (Email, SMS)                       │
└─────────────────────────────────────────────────────────────┘
```

## API Protocols

### 1. REST API (Primary Interface)

**Base URL**: `http://localhost:8080/api/v1`

- **Format**: JSON
- **Authentication**: JWT Bearer tokens or API Keys
- **Versioning**: URL-based (`/api/v1/`)
- **Documentation**: Swagger UI at `/swagger-ui`

### 2. gRPC API (Service-to-Service)

**Address**: `localhost:50051`

- **Protocol**: Protocol Buffers (protobuf)
- **Services**: High-performance internal communication
- **Use Cases**: Microservices, high-throughput operations

### 3. Agentic Commerce API (ChatGPT Integration)

**Base URL**: `http://localhost:8080` (Agentic Server)

- **Protocol**: OpenAI Agentic Commerce Protocol
- **Purpose**: AI-powered checkout in ChatGPT
- **Features**: Natural language shopping, delegated payment

## Core Capabilities

### 1. Order Management System (OMS)

Complete order lifecycle management from creation to fulfillment and beyond.

**Features**:
- Multi-item order creation and tracking
- Order state machine (pending → processing → shipped → delivered)
- Order holds and cancellations
- Order archival for historical records
- Order merging and splitting
- Custom order attributes and tags

**Key Endpoints**:
- `POST /api/v1/orders` - Create order
- `GET /api/v1/orders` - List with filters (status, date range, customer)
- `PUT /api/v1/orders/{id}/status` - Update status
- `POST /api/v1/orders/{id}/cancel` - Cancel order
- `GET /api/v1/orders/{id}/items` - Get order line items

**Workflow Example**:
```
1. Create Order → 2. Authorize Payment → 3. Reserve Inventory →
4. Create Fulfillment → 5. Pick & Pack → 6. Create Shipment →
7. Ship & Track → 8. Deliver → 9. Complete
```

### 2. Inventory Management

Real-time inventory tracking across multiple locations with advanced allocation logic.

**Features**:
- Multi-location inventory tracking
- Inventory reservation and allocation
- Available-to-promise (ATP) calculations
- Safety stock and reorder point management
- Lot tracking and expiration dates
- Cycle counting and adjustments
- Inventory transfer between locations

**Key Endpoints**:
- `GET /api/v1/inventory` - List inventory across locations
- `POST /api/v1/inventory/{id}/reserve` - Reserve for orders
- `POST /api/v1/inventory/{id}/release` - Release reservations
- `GET /api/v1/inventory/low-stock` - Low stock alerts

**Inventory States**:
- **On Hand**: Physical inventory in warehouse
- **Reserved**: Allocated to orders but not shipped
- **Available**: On hand minus reserved
- **On Order**: Incoming from suppliers
- **Available to Promise**: Available + on order - committed

### 3. Returns Management System (RMS)

Streamlined returns processing with automated workflows.

**Features**:
- RMA (Return Merchandise Authorization) generation
- Return approval/rejection workflows
- Restocking with condition tracking
- Refund processing integration
- Return reason tracking and analytics
- Return shipping label generation

**Key Endpoints**:
- `POST /api/v1/returns` - Create return request
- `POST /api/v1/returns/{id}/approve` - Approve return
- `POST /api/v1/returns/{id}/restock` - Restock items
- `GET /api/v1/returns` - List returns with filters

**Return Flow**:
```
1. Customer Request → 2. Review & Approve → 3. Generate RMA →
4. Customer Ships → 5. Receive & Inspect → 6. Restock →
7. Process Refund → 8. Update Inventory
```

### 4. Shipment Tracking

Multi-carrier shipment management with real-time tracking.

**Features**:
- Multi-carrier support (UPS, FedEx, USPS, DHL)
- Automatic tracking updates
- Delivery confirmation
- Shipping label generation
- Rate shopping across carriers
- Delivery signature requirements

**Key Endpoints**:
- `POST /api/v1/shipments` - Create shipment
- `POST /api/v1/shipments/{id}/ship` - Mark as shipped
- `GET /api/v1/shipments/track/{tracking_number}` - Track shipment

### 5. E-Commerce Platform

Full-featured e-commerce capabilities for online retail.

**Product Catalog**:
- Products with unlimited variants
- Rich product attributes
- Category management
- Search and filtering
- Image management

**Shopping Experience**:
- Session-based shopping carts
- Customer accounts and profiles
- Address management
- Wishlist functionality
- Product reviews (planned)

**Checkout Flow**:
```
1. Add to Cart → 2. Review Cart → 3. Enter Shipping →
4. Select Shipping Method → 5. Enter Payment →
6. Review Order → 7. Place Order
```

**Key Endpoints**:
- `GET /api/v1/products` - List products
- `POST /api/v1/carts` - Create cart
- `POST /api/v1/carts/{id}/items` - Add to cart
- `POST /api/v1/checkout` - Start checkout
- `POST /api/v1/checkout/{id}/complete` - Complete purchase

### 6. Agentic Commerce (AI-Powered Checkout)

Revolutionary shopping experience powered by ChatGPT.

**What is Agentic Commerce?**

Agentic Commerce enables customers to shop entirely within ChatGPT using natural language. The AI agent handles the complete checkout process including product selection, shipping options, and payment.

**Key Features**:
- Natural language product search and selection
- Conversational checkout process
- Automatic tax and shipping calculations
- Secure delegated payment (PSP vault)
- Single-use payment tokens
- Order confirmation via chat

**How It Works**:
```
1. Customer chats with ChatGPT: "I need running shoes"
2. ChatGPT calls StateSet API to search products
3. Customer selects product via conversation
4. ChatGPT creates checkout session
5. Customer provides shipping address in chat
6. ChatGPT calculates shipping options
7. Customer chooses shipping method
8. Customer authorizes payment (delegated to PSP)
9. ChatGPT completes checkout
10. Order created in StateSet system
```

**Technical Implementation**:
- Implements OpenAI Agentic Commerce Protocol
- Session-based checkout with state management
- Delegated payment via PSP vault tokens
- Webhook notifications to ChatGPT
- Idempotency for reliability

**Key Endpoints** (Agentic Server):
- `POST /checkout_sessions` - Create checkout session
- `GET /checkout_sessions/{id}` - Get session state
- `POST /checkout_sessions/{id}` - Update session
- `POST /checkout_sessions/{id}/complete` - Complete checkout
- `POST /agentic_commerce/delegate_payment` - Create payment token

### 7. Manufacturing Operations

Bill of Materials (BOM) management and work order tracking.

**Features**:
- Multi-level BOMs
- Component tracking
- Work order creation and scheduling
- Production tracking
- Material requirements planning (MRP)
- Quality control checkpoints

**Key Endpoints**:
- `POST /api/v1/manufacturing/boms` - Create BOM
- `POST /api/v1/work-orders` - Create work order
- `POST /api/v1/work-orders/{id}/complete` - Complete work order

### 8. Financial Operations

**Payment Processing**:
- Credit card processing (Stripe integration)
- Cryptocurrency payments (StablePay)
- Refund management
- Payment method tokenization

**Invoicing**:
- Automated invoice generation
- Invoice tracking and reminders
- Payment reconciliation

**StablePay Integration**:
- Accept stablecoin payments (USDC, USDT)
- Crypto-to-fiat conversion
- Transaction reconciliation
- Blockchain transaction tracking

**Key Endpoints**:
- `POST /api/v1/payments` - Process payment
- `POST /api/v1/payments/refund` - Refund payment
- `GET /api/v1/invoices` - List invoices

### 9. Analytics & Reporting

Business intelligence and operational insights.

**Dashboard Metrics**:
- Total orders and revenue
- Average order value
- Orders by status
- Low stock alerts
- Pending shipments

**Advanced Analytics**:
- Sales trends over time
- Inventory turnover rates
- Return rate analysis
- Customer lifetime value
- Product performance

**Key Endpoints**:
- `GET /api/v1/analytics/dashboard` - Dashboard overview
- `GET /api/v1/analytics/sales/trends` - Sales trends
- `GET /api/v1/analytics/inventory/turnover` - Inventory metrics

## Security Features

### Authentication

**JWT Tokens** (for user sessions):
- Access tokens (1 hour expiry)
- Refresh tokens (30 days expiry)
- Automatic token rotation
- Token revocation on logout

**API Keys** (for service-to-service):
- Scoped permissions
- Expiration dates
- Usage tracking
- Revocable at any time

### Authorization

**Role-Based Access Control (RBAC)**:
- Roles: Admin, Manager, Staff, Customer
- Granular permissions (e.g., `orders:read`, `orders:create`)
- Permission inheritance
- Custom role creation

### Security Best Practices

- `#![forbid(unsafe_code)]` - Memory-safe Rust
- Password hashing (Argon2)
- Rate limiting per user/API key/path
- Idempotency keys for mutations
- Request signature verification
- HTTPS enforcement
- CORS configuration
- SQL injection prevention (parameterized queries)
- XSS protection

## Performance & Scalability

### Performance Characteristics

- **Response Time**: <100ms for most endpoints
- **Throughput**: 1000+ requests/second
- **Concurrent Connections**: 10,000+
- **Database Pooling**: Optimized connection management

### Scalability Features

- **Horizontal Scaling**: Stateless design allows multiple instances
- **Database Read Replicas**: Separate read/write workloads
- **Redis Caching**: Session and frequently-accessed data
- **Async I/O**: Non-blocking operations throughout
- **Connection Pooling**: Efficient resource utilization

### Caching Strategy

- **Session Cache**: User sessions and tokens (Redis)
- **Entity Cache**: Frequently-accessed entities (Redis)
- **HTTP Cache**: Response caching with ETags
- **Query Cache**: Database query results

## Event-Driven Architecture

### Outbox Pattern

All domain events are stored in an outbox table before being published. This ensures reliable event delivery even if external systems are unavailable.

**Event Types**:
- `order.created`
- `order.status_changed`
- `inventory.reserved`
- `inventory.released`
- `shipment.created`
- `shipment.shipped`
- `return.created`
- `return.approved`
- `payment.processed`
- `payment.refunded`

### Webhooks

Subscribe to events via webhooks for real-time notifications.

**Configuration**:
```json
{
  "url": "https://your-app.com/webhooks",
  "events": ["order.created", "shipment.shipped"],
  "secret": "your-webhook-secret"
}
```

**Webhook Payload**:
```json
{
  "event": "order.created",
  "timestamp": "2025-11-05T10:00:00Z",
  "data": {
    "order_id": "550e8400-e29b-41d4-a716-446655440000",
    "order_number": "ORD-12345",
    "customer_id": "customer-uuid",
    "total_amount": 199.98,
    "status": "pending"
  },
  "signature": "sha256=..."
}
```

## Observability

### Logging

**Structured JSON Logging**:
```json
{
  "timestamp": "2025-11-05T10:00:00Z",
  "level": "INFO",
  "message": "Order created successfully",
  "request_id": "req-abc123",
  "user_id": "user-uuid",
  "order_id": "order-uuid",
  "duration_ms": 45
}
```

### Tracing

**OpenTelemetry Integration**:
- Distributed request tracing
- Span context propagation
- Performance bottleneck identification
- Service dependency mapping

### Metrics

**Prometheus Metrics** (available at `/metrics`):

**System Metrics**:
- `http_requests_total{method, route, status}`
- `http_request_duration_ms{method, route, status}`
- `database_connections{state}`
- `redis_operations_total{operation}`

**Business Metrics**:
- `orders_created_total`
- `orders_completed_total`
- `returns_processed_total`
- `inventory_reservations_total`
- `payments_processed_total`

**Rate Limiting Metrics**:
- `rate_limit_allowed_total{key_type, path}`
- `rate_limit_denied_total{key_type, path}`

### Health Checks

**Endpoints**:
- `GET /health` - Basic health check
- `GET /health/readiness` - Database connectivity
- `GET /health/version` - Build and version info

## Data Models

### Core Entities

**Order**:
```rust
{
  id: UUID,
  order_number: String,
  customer_id: UUID,
  status: OrderStatus,
  total_amount: Decimal,
  currency: String,
  items: Vec<OrderItem>,
  shipping_address: Address,
  billing_address: Address,
  created_at: DateTime,
  updated_at: DateTime
}
```

**Inventory**:
```rust
{
  id: UUID,
  product_id: UUID,
  location_id: UUID,
  quantity_on_hand: i32,
  quantity_reserved: i32,
  quantity_available: i32,
  reorder_point: i32,
  safety_stock: i32
}
```

**Return**:
```rust
{
  id: UUID,
  rma_number: String,
  order_id: UUID,
  status: ReturnStatus,
  items: Vec<ReturnItem>,
  reason: String,
  refund_amount: Decimal,
  created_at: DateTime
}
```

## Integration Guides

### Integrating with Shopify

StateSet can act as an OMS backend for Shopify stores:

1. Sync products and inventory
2. Receive orders via webhook
3. Update order status back to Shopify
4. Sync inventory levels
5. Handle returns through StateSet

### Integrating with Stripe

Payment processing integration:

1. Create payment intent
2. Confirm payment
3. Handle webhooks
4. Process refunds

### ChatGPT Integration

Enable AI-powered shopping:

1. Register with OpenAI Agentic Commerce
2. Configure agentic server endpoint
3. Set up payment service provider
4. Test checkout flow in ChatGPT

## Development Tools

### StateSet CLI

Command-line interface for API operations:

```bash
# Authentication
stateset-cli auth login --email user@example.com --password pass --save

# Orders
stateset-cli orders create --customer-id <uuid> --item sku=ABC,quantity=2,price=99.99
stateset-cli orders list --status pending

# Products
stateset-cli products create --name "Widget" --sku ABC-123 --price 99.99

# Inventory
stateset-cli inventory list --low-stock
```

### API Testing

**Swagger UI**: `http://localhost:8080/swagger-ui`

Interactive API documentation with:
- Request/response schemas
- Try-it-out functionality
- Authentication testing
- Response examples

## Deployment Options

### Docker

```bash
docker-compose up -d
```

### Kubernetes

Helm charts available for production deployments:
- Horizontal pod autoscaling
- Load balancing
- Secret management
- Persistent volumes

### Systemd

Run as a system service:

```ini
[Unit]
Description=StateSet API
After=network.target

[Service]
Type=simple
User=stateset
ExecStart=/usr/local/bin/stateset-api
Restart=always

[Install]
WantedBy=multi-user.target
```

## Best Practices

### API Usage

1. **Use Idempotency Keys** for all mutations
2. **Implement Retry Logic** with exponential backoff
3. **Cache Responses** when appropriate
4. **Use API Keys** for service-to-service communication
5. **Monitor Rate Limits** in response headers
6. **Validate Webhooks** using signature verification

### Error Handling

1. **Check HTTP Status Codes** before parsing responses
2. **Use Request IDs** for troubleshooting
3. **Implement Circuit Breakers** for resilience
4. **Log All Errors** with context

### Performance Optimization

1. **Use Pagination** for large result sets
2. **Filter at API Level** rather than client-side
3. **Batch Operations** when possible
4. **Use gRPC** for high-throughput scenarios
5. **Enable Compression** for large payloads

## Support & Resources

- **Documentation**: This file and `examples/` directory
- **Swagger UI**: `http://localhost:8080/swagger-ui`
- **CLI Tool**: `stateset-cli --help`
- **GitHub Issues**: Report bugs and request features
- **Email Support**: support@stateset.io

## Roadmap

Planned features:
- [ ] Advanced shipping integrations
- [ ] Multi-currency support
- [ ] Subscription billing
- [ ] Advanced analytics dashboard
- [ ] Mobile SDKs (iOS, Android)
- [ ] GraphQL API
- [ ] Multi-tenant support
- [ ] Advanced fraud detection
- [ ] Loyalty program management
- [ ] Warehouse management system (WMS)

## Conclusion

StateSet API provides a comprehensive, production-ready solution for modern commerce operations. Its combination of traditional e-commerce capabilities, manufacturing support, and cutting-edge AI-powered shopping makes it suitable for businesses of all sizes, from startups to enterprises.

The API's focus on performance, security, and developer experience ensures that you can build robust applications quickly while maintaining the flexibility to scale as your business grows.
