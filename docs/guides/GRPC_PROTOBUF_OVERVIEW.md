# gRPC & Protocol Buffers Overview (StateSet Services)

This guide gives a high‑level overview of gRPC and Protocol Buffers (Protobuf), and outlines how StateSet services—especially the Order Service and the Sync Server—should communicate internally.

## gRPC (what it is and why we use it)

gRPC is a high‑performance Remote Procedure Call (RPC) framework for service‑to‑service communication. It runs over HTTP/2 and typically uses Protobuf for message encoding.

Why gRPC is a good fit for StateSet:

- **Fast and efficient**: Protobuf’s binary encoding + HTTP/2 multiplexing reduce latency and overhead compared to REST/JSON.
- **Strong API contracts**: `.proto` files define services and messages once, then code is generated for clients and servers.
- **Streaming support**: server‑streaming, client‑streaming, and bidirectional streams make real‑time state propagation natural.
- **Cross‑language**: Go, Rust, TypeScript, Python, Java, etc. can all share the same contract.
- **Operational features built‑in**: deadlines/timeouts, retries, load balancing, auth metadata, and standardized status codes.

## Protocol Buffers (Protobuf) basics

Protobuf is both:

1. An Interface Definition Language (IDL) for defining messages and services in `.proto` files.
2. A compact binary serialization format used by gRPC.

Core concepts:

- **Messages** are typed records with numbered fields.
  - Field numbers are part of the wire contract: keep them stable.
  - Adding new optional fields is backward‑compatible.
  - Removing fields or reusing numbers is breaking.
- **Services** define RPC methods and their request/response message types.
- **Packages & versioning** (e.g., `stateset.order.v1`) allow safe evolution over time.
- **Codegen**: `protoc` (or Buf) generates server stubs and client libraries.

In this repo:

- Protobuf definitions live in `proto/`.
- Order‑related protos are currently in `proto/order.proto` and use the `stateset.order` package.

## Communicating between StateSet services

StateSet is a multi‑service system where each service owns a domain. gRPC + Protobuf provide a shared, strongly‑typed contract for internal communication.

### Shared contracts and ownership

- Each domain service owns its API definitions (e.g., Order Service owns `stateset.order.v1`).
- Common types (money, pagination, addresses, timestamps) live in shared protos (e.g., `stateset.common.v1`).
- Other services import these protos rather than redefining schemas.

### RPC patterns to use

Use the right RPC type for each interaction:

- **Unary RPCs (request/response)** for commands and reads.
  - Examples: `PlaceOrder`, `CancelOrder`, `GetOrder`, `ListOrders`.
- **Server‑streaming RPCs** for continuous event feeds.
  - Examples: `StreamOrderEvents`, `StreamInventoryEvents`.
- **Bidirectional streaming RPCs** for real‑time subscriptions with back‑pressure or acks.
  - Example: Sync Server sends subscription/ack messages while Order Service streams events back.
- **Client‑streaming RPCs** for batching writes or checkpoints (less common, but useful when Sync batches updates).

### Order Service → Sync Server flow

The Sync Server’s job is to keep other services and external clients up‑to‑date with authoritative state changes.

Recommended flow:

1. **Sync Server subscribes** to the Order Service via a streaming RPC (server or bidi streaming).
2. **Order Service publishes events** on every order state transition (accepted, partially filled, shipped, canceled, etc.).
3. **Sync Server processes events**:
   - updates its cache or read‑optimized store,
   - forwards events to WebSocket/HTTP clients,
   - optionally fan‑outs to other internal services.
4. **Acknowledgements/checkpoints (optional)**:
   - Sync Server can send acks/offsets so Order Service can retry or compact history.

This model avoids polling, keeps latency low, and provides a single authoritative source of truth.

### Other internal services

Services such as Risk, Matching, Portfolio, Analytics, and Product can:

- Call unary Order RPCs to fetch authoritative order data when needed.
- Subscribe to order/event streams to stay synchronized without periodic polling.

If some consumers can’t speak gRPC directly, expose a gRPC‑Gateway (HTTP/JSON) on top of the same `.proto` contracts.

### Operational considerations

To make these integrations reliable:

- **Auth and transport security**: use TLS/mTLS between internal services; pass identity/roles in gRPC metadata.
- **Deadlines**: every RPC should set a timeout; servers should honor cancellations.
- **Idempotency**: command RPCs should accept client‑generated IDs so retries are safe.
- **Versioning discipline**: only add fields; never reuse numbers; introduce a new `v2` package for breaking semantic changes.
- **Observability**: use interceptors/middleware for logging, tracing, and metrics; map errors to standard gRPC statuses (`INVALID_ARGUMENT`, `NOT_FOUND`, etc.).

## Next steps

- If you want to formalize Order → Sync streaming, add a `StreamOrderEvents` (server‑streaming or bidi) method to the OrderService `.proto`, then implement it in the gRPC server binary.
- Consider centralizing shared `.proto` packages and enabling CI checks for backward compatibility.

