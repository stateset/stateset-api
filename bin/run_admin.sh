#!/usr/bin/env bash
set -euo pipefail

# Convenience runner: start API with admin/test permissions enabled.
# Useful for local development and end-to-end testing.

export AUTH_ADMIN=${AUTH_ADMIN:-true}
export STATESET_AUTH_ALLOW_ADMIN_OVERRIDE=${STATESET_AUTH_ALLOW_ADMIN_OVERRIDE:-true}
# Ensure write/update/delete are available for tests
export AUTH_DEFAULT_PERMISSIONS=${AUTH_DEFAULT_PERMISSIONS:-orders:write,orders:update,orders:delete}

export RUST_LOG=${RUST_LOG:-info}

echo "Starting stateset-api with: AUTH_ADMIN=$AUTH_ADMIN STATESET_AUTH_ALLOW_ADMIN_OVERRIDE=$STATESET_AUTH_ALLOW_ADMIN_OVERRIDE AUTH_DEFAULT_PERMISSIONS=$AUTH_DEFAULT_PERMISSIONS"
exec cargo run --bin stateset-api "$@"
