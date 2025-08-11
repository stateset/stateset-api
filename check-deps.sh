#!/bin/bash

# Dependency checker script
# This script helps identify problematic dependencies and version conflicts

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== Dependency Check Tool ===${NC}"
echo ""

# Check Rust version
echo -e "${YELLOW}Current Rust version:${NC}"
rustc --version
cargo --version
echo ""

# Check for outdated dependencies
echo -e "${YELLOW}Checking for outdated dependencies...${NC}"
if command -v cargo-outdated &> /dev/null; then
    cargo outdated
else
    echo "cargo-outdated not installed. Install with: cargo install cargo-outdated"
fi
echo ""

# Try to update dependencies conservatively
echo -e "${YELLOW}Attempting conservative dependency update...${NC}"
cargo update --dry-run 2>&1 | tee dependency_check.log

# Check for edition2024 issues
if grep -q "edition2024" dependency_check.log; then
    echo -e "${RED}Warning: Some dependencies require Rust edition 2024${NC}"
    echo -e "${YELLOW}Affected packages:${NC}"
    grep -B2 -A2 "edition2024" dependency_check.log
fi

# Clean up
rm -f dependency_check.log

echo ""
echo -e "${BLUE}Dependency check complete!${NC}" 