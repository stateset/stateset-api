#!/bin/sh
# Minimal entrypoint for containerized deployments.
set -e

is_enabled() {
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

if is_enabled "${RUN_MIGRATIONS_ON_START:-false}"; then
    echo "[entrypoint] Running database migrations before starting stateset-api"
    /app/migration
    echo "[entrypoint] Database migrations completed"
fi

if is_enabled "${SEED_DATA_ON_START:-false}"; then
    echo "[entrypoint] Seeding database with demo data"
    /app/seed-data
    echo "[entrypoint] Database seeding completed"
fi

exec "$@"
