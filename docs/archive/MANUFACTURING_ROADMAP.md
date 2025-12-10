# Manufacturing Roadmap

This note captures how we should slice the unfinished manufacturing surface and the schema work that must land before we wire new HTTP endpoints. The goal is to have well-scoped tickets that can move independently once the data model foundations are in place.

## Ticket Breakdown

### Manufacturing-01 · BOM command/service alignment
- **Scope**: Reconcile `CreateBOMCommand` and related handlers/services (`src/handlers/bom.rs`, `src/services/billofmaterials.rs`, `src/commands/billofmaterials/*`) with the final schema. Replace placeholder logic (e.g., blind inserts, missing revision handling) with SeaORM-backed flows that persist BOM revisions, components, and audit events.
- **Key Tasks**
  - Rebuild command DTOs so IDs/types match the new entities (UUIDs rather than i32s, component references stored as FK columns).
  - Implement CRUD against the finalized BOM tables, including soft-delete/versioning semantics and `Event::BOMCreated/Updated` dispatch.
  - Provide service-level pagination and filtering for list endpoints to satisfy handler contracts.
- **Definition of Done**
  - `cargo check` passes with handlers wired to the new command layer.
  - Sample flow (`POST /bom`, `GET /bom/{id}`, `PUT /bom/{id}`) works against a local database seeded with the new schema.
  - Logger and message bus hooks remain intact.
- **Dependencies**: Requires data model decisions (see below) and migrations in place.

### Manufacturing-02 · Work-order persistence & command layer
- **Scope**: Replace the stubbed work-order service (`src/services/work_orders.rs`) and handler scaffolding (`src/handlers/work_orders.rs`) with persistence backed by SeaORM entities that model scheduling, assignments, materials, and tasks.
- **Key Tasks**
  - Introduce commands for create/update/assign/start/complete operations that map to the new work-order tables.
  - Persist material allocations, task progress, and status transitions atomically (transaction boundary per command).
  - Emit the appropriate domain events for downstream consumers (inventory allocation, scheduling notifications).
- **Definition of Done**
  - Work-order API flows exercise the same schema the manufacturing execution layer will use.
  - Command logic enforces state transitions (e.g., cannot complete without being started).
  - Unit coverage for command validation and repository helpers in place.
- **Dependencies**: Relies on Manufacturing-01 schema + shared lookup tables (products, item master).

### Manufacturing-03 · Manufacturing aggregators
- **Scope**: Build read-focused aggregators that hydrate BOMs and work orders with related product, inventory, and progress data so API responses stay consistent with UI expectations.
- **Key Tasks**
  - Define query layer modules that join the manufacturing tables with `products`, `item_master`, reservation/balance snapshots, and audit trails.
  - Expose reusable view models to handlers so the API can render enriched payloads (e.g., component availability, task status).
  - Consider Redis caching or async message hydration once baseline queries are validated.
- **Definition of Done**
  - Aggregator modules return structured DTOs consumed by both BOM and work-order handlers.
  - Pagination/filtering wrappers exist for common list views (status, work center, late orders, etc.).
- **Dependencies**: Schema finalized and repositories established in Manufacturing-01/02.

### Manufacturing-04 · OpenAPI & HTTP exposure
- **Scope**: Once services stabilize, regenerate Utoipa schemas and surface the manufacturing endpoints via `src/lib.rs` routing.
- **Key Tasks**
  - Author `ToSchema`/`OpenApi` annotations that mirror the new response DTOs (including nested materials/tasks).
  - Wire the routes behind feature flags if we need staged rollout.
  - Update `openapi/` artifacts and swagger UI bundling.
- **Definition of Done**
  - `cargo check --features openapi` succeeds.
  - Swagger UI renders the new manufacturing section without schema errors.
- **Dependencies**: Aggregated DTOs completed (Manufacturing-03).

### Manufacturing-05 · Test & validation coverage
- **Scope**: Backfill unit/integration tests that protect the new manufacturing flows and provide sample fixtures for QA.
- **Key Tasks**
  - Add command/service unit tests (mock DB + real DB via SeaORM `MockDatabase` where possible).
  - Extend `tests/` integration harness with end-to-end BOM + work-order scenarios, including reservation edge cases and failure paths.
  - Capture migration smoke tests to ensure SQLite + Postgres compatibility.
- **Definition of Done**
  - `make test` passes with manufacturing suites enabled.
  - CI jobs seed the schema and run at least one end-to-end manufacturing scenario.
- **Dependencies**: All prior tickets; can run in parallel once command layers stabilize.

