# Orders gRPC / Protobuf API Overview

This guide explains how to integrate with the StateSet gRPC API for creating and retrieving orders using Protocol Buffers.

## Where the Protos Live

- Protobuf definitions are in `proto/`.
- Order definitions are in `proto/order.proto` (package `stateset.order`).
- Shared/common types (money, addresses, pagination) are in `proto/common.proto` (package `stateset.common`).

## Running & Connecting to the gRPC Server

StateSet ships a dedicated gRPC server binary.

```bash
cargo run --bin grpc-server
```

By default it listens on:

- `grpc://{host}:{grpc_port}`
- `grpc_port` defaults to HTTP `port + 1` (e.g., `8080` → `8081`)
- Configure via `grpc_port` in config files or env var `APP__GRPC_PORT`.

## OrderService (gRPC)

The Order API is defined as a unary gRPC service:

```
service OrderService {
  rpc CreateOrder(CreateOrderRequest) returns (CreateOrderResponse);
  rpc GetOrder(GetOrderRequest) returns (GetOrderResponse);
  rpc UpdateOrderStatus(UpdateOrderStatusRequest) returns (UpdateOrderStatusResponse);
  rpc ListOrders(ListOrdersRequest) returns (ListOrdersResponse);
}
```

### Messages

**Order**

`Order` is the main entity used for creates and responses:

- `id` (string): UUID of the order (server-generated on create).
- `customer_id` (string): UUID of the customer placing the order.
- `items` (repeated `OrderItem`): line items.
- `total_amount` (`stateset.common.Money`): total in smallest currency units.
- `status` (`OrderStatus`): lifecycle status.
- `created_at` (`google.protobuf.Timestamp`): creation timestamp.
- `shipping_address` / `billing_address` (`stateset.common.Address`): optional addresses.
- `payment_method_id` (string): payment method reference.
- `shipment_id` (string): shipment/tracking reference.

**OrderItem**

- `product_id` (string): UUID of the product.
- `quantity` (int32): quantity ordered.
- `unit_price` (`stateset.common.Money`): per‑unit price.

**Money**

`stateset.common.Money` uses:

- `currency` (string): ISO‑4217 code (e.g., `USD`).
- `amount` (int64): smallest unit (e.g., cents). Example: `$19.99` → `1999`.

**Pagination**

List calls use:

- `PaginationRequest { page, per_page }`
- `PaginationResponse { total_items, total_pages, current_page, items_per_page, has_next_page, has_previous_page }`

### Create an Order

Send an `Order` inside `CreateOrderRequest`. At minimum you should provide:

- `customer_id`
- `items` (with `product_id`, `quantity`, `unit_price`)
- `total_amount`

Example using `grpcurl` (reflection is not enabled, so pass protos):

```bash
grpcurl -plaintext \
  -import-path proto \
  -proto order.proto \
  localhost:8081 \
  stateset.order.OrderService/CreateOrder \
  '{
    "order": {
      "customer_id": "11111111-1111-1111-1111-111111111111",
      "items": [
        {
          "product_id": "22222222-2222-2222-2222-222222222222",
          "quantity": 1,
          "unit_price": { "currency": "USD", "amount": 1999 }
        }
      ],
      "total_amount": { "currency": "USD", "amount": 1999 }
    }
  }'
```

Response: `CreateOrderResponse { order_id, status, created_at }`.

### Get an Order

```bash
grpcurl -plaintext \
  -import-path proto \
  -proto order.proto \
  localhost:8081 \
  stateset.order.OrderService/GetOrder \
  '{ "order_id": "..." }'
```

Response: `GetOrderResponse { order }`.

### Update Order Status

```bash
grpcurl -plaintext \
  -import-path proto \
  -proto order.proto \
  localhost:8081 \
  stateset.order.OrderService/UpdateOrderStatus \
  '{ "order_id": "...", "new_status": "SHIPPED" }'
```

### List Orders

List supports optional filters + pagination:

- `customer_id`
- `status`
- `start_date` / `end_date` (timestamps)
- `pagination`

```bash
grpcurl -plaintext \
  -import-path proto \
  -proto order.proto \
  localhost:8081 \
  stateset.order.OrderService/ListOrders \
  '{
    "customer_id": "11111111-1111-1111-1111-111111111111",
    "status": "PENDING",
    "pagination": { "page": 1, "per_page": 20 }
  }'
```

Response: `ListOrdersResponse { orders, pagination }`.

## Generating Client Code

Use `proto/*.proto` as your source of truth.

### Go

```bash
protoc --go_out=. --go-grpc_out=. proto/*.proto
```

### Python

```bash
python -m grpc_tools.protoc \
  -I proto \
  --python_out=. \
  --grpc_python_out=. \
  proto/*.proto
```

### Node.js / TypeScript

```bash
protoc \
  -I proto \
  --js_out=import_style=commonjs,binary:. \
  --grpc_out=grpc_js:. \
  proto/*.proto
```

### Java / Kotlin

```bash
protoc -I proto \
  --java_out=. \
  --grpc-java_out=. \
  proto/*.proto
```

## Current Implementation Notes

- Orders gRPC endpoints are unary-only (no streaming).
- The server currently persists a subset of Order fields on `CreateOrder` (customer, totals, payment method). Line items and addresses may not round-trip yet via gRPC. If you need full fidelity today, use the REST API and/or coordinate on gRPC expansion.

## Related Protos / Services

Other gRPC services are available alongside orders:

- Inventory: `proto/inventory.proto` → `stateset.inventory.InventoryService`
- Shipments: `proto/shipment.proto` → `stateset.shipment.ShipmentService`
- Returns: `proto/return.proto` → `stateset.return.ReturnService`
- Work Orders: `proto/work_order.proto` → `stateset.work_order.WorkOrderService`

