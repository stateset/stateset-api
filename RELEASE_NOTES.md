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
