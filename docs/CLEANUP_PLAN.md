# Stateset API - Code Cleanup Plan

**Version:** 1.0
**Date:** 2025-12-10
**Status:** Planning Phase

## Executive Summary

This document outlines a systematic approach to cleaning up the stateset-api codebase. The plan is organized by priority and includes specific action items with affected files, estimated effort, and expected outcomes.

---

## Phase 1: Critical Cleanup (Week 1-2)

### 1.1 Remove Dead Code

**Priority:** HIGH
**Effort:** 2-4 hours
**Impact:** Reduces codebase size, improves clarity

#### Tasks:
- [ ] Delete `src/bin/placeholder.rs` - Empty placeholder file
- [ ] Review and remove or complete commented modules in `src/commands/mod.rs` (lines 29-53):
  - [ ] `inventory` commands
  - [ ] `billofmaterials` commands
  - [ ] `picking` commands
  - [ ] `receiving` commands
  - [ ] `analytics` commands
  - [ ] `audit` commands
  - [ ] `carriers` commands
  - [ ] `customers` commands
  - [ ] `forecasting` commands
  - [ ] `kitting` commands
  - [ ] `maintenance` commands
  - [ ] `packaging` commands
  - [ ] `payments` commands
  - [ ] `quality` commands
  - [ ] `suppliers` commands
  - [ ] `transfers` commands
  - [ ] `warehouses` commands
- [ ] Review disabled handlers in `src/handlers/mod.rs` (lines 20-21):
  - [ ] Reports handler
  - [ ] Suppliers handler
- [ ] Delete or populate `src/schema.rs` (only 9 lines of comments)

**Decision Point:** For each commented module, determine:
1. Is it needed? → Implement it
2. Is it planned for future? → Document in roadmap and remove code
3. Is it obsolete? → Remove entirely

---

### 1.2 Fix Critical Error Handling

**Priority:** HIGH
**Effort:** 8-12 hours
**Impact:** Prevents production panics

#### Tasks:
- [ ] Replace `.unwrap()` with proper error handling in critical files:
  - [ ] `src/services/procurement.rs:228`
  - [ ] `src/versioning/api_versioning.rs:452, 459`
  - [ ] `src/bin/orders_bench.rs:48-50`
- [ ] Audit remaining 60+ files with `.unwrap()` calls
- [ ] Create standard pattern for error handling:
  ```rust
  // Instead of: value.unwrap()
  // Use: value.map_err(|e| ServiceError::...)?
  ```
- [ ] Replace `.expect()` calls in command modules with `?` operator

**Testing:** Ensure all error paths are covered by tests

---

### 1.3 Replace Debug Statements with Logging

**Priority:** HIGH
**Effort:** 4-6 hours
**Impact:** Cleaner production logs, better debugging

#### Tasks:
- [ ] Replace 121 instances of `println!`, `dbg!`, `eprintln!` with `tracing` macros
- [ ] Pattern to follow:
  ```rust
  // Instead of: println!("Debug: {}", value);
  // Use: tracing::debug!("Processing value: {}", value);

  // Instead of: eprintln!("Error: {}", err);
  // Use: tracing::error!("Operation failed: {}", err);
  ```
- [ ] Verify proper log levels (trace, debug, info, warn, error)

---

### 1.4 Complete or Remove Empty Services

**Priority:** HIGH
**Effort:** 16-24 hours (if implementing) OR 2 hours (if removing)
**Impact:** Eliminates misleading API surface

#### Files to Address:
- [ ] `src/services/forecasting.rs` (12 lines, only `new()`)
- [ ] `src/services/business_intelligence.rs` (12 lines, only `new()`)
- [ ] `src/services/leads.rs` (12 lines, only `new()`)
- [ ] `src/services/accounts.rs` (12 lines, only `new()`)

