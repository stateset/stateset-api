# StateSet API Operations Overview

This guide summarizes the HTTP APIs exposed by `stateset-api` so that operations, support, and integrations teams can reason about day-to-day usage, runbooks, and troubleshooting.

## Environments and Documentation

- **Production**: `https://api.stateset.com/v1`
- **Staging**: `https://staging-api.stateset.com/v1`
- **Local development**: `http://localhost:8080/api/v1`
- **Interactive docs**: `GET /swagger-ui` (serves the OpenAPI UI backed by `GET /api-docs/openapi.json`)
- **Version**: Published as `StateSet API 0.1.4` (see OpenAPI info block in `src/openapi/mod.rs`)

All endpoints below are rooted at `/api/v1` unless otherwise noted.

## Authentication and Authorization

- **Access tokens**: Issue JSON Web Tokens (JWT) using the authentication service (`POST /api/v1/auth/login`, see `README.md#Authentication` for full flow). Send every request with `Authorization: Bearer <access_token>`.
- **API keys**: Supported for service-to-service integrations. Provide `X-API-Key: <key>` and omit the bearer token.
- **Permissions**: Routes are permission-gated via `AuthRouterExt::with_permission`. Key strings live in `src/auth/permissions.rs` (for example, `orders:read`, `inventory:adjust`, `payments:write`). Operations users will typically be assigned a role that bundles the permissions called out per endpoint below.
- **User context**: Successful authentication injects an `AuthUser`/`AuthenticatedUser` into handlers, enabling permission checks and audit trails.

## Rate Limiting, Idempotency, and Headers

- **Rate limits**: Defaults to 100 requests per 60 seconds per identity (`DEFAULT_RATE_LIMIT_REQUESTS`, `DEFAULT_RATE_LIMIT_WINDOW_SECS` in `src/config.rs`). Overrides can be configured per API key, user, or path prefix. Successful responses include headers:
  - `X-RateLimit-Limit`
  - `X-RateLimit-Remaining`
  - `X-RateLimit-Reset`
  - Newer deployments may also emit the standard `RateLimit-*` trio.
- **Idempotency**: Mutating requests (`POST`, `PUT`, `PATCH`, `DELETE`) honor the `Idempotency-Key` header (`src/middleware_helpers/idempotency_redis.rs`). Supply a unique key per logical operation to make retries safe; cached responses are stored in Redis for 10 minutes.
- **Tracing**: Every request can carry/returns `X-Request-Id`. Echo this value in support tickets to cross-reference logs.

## Standard Response Envelope

Handlers return `ApiResponse<T>` (`src/lib.rs`):

```json
{
  "success": true,
  "data": {},
  "message": null,
  "errors": null
}
```

- Errors flip `success` to `false`, populate `message`, and may include an `errors` array.
- Paginated endpoints wrap results in `PaginatedResponse<T>` with `items`, `total`, `page`, `limit`, and `total_pages`.

## Pagination and Filtering

- Common list query parameters (`ListQuery` in `src/lib.rs`): `page`, `limit`, `search`, `sort_by`, `sort_order`.
- Domain-specific filters exist (for example `InventoryFilters`, `ReturnListQuery`, `ShipmentListQuery`). See per-endpoint notes below.

## Domain API Summary

### Orders (handlers in `src/handlers/orders.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/orders` | Paginated order list with optional `status`, `customer_id`, `search` filters. | `orders:read` |
| GET | `/orders/{id}` | Fetch order by UUID. | `orders:read` |
| GET | `/orders/by-number/{order_number}` | Fetch order by public order number (e.g. `ORD-12345`). | `orders:read` |
| GET | `/orders/{id}/items` | List items attached to an order. | `orders:read` |
| POST | `/orders` | Create a new order from customer and line item payload. | `orders:create` |
| PUT | `/orders/{id}` | Update addresses, payment method, or notes (currently returns staged data). | `orders:update` |
| PUT | `/orders/{id}/status` | Transition order status with optional reason (maps to service status codes). | `orders:update` |
| POST | `/orders/{id}/items` | Append an item (SKU or variant UUID) to an existing order. | `orders:update` |
| POST | `/orders/{id}/archive` | Archive an order (soft close). | `orders:update` |
| POST | `/orders/{id}/cancel` | Cancel an order with optional `reason`. | `orders:cancel` |
| DELETE | `/orders/{id}` | Permanently delete order (uses underlying service delete). | `orders:delete` |

