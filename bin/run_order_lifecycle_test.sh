#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"

echo "[orders-lifecycle] starting stateset-api server..."
(
  RUST_LOG=info cargo run --bin stateset-api
) &
SERVER_PID=$!

cleanup() {
  echo "[orders-lifecycle] stopping server (pid=$SERVER_PID)"
  kill "$SERVER_PID" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for server to become ready
for i in {1..30}; do
  if curl -sS "$BASE_URL/health" >/dev/null 2>&1; then
    echo "[orders-lifecycle] server is ready"
    break
  fi
  sleep 1
done

if ! curl -sS "$BASE_URL/health" >/dev/null 2>&1; then
  echo "[orders-lifecycle] server did not become ready in time" >&2
  exit 1
fi

echo "[orders-lifecycle] running bin/test_orders.sh against $BASE_URL"
BASE_URL="$BASE_URL" bash bin/test_orders.sh

echo "[orders-lifecycle] lifecycle test complete"