#### Decision Matrix:
| Service | Keep? | Action Required |
|---------|-------|-----------------|
| Forecasting | TBD | Implement or move to future roadmap |
| Business Intelligence | TBD | Implement or move to future roadmap |
| Leads | TBD | Implement or remove |
| Accounts | TBD | Implement or remove |

---

### 1.5 ML Module - Implement or Remove

**Priority:** HIGH
**Effort:** 40+ hours (if implementing) OR 4 hours (if removing)
**Impact:** Sets clear expectations for ML capabilities

#### Current State:
- `src/ml/forecasting.rs` - Returns "not yet implemented" error
- `src/ml/anomaly_detection.rs` - Returns empty vec
- `src/ml/recommendations.rs` - Returns empty vec
- `src/ml/mod.rs` - Placeholder functions (lines 43-93)

#### Options:
1. **Option A: Remove ML Module**
   - Delete `src/ml/` directory
   - Remove ML-related dependencies
   - Update documentation to reflect no ML support
   - Mark as "Future Enhancement" in roadmap

2. **Option B: Mark as Experimental**
   - Add `#[deprecated]` attributes with message directing to roadmap
   - Add clear "EXPERIMENTAL - NOT FOR PRODUCTION" warnings
   - Document planned implementation timeline

3. **Option C: Implement Basic ML Features**
   - Requires significant development effort
   - Should be separate project/sprint

**Recommendation:** Option A or B (remove or mark experimental)

---

## Phase 2: Architectural Improvements (Week 3-4)

### 2.1 Consolidate Entity/Model Dual System

**Priority:** HIGH
**Effort:** 20-30 hours
**Impact:** Reduces confusion, improves maintainability

#### Current Problem:
- Both `src/entities/` (95 files) and `src/models/` (95 files) exist
- Overlapping functionality (e.g., `entities/order.rs` vs `models/order.rs`)
- Unclear which to use when

#### Proposed Solution:
```
Decision: Keep entities/ (SeaORM convention), deprecate models/

Step 1: Audit current usage
  - [ ] Identify all imports from models/
  - [ ] Identify all imports from entities/
  - [ ] Map overlapping types

Step 2: Migration strategy
  - [ ] Create type aliases in models/ pointing to entities/
  - [ ] Add deprecation warnings
  - [ ] Update all internal code to use entities/

Step 3: Remove models/
  - [ ] Remove models/ directory
  - [ ] Update documentation
  - [ ] Bump major version (breaking change)
```

#### Files Requiring Changes:
- [ ] Update imports in all service files
- [ ] Update imports in all handler files
- [ ] Update imports in all command files
- [ ] Update tests

---

### 2.2 Standardize Service File Naming

**Priority:** MEDIUM
**Effort:** 4-6 hours
**Impact:** Reduces confusion, improves discoverability

#### Current Inconsistencies:
| Large File | Re-export File | Decision |
|------------|----------------|----------|
| `inventory.rs` (1340 lines) | `inventory_service.rs` (1 line) | Keep inventory.rs only |
| `orders.rs` (1814 lines) | `order_service.rs` (1 line) | Keep orders.rs only |
| `returns.rs` | `return_service.rs` (1 line) | Keep returns.rs only |
| `shipments.rs` | `shipment_service.rs` (1 line) | Keep shipments.rs only |
| `warranties.rs` | `warranty_service.rs` (1 line) | Keep warranties.rs only |

#### Tasks:
- [ ] Delete 1-line re-export files
- [ ] Update imports to use main service files
- [ ] Standardize on pattern: `src/services/{domain}.rs` or `src/services/{domain}/mod.rs`

---

### 2.3 Command Pattern Consistency

**Priority:** MEDIUM
**Effort:** 12-16 hours
**Impact:** Consistent architecture across modules

#### Current State:
- 186 command files exist
- Only partially implemented across modules
- Some modules use commands, others call services directly

#### Options:
1. **Fully adopt command pattern**
   - Complete missing commands for all operations
   - Consistent CQRS-style architecture
   - More boilerplate but clearer separation