## Data Model Alignment

### Guiding Principles
- Use UUIDs for external API identifiers while maintaining foreign keys to existing ERP tables (`item_master`, `inventory_items`) needed for inventory integration.
- Collocate SeaORM entities under `src/entities/manufacturing/` (or similar) to avoid the current duplication between `models::billofmaterials` and legacy `bom_*` entities.
- Favor normalized tables for materials, tasks, and notes so we can reason about allocations and audit histories.

### Proposed Tables & Relationships

| Table | Key Columns | Notes |
| --- | --- | --- |
| `manufacturing_boms` | `id UUID PK`, `product_id UUID FK -> products.id`, `item_master_id BIGINT FK -> item_master.inventory_item_id`, `bom_number TEXT UNIQUE`, `revision TEXT`, `lifecycle_status TEXT`, `metadata JSONB`, timestamps | Bridges the product catalog and legacy item master. Keep both FKs so manufacturing can run even if a product has not been published. |
| `manufacturing_bom_components` | `id UUID PK`, `bom_id UUID FK -> manufacturing_boms.id`, `component_item_id BIGINT FK`, `component_product_id UUID FK (nullable)`, `quantity DECIMAL(18,6)`, `uom TEXT FK -> inventory_uoms`, `position TEXT`, `notes TEXT`, timestamps | Component rows drive material allocation. Store both product + item IDs when available. |
| `manufacturing_bom_audits` | `id UUID PK`, `bom_id UUID`, `event_type TEXT`, `user_id UUID`, `notes TEXT`, `event_at TIMESTAMP` | Supports audit trail invoked by the audit command. |
| `manufacturing_work_orders` | `id UUID PK`, `work_order_number TEXT UNIQUE`, `product_id UUID`, `bom_id UUID`, `scheduled_start TIMESTAMP`, `scheduled_end TIMESTAMP`, `status TEXT`, `priority TEXT`, `work_center_id UUID/TEXT`, `assigned_to UUID`, `quantity_to_build DECIMAL`, `quantity_completed DECIMAL`, timestamps | Canonical work-order header table. Tie to BOM revision captured at creation. |
| `manufacturing_work_order_materials` | `id UUID PK`, `work_order_id UUID`, `component_id UUID`, `reserved_quantity DECIMAL`, `consumed_quantity DECIMAL`, `inventory_reservation_id UUID (nullable)`, timestamps | Tracks material allocation lifecycle; ties into `inventory_reservations`. |
| `manufacturing_work_order_tasks` | `id UUID PK`, `work_order_id UUID`, `sequence INT`, `task_name TEXT`, `status TEXT`, `estimated_hours DECIMAL`, `actual_hours DECIMAL`, `assigned_to UUID`, `started_at TIMESTAMP`, `completed_at TIMESTAMP`, `notes TEXT` | Supports task progress, assignment, and reporting. |
| `manufacturing_work_order_notes` | `id UUID PK`, `work_order_id UUID`, `author_id UUID`, `body TEXT`, `created_at TIMESTAMP` | Optional, but keeps notes separate from task comments. |

### SeaORM & Model Actions
- Generate entities for the new tables and place them under an explicit manufacturing module (e.g., `src/entities/manufacturing/mod.rs`) to avoid namespace clashes.
- Update the command layer to depend on these entities instead of the legacy `models::billofmaterials` structs. Remove the outdated models once the new entities compile.
- Ensure the product domain exposes a lookup (either a `product.inventory_item_id` column or a join table) so manufacturing can resolve `item_master` references needed for inventory allocation.

### Migration Strategy
- Author forward migrations that create the manufacturing tables with proper indexes (unique constraint on `(product_id, revision)` for BOMs, composite keys for materials/tasks).
- Backfill data from any existing `bom_headers`/`bom_lines` tables via transitional migrations if needed, then deprecate the legacy structures or mark them read-only.
- Provide reversible down migrations so the schema stays compatible with SQLite (development) and Postgres (production).

### Contract Mapping
- Map API-layer DTOs to the new schema: external clients continue to send/receive UUIDs while internal logic resolves integer-based item IDs.
- Define enum types (`manufacturing_bom_status`, `manufacturing_work_order_status`, `manufacturing_work_order_priority`) shared between Rust enums and DB enums to enforce consistency.
- Document serialization expectations in OpenAPI so the UI team can prepare for the new payload shape (especially materials/tasks arrays).

With the tickets and schema decisions captured here, we can unblock implementation by first landing the migrations and SeaORM entities, then iterating through the Manufacturing-01 → Manufacturing-05 tickets.
