# Repository Guidelines

## Project Structure & Module Organization
- Source: `src/` (HTTP in `handlers/`, services in `services/`, data access in `repositories/`, domain models in `models/`, DB entities in `entities/`).
- Entry points: `src/main.rs` (`stateset-api`) and additional binaries in `src/bin/` (e.g., `grpc_server.rs`, `minimal_server.rs`).
- Integrations: `proto/` (gRPC), `openapi/` (docs), `migrations/` (DB), `demos/` (end‑to‑end flows).
- Tests: integration tests in `tests/` (e.g., `integration_tests.rs`), unit tests colocated with modules via `#[cfg(test)]`.

## Build, Test, and Development Commands
- `make build`: build with logging via `build.sh`.
- `make test`: run tests with logging (`build.sh --with-tests`).
- `cargo run --bin stateset-api`: start the main API locally.
- `cargo build --release`: optimized release build.
- `docker-compose up -d`: spin up `stateset-api` + Redis (see `docker-compose.yml`).
- `make logs` / `make tail-logs`: view or tail `build_errors.log`.

## Coding Style & Naming Conventions
- Formatting: `cargo fmt` (4‑space indent, Rust defaults).
- Linting: `cargo clippy -- -D warnings` (fix all lints before PR).
- Naming: `snake_case` for files/functions, `CamelCase` for types, `SCREAMING_SNAKE_CASE` for consts.
- Modules: prefer small, focused modules; keep HTTP types in `handlers/` and business logic in `services/`.

## Testing Guidelines
- Run: `cargo test` or `make test`.
- Layout: unit tests inline with modules; integration tests in `tests/` (name with clear intent, e.g., `inventory_adjustment_test.rs`).
- Scope: cover happy paths, errors, and edge cases; mock I/O where possible.
- Optional coverage: `cargo tarpaulin` (if installed locally).

## Commit & Pull Request Guidelines
- Commits: follow Conventional Commits (e.g., `feat(orders): add bulk import`).
- Before pushing: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.
- PRs: clear description, linked issues (e.g., `Fixes #123`), test plan, and any relevant logs or screenshots of API responses.
- Keep changes scoped; include migrations and docs updates when applicable.

## Security & Configuration Tips
- Configuration: use `.env` (see `ENV_VARIABLES.md`). Never commit secrets.
- Local stack: `docker-compose` provides Redis; DB URL defaults to SQLite unless overridden.
- Review `SECURITY.md` before exposing services publicly.