2. **Phase out command pattern**
   - Simplify to direct service calls
   - Less boilerplate
   - Faster development

#### Tasks (if adopting):
- [ ] Complete command implementations for all services
- [ ] Document command pattern usage
- [ ] Create command generator script/template

#### Tasks (if phasing out):
- [ ] Move command logic to service methods
- [ ] Delete command files
- [ ] Update handlers to call services directly

**Decision Required:** Discuss with team which approach to take

---

## Phase 3: Code Quality Improvements (Week 5-6)

### 3.1 Reduce Excessive Cloning

**Priority:** MEDIUM
**Effort:** 16-20 hours
**Impact:** Improved performance, reduced memory usage

#### Current State:
- 1,396 `.clone()` calls throughout codebase
- Potential performance overhead

#### Approach:
1. **Profile first**
   - [ ] Run performance benchmarks
   - [ ] Identify hot paths
   - [ ] Measure clone impact

2. **Target high-impact areas**
   - [ ] Focus on request handlers (most frequent)
   - [ ] Focus on loops (multiplied impact)
   - [ ] Focus on large data structures

3. **Refactoring patterns**
   ```rust
   // Instead of: let data = expensive_data.clone();
   // Use: let data = &expensive_data;

   // Or use Arc for shared ownership
   // Instead of: struct { data: Vec<BigStruct> }
   // Use: struct { data: Arc<Vec<BigStruct>> }
   ```

#### Files to Review:
- Services with high request volume
- Data transformation pipelines
- Event processing

---

### 3.2 Break Up Large Files

**Priority:** MEDIUM
**Effort:** 20-30 hours
**Impact:** Improved maintainability, faster compilation

#### Files Over 1000 Lines:
- [ ] `services/commerce/agentic_checkout.rs` (2289 lines)
  - Split into: handlers, validators, processors, models
- [ ] `migrator.rs` (1910 lines)
  - Keep as-is (generated code) or split by migration year
- [ ] `bin/stateset_cli.rs` (1847 lines)
  - Split into: cli/commands/, cli/handlers/
- [ ] `services/orders.rs` (1814 lines)
  - Split into: orders/create.rs, orders/update.rs, orders/query.rs, orders/workflows.rs
- [ ] `services/commerce/product_feed_service.rs` (1595 lines)
  - Split into: feed/parser.rs, feed/validator.rs, feed/importer.rs
- [ ] Additional 12+ files over 1000 lines

#### Refactoring Pattern:
```
Before: src/services/orders.rs (1814 lines)

After:
src/services/orders/
  ├── mod.rs          (exports and shared types)
  ├── create.rs       (order creation logic)
  ├── update.rs       (order updates)
  ├── queries.rs      (read operations)
  ├── workflows.rs    (state transitions)
  └── validation.rs   (business rules)
```

---

### 3.3 Remove Global Allow Directives

**Priority:** LOW
**Effort:** 8-12 hours
**Impact:** Better compile-time checks

#### Current State:
- `src/lib.rs:7` has `#![allow(dead_code)]` globally

#### Tasks:
- [ ] Remove `#![allow(dead_code)]` from lib.rs
- [ ] Fix or explicitly allow each dead code warning
- [ ] Review other global allows (unused_imports, etc.)
- [ ] Add specific allows only where justified

---

### 3.4 Replace Wildcard Imports

**Priority:** LOW
**Effort:** 6-8 hours
**Impact:** Clearer dependencies, better IDE support

#### Current State:
- 50+ instances of `use module::*`
- Heavy use of `use sea_orm::*` in commands

#### Tasks:
- [ ] Replace wildcard imports with explicit imports
- [ ] Pattern to follow:
  ```rust
  // Instead of: use sea_orm::*;
  // Use: use sea_orm::{EntityTrait, QueryFilter, QuerySelect};
  ```
- [ ] Use IDE refactoring tools to assist

