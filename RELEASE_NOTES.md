## v0.1.5

- add dedicated products, product variants, and customers migrations plus order metadata columns, and mirror the schema bootstrap in SQLite so dev/test environments match production
- harden the test harness by resetting commerce/auth tables between runs and adding helpers to seed real catalog variants for end-to-end scenarios
- expand the comprehensive API smoke test to create orders from seeded products and assert both service-level and database reads for improved release confidence

## v0.1.4

- roll out a dedicated manufacturing schema with SeaORM entities, migrations, and a rebuilt bill of materials service that tracks components, audits, and work order readiness
- overhaul authentication by centralizing permission metadata, expanding API key handling, and tightening JWT/RBAC flows across handlers
- publish an API operations playbook and ship the `openapi_export` helper so teams can regenerate the OpenAPI spec alongside the release
- add an in-memory test harness with seeded catalog helpers to increase integration coverage for orders and inventory adjustments

## v0.1.3

- wire up SeaORM relationships between orders, shipments, payments, and products so downstream queries can eagerly load related records
- upgrade the reporting service to compute revenue from order line items and surface order counts by status for richer dashboards
- streamline the integration suite with a targeted relationship smoke test while modernizing shared middleware wiring

## v0.1.2

- ship the new `agentic_server` binary, docs, and demo tooling to showcase the delegated checkout experience
- expand core API features with Stablepay services, product feed automation, and updated returns/shipments flows
- add the outbox pattern migration plus helper scripts and follow-up timestamp migration for orders
- refresh integration coverage for inventory & returns endpoints to track the new behaviours

## v0.1.1

- add dedicated `migration` binary for running database migrations alongside the service
- keep Docker build cache warm by copying the `simple_api` manifest and providing stub binaries
- enable automatic database migrations by default in `config/default.toml`

## v0.0.1

- initial public release
