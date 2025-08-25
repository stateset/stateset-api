# Makefile for StateSet API with build error logging

.PHONY: build build-offline build-release build-release-offline test test-offline clean run logs help

# Default target
help:
	@echo "Available targets:"
	@echo "  build         - Build the project in debug mode (with error logging)"
	@echo "  build-release - Build the project in release mode (with error logging)"
	@echo "  test          - Run tests (with error logging)"
	@echo "  clean         - Clean build artifacts and logs"
	@echo "  run           - Run the main server"
	@echo "  run-admin     - Run server with admin/test permissions"
	@echo "  logs          - View build error logs"
	@echo "  tail-logs     - Tail build error logs"
	@echo "  smoke         - Run API smoke tests (server must be running)"
	@echo "  test-orders   - Run order endpoint tests"
	@echo "  test-returns  - Run returns endpoint tests"
	@echo "  test-shipments- Run shipments endpoint tests"

# Build in debug mode with error logging
build:
	@echo "Building project (debug mode)..."
	@./build.sh

# Build in debug mode offline (no network)
build-offline:
	@echo "Building project (debug mode, offline)..."
	@CARGO_OFFLINE=1 ./build.sh

# Build in release mode with error logging
build-release:
	@echo "Building project (release mode)..."
	@echo "[`date '+%Y-%m-%d %H:%M:%S'`] ===== Release build started =====" >> build_errors.log
	@cargo build --release 2>&1 | tee -a build_errors.log; \
	if [ $${PIPESTATUS[0]} -eq 0 ]; then \
		echo "[`date '+%Y-%m-%d %H:%M:%S'`] Release build completed successfully" >> build_errors.log; \
		echo "✅ Release build successful!"; \
	else \
		echo "[`date '+%Y-%m-%d %H:%M:%S'`] Release build failed with exit code: $${PIPESTATUS[0]}" >> build_errors.log; \
		echo "❌ Release build failed! Check build_errors.log for details."; \
		exit $${PIPESTATUS[0]}; \
	fi

# Build in release mode offline (no network)
build-release-offline:
	@echo "Building project (release mode, offline)..."
	@echo "[`date '+%Y-%m-%d %H:%M:%S'`] ===== Release build started (offline) =====" >> build_errors.log
	@cargo build --release --offline 2>&1 | tee -a build_errors.log; \
	if [ $${PIPESTATUS[0]} -eq 0 ]; then \
		echo "[`date '+%Y-%m-%d %H:%M:%S'`] Release build completed successfully (offline)" >> build_errors.log; \
		echo "✅ Release build successful (offline)!"; \
	else \
		echo "[`date '+%Y-%m-%d %H:%M:%S'`] Release build failed with exit code: $${PIPESTATUS[0]} (offline)" >> build_errors.log; \
		echo "❌ Release build failed (offline)! Check build_errors.log for details."; \
		exit $${PIPESTATUS[0]}; \
	fi

# Run tests with error logging
test:
	@echo "Running tests..."
	@./build.sh --with-tests

# Run tests in offline mode
test-offline:
	@echo "Running tests (offline)..."
	@CARGO_OFFLINE=1 ./build.sh --with-tests

# Clean build artifacts
clean:
	@echo "Cleaning build artifacts..."
	@cargo clean
	@echo "Build artifacts cleaned."

# Run the main server
run:
	@echo "Running StateSet API server..."
	@cargo run --bin stateset-api

.PHONY: run-admin
run-admin:
	@echo "Running StateSet API server with admin/test permissions..."
	@bash bin/run_admin.sh

# View build logs
logs:
	@if [ -f build_errors.log ]; then \
		cat build_errors.log; \
	else \
		echo "No build logs found."; \
	fi

# Tail build logs
tail-logs:
	@if [ -f build_errors.log ]; then \
		tail -f build_errors.log; \
	else \
		echo "No build logs found."; \
	fi

# Run API smoke tests against localhost
.PHONY: smoke
smoke:
	@bash bin/smoke.sh

.PHONY: test-orders
test-orders:
	@bash bin/test_orders.sh

.PHONY: test-returns
test-returns:
	@bash bin/test_returns.sh

.PHONY: test-shipments
test-shipments:
	@bash bin/test_shipments.sh

# Build with the build-logger binary
build-with-logger:
	@echo "Building with build-logger..."
	@cargo run --bin build-logger -- build 