---

## Phase 4: Testing & Documentation (Week 7-8)

### 4.1 Increase Test Coverage

**Priority:** HIGH
**Effort:** 40-60 hours
**Impact:** Higher confidence, easier refactoring

#### Current State:
- 6.6% test coverage (39 files with tests out of 589)

#### Target:
- 60% test coverage minimum
- 80% for critical paths (orders, payments, inventory)

#### Approach:
1. **Unit tests for services**
   - [ ] Test each public service method
   - [ ] Mock database calls
   - [ ] Test error cases

2. **Integration tests for handlers**
   - [ ] Test HTTP endpoints
   - [ ] Test request validation
   - [ ] Test response formats

3. **Property-based tests for business logic**
   - Already have proptest dependency
   - [ ] Test invariants (e.g., order total = sum of items)

#### Priority Modules for Testing:
- [ ] Authentication/Authorization
- [ ] Payment processing
- [ ] Order management
- [ ] Inventory tracking
- [ ] Shipment handling

---

### 4.2 Add Module Documentation

**Priority:** MEDIUM
**Effort:** 12-16 hours
**Impact:** Better onboarding, clearer architecture

#### Current State:
- 3,171 doc comments (`///`)
- Only 73 module docs (`//!`)

#### Tasks:
- [ ] Add module-level docs to all public modules
- [ ] Document architectural decisions
- [ ] Add usage examples