Operational notes:
- SKU validation happens against the product catalog (`resolve_variant_identifier`).
- Order totals are recalculated server-side; mismatched line pricing returns validation errors.
- Use `Idempotency-Key` when creating or cancelling orders to protect against retried requests.

### Inventory (handlers in `src/handlers/inventory.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/inventory` | Paginated view of aggregated inventory (`InventoryFilters` supports `product_id`, `location_id`, `low_stock`, `limit`, `offset`). | `inventory:read` |
| GET | `/inventory/{id}` | Fetch by inventory item ID or item number. | `inventory:read` |
| GET | `/inventory/low-stock` | Convenience endpoint that forces `low_stock=true` with optional `threshold`. | `inventory:read` |
| POST | `/inventory` | Create inventory item and optionally seed on-hand quantity. | `inventory:adjust` |
| PUT | `/inventory/{id}` | Update item metadata and/or set on-hand quantity at a location. | `inventory:adjust` |
| POST | `/inventory/{id}/reserve` | Reserve quantity at a location (`ReserveInventoryRequest`). | `inventory:adjust` |
| POST | `/inventory/{id}/release` | Release previously reserved inventory. | `inventory:adjust` |
| DELETE | `/inventory/{id}` | Zero out and remove inventory item across locations. | `inventory:adjust` |

Operational notes:
- Quantities are represented as strings in responses for precision; use decimal-safe parsing.
- Reservation endpoints support optional `reference_id` (UUID) and `reference_type` to tie back to orders or work orders.

### Returns (handlers in `src/handlers/returns.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/returns` | Paginated list (`page`, `limit`, optional `status`). | `returns:read` |
| GET | `/returns/{id}` | Fetch single return. | `returns:read` |
| POST | `/returns` | Initiate an RMA for an order (`order_id`, `reason`). | `returns:create` |
| POST | `/returns/{id}/approve` | Approve a pending return. | `returns:create` |
| POST | `/returns/{id}/restock` | Trigger restocking workflow (publishes events). | `returns:create` |

Operational notes:
- Restock requests enqueue work onto the returns service pipeline and may interact with inventory reservations.
- Rejection endpoints are scaffolded but currently disabled.

### Shipments (handlers in `src/handlers/shipments.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/shipments` | Paginated list with optional `status` filter. | `shipments:read` |
| GET | `/shipments/{id}` | Fetch by shipment UUID. | `shipments:read` |
| GET | `/shipments/{id}/track` | Request live tracking update via service command. | `shipments:read` |
| GET | `/shipments/track/{tracking_number}` | Lookup by carrier tracking number. | `shipments:read` |
| POST | `/shipments` | Create shipment for an order (`shipping_method`, `tracking_number`, etc.). | `shipments:update` |
| POST | `/shipments/{id}/ship` | Mark as shipped (sets timestamps, status). | `shipments:update` |
| POST | `/shipments/{id}/deliver` | Mark as delivered. | `shipments:update` |

Operational notes:
- Shipping methods are validated against the enumerated set defined in `shipment::ShippingMethod`.
- Downstream carrier polling leverages Redis-backed circuit breakers; failures bubble up as `ServiceError`.

### Payments (handlers in `src/handlers/payments.rs` and `payment_webhooks.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| POST | `/payments` | Process payment against order (`CreatePaymentRequest`). | `payments:access` + `payments:write` |
| GET | `/payments/{payment_id}` | Fetch single payment. | `payments:access` + `payments:read` |
| GET | `/payments/order/{order_id}` | List payments for an order. | `payments:access` + `payments:read` |
| GET | `/payments/order/{order_id}/total` | Return aggregate amount paid for an order. | `payments:access` + `payments:read` |
| GET | `/payments` | Paginated list with optional `status` filter plus `page`, `per_page`. | `payments:access` + `payments:read` |
| POST | `/payments/refund` | Issue refund (`payment_id`, optional partial `amount`). | `payments:access` + `payments:write` |
| POST | `/payments/webhook` | Ingest external payment events. | Signature-protected (no auth header) |

