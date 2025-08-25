#!/bin/bash

set -euo pipefail

# Build script with error logging
# This script runs cargo build and logs any errors to build_errors.log

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Log file
BUILD_LOG="build_errors.log"

# Function to log with timestamp
log_with_timestamp() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" >> "$BUILD_LOG"
}

# Function to check Rust version
check_rust_version() {
    echo -e "${YELLOW}Checking Rust version...${NC}"
    RUST_VERSION=$(rustc --version)
    echo "Current Rust version: $RUST_VERSION"
    log_with_timestamp "Rust version: $RUST_VERSION"
    
    # Check if we're on stable or nightly
    if rustc --version | grep -q "nightly"; then
        echo -e "${GREEN}Running on Rust nightly${NC}"
        log_with_timestamp "Running on Rust nightly"
    else
        echo -e "${YELLOW}Running on Rust stable${NC}"
        log_with_timestamp "Running on Rust stable"
    fi
}

# Start logging
echo -e "${YELLOW}Starting build process...${NC}"
log_with_timestamp "===== Build started ====="

# Check Rust version
check_rust_version

## Determine cargo args (support offline builds)
OFFLINE_FLAG=""
if [[ "${CARGO_OFFLINE:-0}" == "1" ]]; then
    OFFLINE_FLAG="--offline"
    echo -e "${YELLOW}Building in offline mode (--offline)${NC}"
    log_with_timestamp "Building in offline mode"
fi

# Run cargo build and capture output, ensuring cargo's exit code is preserved
echo -e "${YELLOW}Running cargo build...${NC}"
set +e
cargo build ${OFFLINE_FLAG} 2>&1 | tee -a "$BUILD_LOG"
CARGO_STATUS=${PIPESTATUS[0]}
set -e
if [[ $CARGO_STATUS -eq 0 ]]; then
    echo -e "${GREEN}Build completed successfully!${NC}"
    log_with_timestamp "Build completed successfully"
else
    echo -e "${RED}Build failed with exit code: $CARGO_STATUS${NC}"
    log_with_timestamp "Build failed with exit code: $CARGO_STATUS"

    # Check for specific error patterns
    if grep -q "edition2024" "$BUILD_LOG"; then
        echo -e "${RED}Error: A dependency requires Rust edition 2024${NC}"
        echo -e "${YELLOW}To fix this issue, you can:${NC}"
        echo -e "  1. Update to Rust nightly: rustup install nightly && rustup default nightly"
        echo -e "  2. Or use an older version of the problematic dependency"
        log_with_timestamp "ERROR: Dependency requires Rust edition 2024 - consider using Rust nightly"
    fi

    # Also try to capture more detailed error information
    echo -e "${YELLOW}Running cargo check for additional diagnostics...${NC}"
    set +e
    cargo check ${OFFLINE_FLAG} --message-format=json 2>&1 | while IFS= read -r line; do
        # Parse JSON messages for errors
        if echo "$line" | grep -q '"level":"error"'; then
            log_with_timestamp "ERROR: $line"
        fi
    done
    set -e

    exit $CARGO_STATUS
fi

# Optional: Run tests and log any failures
if [ "${1:-}" == "--with-tests" ]; then
    echo -e "${YELLOW}Running tests...${NC}"
    log_with_timestamp "===== Running tests ====="
    
    set +e
    cargo test ${OFFLINE_FLAG} 2>&1 | tee -a "$BUILD_LOG"
    TEST_STATUS=${PIPESTATUS[0]}
    set -e
    if [[ $TEST_STATUS -eq 0 ]]; then
        echo -e "${GREEN}Tests passed!${NC}"
        log_with_timestamp "Tests completed successfully"
    else
        echo -e "${RED}Tests failed with exit code: $TEST_STATUS${NC}"
        log_with_timestamp "Tests failed with exit code: $TEST_STATUS"
        exit $TEST_STATUS
    fi
fi

log_with_timestamp "===== Build process ended ====="
echo -e "${GREEN}Build log saved to: $BUILD_LOG${NC}" 