#### Template:
```rust
//! # Order Service Module
//!
//! This module handles all order-related operations including creation,
//! updates, status transitions, and queries.
//!
//! ## Architecture
//!
//! The service follows a layered architecture:
//! - Public API methods in OrderService
//! - Business logic in private helper functions
//! - Data access via Repository pattern
//!
//! ## Example
//!
//! ```rust
//! let service = OrderService::new(db);
//! let order = service.create_order(request).await?;
//! ```
```

---

### 4.3 Security Audit

**Priority:** HIGH
**Effort:** 16-24 hours
**Impact:** Improved security posture

#### Areas to Audit:
- [ ] Authentication module (30+ files handle passwords/tokens)
- [ ] API key management
- [ ] JWT token handling
- [ ] Password hashing (using argon2)
- [ ] Secret storage and masking
- [ ] SQL injection vectors (low risk, but verify)
- [ ] Input validation
- [ ] Rate limiting implementation

#### Specific Findings to Address:
- [ ] `services/stablepay_service.rs:570, 581` - Secret masking with `.unwrap_or("XXXXXXXX")`
- [ ] Review raw SQL in `events/outbox.rs` (lines 46-60, 96-109)

---

## Phase 5: Performance Optimization (Week 9-10)

### 5.1 Profile and Benchmark

**Priority:** MEDIUM
**Effort:** 12-16 hours
**Impact:** Identify bottlenecks

#### Tasks:
- [ ] Set up criterion benchmarks (already dependency)
- [ ] Benchmark critical paths:
  - [ ] Order creation
  - [ ] Product search
  - [ ] Inventory updates
  - [ ] Report generation
- [ ] Profile with flamegraph
- [ ] Identify N+1 queries
- [ ] Measure cache hit rates

---

### 5.2 Optimize Database Queries

**Priority:** MEDIUM
**Effort:** 16-24 hours
**Impact:** Reduced latency, better scalability

#### Tasks:
- [ ] Review SeaORM queries for N+1 patterns
- [ ] Add database indices where needed
- [ ] Implement query result caching
- [ ] Use select_only() to fetch only needed columns
- [ ] Batch operations where possible

---

### 5.3 Review Redis Cache Usage

**Priority:** LOW
**Effort:** 4-6 hours
**Impact:** Consistent caching behavior

#### Current Issue:
- `src/cache/mod.rs:186` - Silently falls back to in-memory cache

#### Tasks:
- [ ] Add warning log when Redis unavailable
- [ ] Make cache backend configurable
- [ ] Document cache fallback behavior
- [ ] Consider making Redis optional at compile time

---

## Implementation Strategy

### Recommended Order:
1. **Week 1:** Phase 1.1, 1.2, 1.3 (Remove dead code, fix error handling, fix logging)
2. **Week 2:** Phase 1.4, 1.5 (Empty services, ML module)
3. **Week 3:** Phase 2.1 (Entity/Model consolidation)
4. **Week 4:** Phase 2.2, 2.3 (Naming, command pattern)
5. **Week 5:** Phase 3.1, 3.2 (Cloning, large files)
6. **Week 6:** Phase 3.3, 3.4 (Allow directives, wildcard imports)
7. **Week 7:** Phase 4.1 (Testing)
8. **Week 8:** Phase 4.2, 4.3 (Documentation, security)
9. **Week 9-10:** Phase 5 (Performance)

### Each Phase Should Include:
1. Create feature branch
2. Make changes
3. Run tests
4. Update documentation
5. Code review
6. Merge to main

### Risk Mitigation:
- Make changes incrementally
- Maintain backward compatibility where possible
- Add deprecation warnings before removal
- Version bump appropriately (major for breaking changes)

---

## Success Metrics

### Code Quality:
- [ ] Zero placeholder/empty files
- [ ] Zero commented-out module imports
- [ ] <10 `.unwrap()` calls in production code
- [ ] Zero debug print statements in production code

### Architecture:
- [ ] Single entity system (no dual entity/model)
- [ ] Consistent naming conventions
- [ ] Clear command pattern usage (all or none)

### Testing:
- [ ] 60%+ test coverage
- [ ] All critical paths tested
- [ ] All public APIs documented

### Performance:
- [ ] <500ms p95 for order creation
- [ ] <200ms p95 for product search
- [ ] <100ms p95 for inventory lookup

---

## Maintenance Plan

After cleanup completion:
1. Add pre-commit hooks for:
   - Prevent `println!` in src/ (allow in tests/)
   - Warn on `.unwrap()` usage
   - Enforce consistent formatting

2. Update contributing guidelines with:
   - Error handling patterns
   - Logging standards
   - Testing requirements
   - Documentation requirements

3. Schedule quarterly code quality reviews

---

## Decision Log

| Date | Decision | Rationale | Owner |
|------|----------|-----------|-------|
| TBD | ML Module fate | TBD | TBD |
| TBD | Entity vs Model | TBD | TBD |
| TBD | Command pattern adoption | TBD | TBD |

---

## Appendix A: Quick Wins

These can be done independently at any time:
- [ ] Delete `src/bin/placeholder.rs`
- [ ] Delete 1-line re-export service files
- [ ] Delete `src/schema.rs` or populate it
- [ ] Replace `println!` with `tracing::info!` in main.rs
- [ ] Add `.env.example` file
- [ ] Update .gitignore to exclude build artifacts

---

## Appendix B: Tools & Automation

### Recommended Tools:
- `cargo clippy` - Lint catching
- `cargo fmt` - Code formatting
- `cargo audit` - Security vulnerabilities
- `cargo outdated` - Dependency updates
- `cargo bloat` - Binary size analysis
- `cargo tarpaulin` - Test coverage
- `cargo deny` - License/dependency checking

### Automation Scripts:
```bash
# Quick cleanup script
#!/bin/bash
cargo fmt
cargo clippy --fix --allow-dirty
cargo test

# Find unwrap usage
rg "\.unwrap\(\)" --type rust src/

# Find println usage
rg "println!" --type rust src/

# Find TODO/FIXME
rg "(TODO|FIXME)" --type rust src/
```

---

**Next Steps:**
1. Review this plan with the team
2. Make decisions on open questions (ML module, Entity/Model, Command pattern)
3. Prioritize phases based on business needs
4. Assign owners to each phase
5. Create tracking issues/tickets
6. Begin Phase 1
