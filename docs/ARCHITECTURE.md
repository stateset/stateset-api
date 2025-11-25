# StateSet API Architecture

This document provides visual architectural diagrams and detailed explanations of the StateSet API system design.

## Table of Contents

- [System Overview](#system-overview)
- [Authentication Flow](#authentication-flow)
- [Order Fulfillment Flow](#order-fulfillment-flow)
- [Data Flow Architecture](#data-flow-architecture)
- [Component Relationships](#component-relationships)

---

## System Overview

The StateSet API follows a layered architecture with clear separation of concerns:

```mermaid
graph TB
    subgraph "Client Layer"
        CLI[CLI Tool]
        Web[Web Clients]
        Mobile[Mobile Apps]
        API_Clients[API Integrations]
    end

    subgraph "API Gateway Layer"
        HTTP[HTTP/REST<br/>Port 8080]
        GRPC[gRPC<br/>Port 50051]
    end

    subgraph "Middleware Layer"
        Auth[Authentication<br/>JWT/API Keys]
        RateLimit[Rate Limiting<br/>Redis-backed]
        Metrics[Metrics<br/>Prometheus]
        Tracing[Request Tracing<br/>OpenTelemetry]
        Validation[Input Validation]
    end

    subgraph "Handler Layer"
        OrderH[Order Handlers]
        InvH[Inventory Handlers]
        ReturnH[Return Handlers]
        WarrantyH[Warranty Handlers]
        ShipH[Shipment Handlers]
        WorkH[Work Order Handlers]
    end

    subgraph "Service Layer"
        OrderS[Order Service]
        InvS[Inventory Service]
        ReturnS[Return Service]
        WarrantyS[Warranty Service]
        ShipS[Shipment Service]
        WorkS[Work Order Service]
    end

    subgraph "Data Layer"
        Commands[Commands<br/>Write Operations]
        Queries[Queries<br/>Read Operations]
        Repos[Repositories<br/>Data Access]
    end

    subgraph "Storage Layer"
        DB[(PostgreSQL/<br/>SQLite)]
        Redis[(Redis<br/>Cache)]
        EventStore[Event Outbox]
    end

    subgraph "External Services"
        Webhooks[Webhook Delivery]
        Notifications[Notifications]
    end

    CLI --> HTTP
    Web --> HTTP
    Mobile --> HTTP
    API_Clients --> HTTP
    API_Clients --> GRPC

    HTTP --> Auth
    GRPC --> Auth
    Auth --> RateLimit
    RateLimit --> Metrics
    Metrics --> Tracing
    Tracing --> Validation
    Validation --> OrderH
    Validation --> InvH
    Validation --> ReturnH
    Validation --> WarrantyH
    Validation --> ShipH
    Validation --> WorkH

    OrderH --> OrderS
    InvH --> InvS
    ReturnH --> ReturnS
    WarrantyH --> WarrantyS
    ShipH --> ShipS
    WorkH --> WorkS

    OrderS --> Commands
    OrderS --> Queries
    InvS --> Commands
    InvS --> Queries
    ReturnS --> Commands
    ReturnS --> Queries

    Commands --> Repos
    Queries --> Repos
    Repos --> DB
    Repos --> Redis

    OrderS -.->|Events| EventStore
    InvS -.->|Events| EventStore
    EventStore -.->|Async| Webhooks
    EventStore -.->|Async| Notifications
```

### Key Architectural Patterns

1. **Layered Architecture**: Clear separation between handlers, services, and data access
2. **CQRS Pattern**: Separate command (write) and query (read) operations
3. **Repository Pattern**: Abstraction layer for data access
4. **Event-Driven**: Asynchronous event processing with outbox pattern
5. **Middleware Composition**: Composable cross-cutting concerns

---

## Authentication Flow

StateSet API supports multiple authentication methods with a robust security model:

```mermaid
sequenceDiagram
    participant Client
    participant API
    participant AuthService
    participant Database
    participant Redis

    Note over Client,Redis: Initial Login Flow
    Client->>+API: POST /auth/login<br/>{email, password}
    API->>+AuthService: authenticate(credentials)
    AuthService->>+Database: SELECT user WHERE email
    Database-->>-AuthService: User record + password_hash
    AuthService->>AuthService: Verify password<br/>(Argon2)
    AuthService->>+Database: SELECT roles & permissions
    Database-->>-AuthService: User roles
    AuthService->>AuthService: Generate JWT tokens<br/>(access + refresh)
    AuthService->>+Database: Store refresh_token
    Database-->>-AuthService: OK
    AuthService-->>-API: {access_token, refresh_token}
    API-->>-Client: 200 OK<br/>{access_token, refresh_token}

    Note over Client,Redis: Authenticated Request Flow
    Client->>+API: GET /orders<br/>Header: Authorization: Bearer {token}
    API->>+AuthService: validate_token(jwt)
    AuthService->>AuthService: Verify JWT signature<br/>Check expiration
    AuthService->>+Redis: Check token blacklist
    Redis-->>-AuthService: Not blacklisted
    AuthService->>+Database: SELECT permissions<br/>for user_id
    Database-->>-AuthService: Permission list
    AuthService->>AuthService: Check permission:<br/>"orders:read"
    AuthService-->>-API: Claims {user_id, roles, permissions}
    API->>API: Execute request
    API-->>-Client: 200 OK + Response

    Note over Client,Redis: API Key Flow
    Client->>+API: GET /orders<br/>Header: X-API-Key: sk_live_***
    API->>+AuthService: validate_api_key(key)
    AuthService->>+Redis: GET api_key_cache
    Redis-->>-AuthService: Cache miss
    AuthService->>+Database: SELECT api_key<br/>JOIN permissions
    Database-->>-AuthService: Key + permissions
    AuthService->>+Redis: SET api_key_cache
    Redis-->>-AuthService: Cached
    AuthService->>AuthService: Check permission<br/>& active status
    AuthService-->>-API: Valid + permissions
    API->>API: Execute request
    API-->>-Client: 200 OK + Response

    Note over Client,Redis: Token Refresh Flow
    Client->>+API: POST /auth/refresh<br/>{refresh_token}
    API->>+AuthService: refresh_access_token
    AuthService->>+Database: SELECT refresh_token<br/>CHECK valid & not revoked
    Database-->>-AuthService: Valid token
    AuthService->>AuthService: Generate new<br/>access_token
    AuthService-->>-API: {access_token}
    API-->>-Client: 200 OK<br/>{access_token}
```

### Security Features

- **Argon2 Password Hashing**: Industry-standard password hashing with salt
- **JWT with Refresh Tokens**: Short-lived access tokens (1 hour) + long-lived refresh tokens (24 hours)
- **Token Blacklisting**: Redis-backed revocation for immediate token invalidation
- **API Key Management**: Scoped API keys with fine-grained permissions
- **Permission-Based Access Control**: Role-based with wildcard support (e.g., `orders:*`)
- **Rate Limiting**: Per-user, per-API-key, and per-path rate limits

---

## Order Fulfillment Flow

Complete end-to-end order processing workflow:

```mermaid
sequenceDiagram
    participant Customer
    participant API
    participant OrderService
    participant InventoryService
    participant ShipmentService
    participant PaymentService
    participant Database
    participant EventBus
    participant Webhook

    Note over Customer,Webhook: Order Creation
    Customer->>+API: POST /orders<br/>{customer_id, items[]}
    API->>+OrderService: create_order_with_items()
    OrderService->>+Database: BEGIN TRANSACTION
    Database-->>-OrderService: Transaction started

    OrderService->>OrderService: Validate order<br/>- Items not empty<br/>- Amount > 0<br/>- Valid customer
    OrderService->>+Database: INSERT INTO orders
    Database-->>-OrderService: Order created
    OrderService->>+Database: INSERT INTO order_items
    Database-->>-OrderService: Items created

    OrderService->>+Database: COMMIT TRANSACTION
    Database-->>-OrderService: Committed
    OrderService->>+EventBus: Publish OrderCreated event
    EventBus-->>-OrderService: Event queued
    OrderService-->>-API: OrderResponse
    API-->>-Customer: 201 Created

    EventBus->>+Webhook: POST webhook_url<br/>{event: "order.created"}
    Webhook-->>-EventBus: 200 OK

    Note over Customer,Webhook: Inventory Reservation
    API->>+OrderService: POST /orders/{id}/process
    OrderService->>+InventoryService: reserve_inventory()<br/>for each item
    InventoryService->>+Database: BEGIN TRANSACTION
    Database-->>-InventoryService: Transaction started

    loop For each order item
        InventoryService->>+Database: SELECT inventory_balance<br/>WHERE item_id AND location<br/>FOR UPDATE
        Database-->>-InventoryService: Current balance

        alt Sufficient inventory
            InventoryService->>InventoryService: Check: available >= quantity
            InventoryService->>+Database: UPDATE inventory_balance<br/>SET allocated += quantity
            Database-->>-InventoryService: Updated
            InventoryService->>+Database: INSERT reservation record
            Database-->>-InventoryService: Reservation created
        else Insufficient inventory
            InventoryService->>+Database: ROLLBACK
            Database-->>-InventoryService: Rolled back
            InventoryService-->>OrderService: Error: Insufficient stock
            OrderService-->>API: 409 Conflict
        end
    end

    InventoryService->>+Database: COMMIT TRANSACTION
    Database-->>-InventoryService: Committed
    InventoryService->>+EventBus: Publish InventoryReserved event
    EventBus-->>-InventoryService: Event queued
    InventoryService-->>-OrderService: Reservations confirmed
    OrderService-->>-API: 200 OK

    Note over Customer,Webhook: Payment Processing
    API->>+PaymentService: process_payment()<br/>{order_id, amount}
    PaymentService->>PaymentService: Process payment<br/>(external gateway)
    alt Payment successful
        PaymentService->>+OrderService: update_payment_status<br/>("paid")
        OrderService->>+Database: UPDATE orders<br/>SET payment_status = 'paid'
        Database-->>-OrderService: Updated
        OrderService->>+EventBus: Publish OrderPaid event
        EventBus-->>-OrderService: Event queued
        OrderService-->>-PaymentService: Updated
        PaymentService-->>API: 200 OK
    else Payment failed
        PaymentService->>+InventoryService: release_reservation()<br/>for order items
        InventoryService->>+Database: UPDATE inventory_balance<br/>SET allocated -= quantity
        Database-->>-InventoryService: Released
        InventoryService-->>-PaymentService: Inventory released
        PaymentService->>+OrderService: update_status("cancelled")
        OrderService-->>-PaymentService: Updated
        PaymentService-->>API: 402 Payment Required
    end

    Note over Customer,Webhook: Shipment Creation
    API->>+ShipmentService: create_shipment()<br/>{order_id, items[]}
    ShipmentService->>+Database: INSERT INTO shipments
    Database-->>-ShipmentService: Shipment created
    ShipmentService->>ShipmentService: Assign carrier<br/>Generate tracking
    ShipmentService->>+Database: UPDATE shipments<br/>SET tracking_number
    Database-->>-ShipmentService: Updated
    ShipmentService->>+OrderService: update_order()<br/>{tracking_number}
    OrderService->>+Database: UPDATE orders<br/>SET tracking_number,<br/>status = 'processing'
    Database-->>-OrderService: Updated
    OrderService-->>-ShipmentService: Updated
    ShipmentService->>+EventBus: Publish ShipmentCreated event
    EventBus-->>-ShipmentService: Event queued
    ShipmentService-->>-API: ShipmentResponse

    Note over Customer,Webhook: Mark as Shipped
    API->>+ShipmentService: POST /shipments/{id}/ship
    ShipmentService->>+Database: UPDATE shipments<br/>SET status = 'shipped',<br/>shipped_at = NOW()
    Database-->>-ShipmentService: Updated
    ShipmentService->>+InventoryService: allocate_inventory()<br/>(deduct from on_hand)
    InventoryService->>+Database: UPDATE inventory_balance<br/>SET on_hand -= quantity,<br/>allocated -= quantity
    Database-->>-InventoryService: Updated
    InventoryService->>+EventBus: Publish InventoryAllocated event
    EventBus-->>-InventoryService: Event queued
    InventoryService-->>-ShipmentService: Inventory allocated
    ShipmentService->>+OrderService: update_status("shipped")
    OrderService->>+Database: UPDATE orders<br/>SET status = 'shipped',<br/>fulfillment_status = 'fulfilled'
    Database-->>-OrderService: Updated
    OrderService->>+EventBus: Publish OrderShipped event
    EventBus-->>-OrderService: Event queued
    OrderService-->>-ShipmentService: Updated
    ShipmentService-->>-API: 200 OK

    EventBus->>+Webhook: POST webhook_url<br/>{event: "order.shipped", tracking}
    Webhook-->>-EventBus: 200 OK
    EventBus->>+Customer: Email/SMS notification<br/>"Your order has shipped!"
    Customer-->>-EventBus: Received

    Note over Customer,Webhook: Delivery Confirmation
    ShipmentService->>+ShipmentService: Carrier webhook callback
    ShipmentService->>+Database: UPDATE shipments<br/>SET status = 'delivered',<br/>delivered_at = NOW()
    Database-->>-ShipmentService: Updated
    ShipmentService->>+OrderService: update_status("delivered")
    OrderService->>+Database: UPDATE orders<br/>SET status = 'delivered'
    Database-->>-OrderService: Updated
    OrderService->>+EventBus: Publish OrderDelivered event
    EventBus-->>-OrderService: Event queued
    OrderService-->>-ShipmentService: Updated
    EventBus->>+Webhook: POST webhook_url<br/>{event: "order.delivered"}
    Webhook-->>-EventBus: 200 OK
```

### Workflow Stages

1. **Order Creation**: Validate and persist order with line items in a transaction
2. **Inventory Reservation**: Reserve stock with optimistic locking
3. **Payment Processing**: Authorize and capture payment
4. **Shipment Creation**: Generate shipping labels and tracking
5. **Fulfillment**: Allocate inventory and mark as shipped
6. **Delivery**: Confirm delivery via carrier webhook

---

## Data Flow Architecture

How data flows through the system with CQRS pattern:

```mermaid
graph LR
    subgraph "Write Path (Commands)"
        WR[Write Request] --> CMD[Command Handler]
        CMD --> SVC1[Service]
        SVC1 --> TXN[Transaction]
        TXN --> DB1[(Database)]
        SVC1 -.->|Async| EVT[Event Bus]
        EVT -.-> OUTBOX[(Outbox Table)]
        OUTBOX -.-> WORKER[Outbox Worker]
        WORKER -.-> WEBHOOK[Webhooks]
    end

    subgraph "Read Path (Queries)"
        RR[Read Request] --> QRY[Query Handler]
        QRY --> CACHE{Cache Hit?}
        CACHE -->|Yes| RETURN1[Return Cached]
        CACHE -->|No| DB2[(Database)]
        DB2 --> REDIS[(Redis Cache)]
        DB2 --> RETURN2[Return Data]
    end

    subgraph "Consistency"
        EVT -.->|Update| CACHE
        WORKER -.->|Invalidate| REDIS
    end

    style WR fill:#e1f5ff
    style RR fill:#fff3e0
    style CMD fill:#c8e6c9
    style QRY fill:#ffecb3
    style DB1 fill:#f8bbd0
    style DB2 fill:#f8bbd0
```

### CQRS Benefits

- **Scalability**: Read and write paths can scale independently
- **Performance**: Queries can use denormalized views and caching
- **Maintainability**: Clear separation of concerns
- **Flexibility**: Different models for reads vs writes

---

## Component Relationships

Entity relationships and dependencies:

```mermaid
erDiagram
    CUSTOMER ||--o{ ORDER : places
    CUSTOMER ||--o{ CUSTOMER_ADDRESS : has
    ORDER ||--|{ ORDER_ITEM : contains
    ORDER ||--o| SHIPMENT : fulfilled_by
    ORDER ||--o{ RETURN : may_have
    ORDER ||--o| PAYMENT : paid_with

    PRODUCT ||--o{ ORDER_ITEM : ordered_in
    PRODUCT ||--o{ PRODUCT_VARIANT : has
    PRODUCT ||--o| BILL_OF_MATERIALS : defined_by

    INVENTORY_ITEM ||--o{ INVENTORY_BALANCE : tracked_at
    INVENTORY_BALANCE }o--|| LOCATION : stored_at
    INVENTORY_ITEM ||--o{ INVENTORY_RESERVATION : reserved_in

    SHIPMENT ||--o{ SHIPMENT_ITEM : contains
    SHIPMENT }o--|| CARRIER : shipped_via

    RETURN ||--|{ RETURN_ITEM : contains
    RETURN ||--o| REFUND : results_in

    WARRANTY ||--o{ WARRANTY_CLAIM : may_have

    WORK_ORDER ||--|{ WORK_ORDER_ITEM : contains
    BILL_OF_MATERIALS ||--|{ BOM_LINE_ITEM : composed_of

    USER ||--o{ REFRESH_TOKEN : owns
    USER ||--o{ API_KEY : generates
    USER }o--o{ ROLE : assigned
    ROLE }o--o{ PERMISSION : grants

    CUSTOMER {
        uuid id PK
        string email UK
        string first_name
        string last_name
        string phone
        timestamp created_at
    }

    ORDER {
        uuid id PK
        uuid customer_id FK
        string order_number UK
        string status
        decimal total_amount
        string currency
        timestamp order_date
    }

    ORDER_ITEM {
        uuid id PK
        uuid order_id FK
        uuid product_id FK
        string sku
        int quantity
        decimal unit_price
    }

    PRODUCT {
        uuid id PK
        string name
        string sku UK
        decimal price
        string brand
    }

    INVENTORY_ITEM {
        int id PK
        string item_number UK
        string description
        string uom
    }

    INVENTORY_BALANCE {
        int id PK
        int inventory_item_id FK
        int location_id FK
        decimal on_hand
        decimal allocated
        decimal available
    }
```

---

## Deployment Architecture

Production deployment topology:

```mermaid
graph TB
    subgraph "Load Balancer"
        LB[Nginx/HAProxy]
    end

    subgraph "API Instances"
        API1[API Instance 1<br/>:8080]
        API2[API Instance 2<br/>:8080]
        API3[API Instance 3<br/>:8080]
    end

    subgraph "Storage"
        PG_Primary[(PostgreSQL<br/>Primary)]
        PG_Replica[(PostgreSQL<br/>Read Replica)]
        Redis_Master[(Redis<br/>Master)]
        Redis_Replica[(Redis<br/>Replica)]
    end

    subgraph "Monitoring"
        Prom[Prometheus]
        Graf[Grafana]
        Jaeger[Jaeger<br/>Tracing]
    end

    LB --> API1
    LB --> API2
    LB --> API3

    API1 --> PG_Primary
    API2 --> PG_Primary
    API3 --> PG_Primary

    API1 -.->|Reads| PG_Replica
    API2 -.->|Reads| PG_Replica
    API3 -.->|Reads| PG_Replica

    API1 --> Redis_Master
    API2 --> Redis_Master
    API3 --> Redis_Master

    Redis_Master -.->|Replication| Redis_Replica

    API1 -.->|Metrics| Prom
    API2 -.->|Metrics| Prom
    API3 -.->|Metrics| Prom

    Prom --> Graf

    API1 -.->|Traces| Jaeger
    API2 -.->|Traces| Jaeger
    API3 -.->|Traces| Jaeger

    PG_Primary -.->|Replication| PG_Replica
```

### Infrastructure Components

- **Load Balancer**: Distributes traffic across API instances
- **API Instances**: Stateless, horizontally scalable
- **PostgreSQL**: Primary for writes, replicas for read scaling
- **Redis**: Master for writes/cache, replica for failover
- **Prometheus**: Metrics collection and alerting
- **Grafana**: Metrics visualization
- **Jaeger**: Distributed tracing for request flows

---

## Technology Stack

### Core
- **Language**: Rust 1.88+
- **Web Framework**: Axum (Tokio-based)
- **ORM**: SeaORM with async support
- **Runtime**: Tokio for async operations

### Protocols
- **REST API**: Primary client interface
- **gRPC**: Service-to-service communication
- **WebSockets**: Real-time updates (future)

### Data
- **Primary Database**: PostgreSQL 15+
- **Cache/Session**: Redis 7+
- **Search**: Future: Elasticsearch integration

### Observability
- **Metrics**: Prometheus + custom registry
- **Tracing**: OpenTelemetry + Jaeger
- **Logging**: Structured JSON logs via tracing-subscriber
- **Health**: Built-in health check endpoints

### Security
- **Authentication**: JWT + API Keys
- **Password Hashing**: Argon2
- **TLS**: Rustls for HTTPS
- **Rate Limiting**: Redis-backed token bucket

---

## Performance Characteristics

### Throughput
- **Peak**: 10,000+ req/sec per instance
- **Average Response Time**: < 50ms (p50)
- **P99 Response Time**: < 200ms

### Scalability
- **Horizontal**: Stateless design for easy scaling
- **Vertical**: Efficient memory usage (~50MB baseline)
- **Connection Pooling**: Configurable pool sizes

### Reliability
- **Availability**: 99.9%+ with multi-instance deployment
- **Fault Tolerance**: Automatic retry with exponential backoff
- **Data Consistency**: ACID transactions for critical operations

---

## Future Enhancements

1. **Microservices**: Split into domain-specific services
2. **Event Sourcing**: Full event-sourced architecture
3. **GraphQL**: Add GraphQL API alongside REST
4. **Caching**: Multi-layer caching strategy
5. **Search**: Full-text search with Elasticsearch
6. **Real-time**: WebSocket support for live updates
7. **Multi-tenancy**: Tenant isolation and routing

---

*Last Updated: 2025-01-25*
