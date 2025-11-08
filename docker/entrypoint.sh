#!/bin/sh
# Minimal entrypoint for containerized deployments.
set -e

should_run_migrations() {
    value=$(printf "%s" "${1:-}" | tr '[:upper:]' '[:lower:]')
    case "$value" in
        1|true|yes)
            return 0
            ;;
        *)
            return 1
            ;;
    esac
}

if should_run_migrations "${RUN_MIGRATIONS_ON_START:-false}"; then
    echo "[entrypoint] Running database migrations before starting stateset-api"
    /app/migration
    echo "[entrypoint] Database migrations completed"
fi

exec "$@"