Operational notes:
- Supported `payment_method` values: `credit_card`, `debit_card`, `paypal`, `bank_transfer`, `cash`, `check`.
- The webhook verifies either `x-timestamp`/`x-signature` with HMAC-SHA256 or Stripe-style `Stripe-Signature`. Configure `payment_webhook_secret` and optional tolerance in app config.
- Payments service emits events to the outbox (`PaymentSucceeded`, `PaymentFailed`, `PaymentRefunded`).

### Warranties (handlers in `src/handlers/warranties.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/warranties` | Paginated list (filter by `status`). | `warranties:read` |
| GET | `/warranties/{id}` | Fetch warranty details. | `warranties:read` |
| POST | `/warranties` | Register new warranty (product, customer, terms). | `warranties:create` |
| POST | `/warranties/{id}/extend` | Extend warranty duration by `additional_months`. | `warranties:update` |
| POST | `/warranties/claims` | File warranty claim. | `warranties:update` |
| POST | `/warranties/claims/{id}/approve` | Approve outstanding warranty claim. | `warranties:update` |

### Work Orders (handlers in `src/handlers/work_orders.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/work-orders` | List work orders (supports rich `WorkOrderFilters`). | `workorders:read` |
| GET | `/work-orders/{id}` | Fetch work order. | `workorders:read` |
| POST | `/work-orders` | Create new work order (quantity, priority, scheduling). | `workorders:create` |
| PUT | `/work-orders/{id}` | Update metadata (status, priority, assignments). | `workorders:update` |
| POST | `/work-orders/{id}/assign` | Assign to resource/work center. | `workorders:update` |
| POST | `/work-orders/{id}/complete` | Mark as complete. | `workorders:update` |
| PUT | `/work-orders/{id}/status` | Explicit status update payload. | `workorders:update` |
| DELETE | `/work-orders/{id}` | Remove work order. | `workorders:delete` |

Operational notes:
- Current implementation returns rich mock data for some endpoints while services are finalized. Treat as contract for upcoming persistence-backed version.

### Commerce: Products, Carts, Checkout, Customers (handlers in `src/handlers/commerce/`)

| Method | Path | Purpose | Notes / Permissions |
| --- | --- | --- | --- |
| GET | `/products` | Search and paginate catalog (`per_page`, `page`). | Requires authenticated context (`AuthenticatedUser`). |
| POST | `/products` | Create product (name, slug, description). | Needs user with product management role. |
| GET | `/products/{id}` | Fetch product by UUID. |  |
| GET | `/products/{id}/variants` | List product variants. |  |
| POST | `/products/{id}/variants` | Create variant (SKU, price). |  |
| PUT | `/products/variants/{variant_id}/price` | Update variant pricing. |  |
| GET | `/products/search` | Text search via `ProductSearchQuery`. |  |
| POST | `/carts` | Create cart (session/customer scoped). |  |
| GET | `/carts/{id}` | Retrieve cart with items. |  |
| POST | `/carts/{id}/items` | Add SKU/variant to cart. | Validates quantity >= 1. |
| PUT | `/carts/{id}/items/{item_id}` | Update line quantity. |  |
| DELETE | `/carts/{id}/items/{item_id}` | Remove line item. |  |
| POST | `/carts/{id}/clear` | Empty cart. |  |
| POST | `/checkout` | Start checkout from cart (`StartCheckoutRequest`). | Persists session in Redis. |
| GET | `/checkout/{session_id}` | Load checkout session. |  |
| PUT | `/checkout/{session_id}/customer` | Attach customer info. |  |
| PUT | `/checkout/{session_id}/shipping-address` | Provide shipping address. |  |
| PUT | `/checkout/{session_id}/shipping-method` | Select shipping rate. |  |
| POST | `/checkout/{session_id}/complete` | Complete checkout with payment token. | Returns order payload. |
| POST | `/customers/register` | Register storefront customer. |  |
| POST | `/customers/login` | Customer login (returns tokens plus profile). |  |
| GET | `/customers/me` | Get authenticated customer profile. | Requires customer JWT. |
| PUT | `/customers/me` | Update profile. |  |
| GET | `/customers/me/addresses` | List customer addresses. |  |
| POST | `/customers/me/addresses` | Add shipping/billing address. |  |

