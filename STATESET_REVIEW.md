# StateSet API – Repository Review

## Snapshot
- Axum/Tokio service bootstrap wires configuration, persistence, caching, metrics, and rate-limiting layers before exposing the HTTP surface (`src/main.rs:16`).
- The crate is organized as a layered platform (handlers ⇄ services ⇄ repositories) with shared state encapsulated in `AppState` (`src/lib.rs:8`, `src/lib.rs:48`).
- gRPC support, outbox-driven events, and Redis-backed idempotency hooks are present alongside REST routing (`src/api.rs:1`, `src/events/outbox.rs:30`, `src/main.rs:135`).
- Documentation and demos cover onboarding and vertical use cases (e.g., `README.md`, `demos/`, `AGENTIC_OPS_PITCH.md`), signalling product scope beyond pure API code.

## Strengths
- Clear module boundaries make it easy to reason about where HTTP, business logic, and persistence belong (`src/lib.rs:8`).
- Service bootstrap composes telemetry, rate limiting, CORS, and idempotency middleware in one place (`src/main.rs:100`).
- The outbox worker and event abstractions illustrate aspirations for reliable async processing (`src/events/outbox.rs:63`).
- Metrics module exports Prometheus and JSON outputs, giving a base for platform observability (`src/metrics/mod.rs:18`).
- Rich documentation and demos shorten the learning curve for new contributors (`README.md`, `advanced_caching_demo/`, `START_HERE.md`).

## Key Findings

### Critical
- `Cargo.toml:13` pins `axum = "0.8"` (not yet released) and defines orphan dependency keys at the bottom (`Cargo.toml:119`), which makes `cargo metadata`/`cargo build` fail out of the gate.
- Repository-level database helpers misuse `AppError::DatabaseError` with string literals so they cannot compile (`src/repositories/order_repository.rs:126`, `src/repositories/order_repository.rs:167`).
- The login handler issues JWTs without validating credentials or checking stored hashes, effectively authorizing anyone (`src/auth/mod.rs:849`).

### High
- Tracing/OTel integration is commented out because of version drift, leaving only partial HTTP-level telemetry (`src/tracing/mod.rs:811`).
- Domain models still contain placeholder entities and TODO relations, so persistence integrity is uncertain (`src/entities/return_entity.rs:1`, `src/models/billofmaterials.rs:38`).
- Cache abstraction always falls back to in-memory because Redis support is disabled, negating distributed idempotency/caching claims (`src/cache/mod.rs:184`).
- Integration tests assume a running PostgreSQL instance on localhost, limiting reproducibility in CI and for new developers (`tests/integration_orders_test.rs:45`).

### Medium
- Auth roles/permissions come from ad-hoc environment overrides instead of the database, so enforcement will diverge across environments (`src/auth/mod.rs:363`, `src/auth/mod.rs:376`).
- Refresh tokens are never persisted or revoked; the API only logs operations, leaving session management incomplete (`src/auth/mod.rs:404`, `src/auth/mod.rs:418`).
- Numerous TODO markers across models/queries highlight unfinished features and make it hard to tell what is production-ready (`src/models/asn_items.rs:34`, `src/db/query_builder.rs:70`).
- Integration tests seed and clean data manually but skip transactional fixtures or SeaORM test helpers, increasing flakiness (`tests/integration_orders_test.rs:96`).

### Low
- Duplicate dependency declarations (`Cargo.toml:14`, `Cargo.toml:121`) and repeated chrono/regex entries add noise when auditing supply chain.
- Several modules contain verbose inline dictionaries (e.g., password policy word list) that could shift to data files or build-time assets for maintainability (`src/auth/password_policy.rs:89`).
- Metrics counters and histograms currently store integers only; lack of buckets or aggregation hints limits operational usability (`src/metrics/mod.rs:115`).
- Documentation overlaps (multiple README variants) risk drift without a clear single source of truth (`README.md`, `README_PLATFORM.md`, `README_SETUP.md`).

## Plan of Action

### 0–2 Weeks
1. Restore build health: align dependency versions with crates.io releases, remove stray manifest keys, and run `cargo check` across all binaries.
2. Fix compilation blockers in repositories/services, add unit coverage to prevent regressions on error handling surfaces.
3. Implement real authentication flows: persist users, hash/verify passwords (prefer Argon2id), and wire refresh-token storage for logout/rotation.
4. Harden CI/dev ergonomics: add a lightweight Postgres fixture (Docker compose profile or `testcontainers`) and ensure `make test` provisions it automatically.

### 2–6 Weeks
1. Finish core domain models: replace placeholder entities, backfill missing relations, and validate SeaORM migrations reflect the canonical schema.
2. Re-enable distributed cache/Redis paths (or drop them) and exercise idempotency middleware end-to-end with integration tests.
3. Resurrect tracing and OTLP exporters with consistent OpenTelemetry versions, propagating request IDs through handlers/services.
4. Introduce service-level contracts: OpenAPI coverage tests, gRPC compatibility checks, and property-based tests around inventory/order math.

### 6+ Weeks
1. Factor the event pipeline into dedicated worker binaries, support exponential backoff persistence, and add dead-letter queues for poison messages.
2. Layer feature flags/config management so verticals (StablePay, commerce, manufacturing) can be toggled without code edits.
3. Build automated quality gates (cargo fmt/clippy/test) plus security scanning into CI/CD, and publish release playbooks from existing docs.
4. Evaluate slicing the monolith into bounded contexts or crates to shorten compile times and clarify ownership.

## Additional Opportunities
- Consolidate onboarding docs into a single contributor guide, linking to demos and process files.
- Capture a migration strategy for SQLite ↔ PostgreSQL parity, including fixtures for both targets.
- Add dashboards/alerts leveraging the existing metrics endpoints to close the loop with operations.
- Track TODO debt in an issue board so owners can differentiate placeholders from intentional stubs.