### Agentic Checkout API (handlers in `src/handlers/commerce/agentic_checkout.rs`)

| Method | Path | Purpose | Notes |
| --- | --- | --- | --- |
| POST | `/checkout_sessions` | Create conversational checkout session (`CheckoutSessionCreateRequest`). | Honors `Idempotency-Key` and echoes it back. |
| GET | `/checkout_sessions/{checkout_session_id}` | Load session state. |  |
| POST | `/checkout_sessions/{checkout_session_id}` | Update session items, addresses, or metadata. |  |
| POST | `/checkout_sessions/{checkout_session_id}/complete` | Finalize session and issue order. |  |
| POST | `/checkout_sessions/{checkout_session_id}/cancel` | Cancel an in-flight session. | Returns session snapshot. |

### Agents API (handlers in `src/handlers/agents.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/agents/recommendations` | Product recommendation feed (filters: `page`, `per_page`, `search`, `is_active`). | `agents:access` |
| POST | `/agents/customers/{customer_id}/carts/{cart_id}/items` | Agent adds item to a customer cart. | `agents:access` |

### Admin Outbox (handlers in `src/handlers/outbox_admin.rs`)

| Method | Path | Purpose | Required permission(s) |
| --- | --- | --- | --- |
| GET | `/admin/outbox` | Inspect recent outbox events (pending, failed, processing). | `admin:outbox` |
| POST | `/admin/outbox/{id}/retry` | Reset an outbox record to `pending` for reprocessing. | `admin:outbox` |

### Observability and Service Health

| Method | Path | Purpose | Notes |
| --- | --- | --- | --- |
| GET | `/` | Plain-text heartbeat (`stateset-api up`). | No auth. |
| GET | `/metrics` | Prometheus text format metrics (includes HTTP latency, rate limiting, business counters). | No auth by default; secure via ingress if needed. |
| GET | `/metrics/json` | Metrics rendered as JSON. | |
| GET | `/api/v1/status` | High-level service metadata (`version`, `git`, `timestamp`). | Wrapped in `ApiResponse`. |
| GET | `/api/v1/health` | Composite health check (DB ping, Redis ping). | Returns status per dependency. |
| GET | `/health` and `/health/detailed` | Legacy health routes from `src/handlers/health.rs`; detailed variant verifies DB connectivity and timing. | Mounted separately when health router is used. |

### Webhooks and Events

- **Payment webhook** (`POST /api/v1/payments/webhook`):
  - Verify HMAC signature via `x-timestamp` + `x-signature` headers or Stripe `Stripe-Signature`.
  - Duplicate detection uses Redis to guard against replay of identical event IDs for 24 hours.
  - Enqueues outbox events for asynchronous processing (`PaymentSucceeded`, `PaymentFailed`, `PaymentRefunded`).
- **Event outbox** (`GET /api/v1/admin/outbox`) can be queried to confirm downstream integrations have materialized.

## Typical Operations Workflows

1. **Order fulfillment**: Create order → monitor status → allocate inventory (`reserve_inventory`) → create shipment → mark shipped/delivered → capture payment (or reconcile via `get_order_payment_total`).
2. **Return and restock**: Approve return → trigger `restock` → confirm inventory adjustments via `/inventory/{id}`.
3. **Warranty claim**: Locate warranty → record claim → approve or extend as needed.
4. **Payment reconciliation**: Use `GET /payments` (filter by `status`) and `GET /payments/order/{order_id}/total`; reconcile webhook-driven status updates via outbox inspection.

## Support and Escalation

- Contact: `support@stateset.com`
- Include `X-Request-Id` (or request timestamp and path) when filing tickets.
- For authorization issues, verify granted permissions against `src/auth/permissions.rs`.

Keep this document alongside the OpenAPI schema for day-to-day reference; update it whenever handlers or permissions change.
