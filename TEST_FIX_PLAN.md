# StateSet API Test Fix Plan

## Executive Summary

Based on comprehensive test analysis, there are **64 failing tests** across 21 test files. The failures fall into several categories that can be addressed systematically.

---

## Root Cause Analysis

### Failure Categories

| Category | Count | Impact |
|----------|-------|--------|
| Missing/Incorrect Routes (404) | ~25 | High |
| Database Schema Issues | ~10 | High |
| Response Format Mismatches | ~15 | Medium |
| Service Logic Errors (500) | ~8 | High |
| Validation/Business Logic | ~6 | Medium |

---

## Phase 1: Database Schema Fixes (Critical)

### 1.1 Fix `inventory_locations` Table - `created_at` Constraint

**Error:** `NOT NULL constraint failed: inventory_locations.created_at`

**File:** `src/migrator.rs` (line ~1290)

**Fix:** The `inventory_locations` migration needs to set a default value for `created_at` or the entity's `ActiveModelBehavior` needs to set it automatically.

```rust
// In the inventory_locations migration, change:
.col(ColumnDef::new(InventoryLocations::CreatedAt).timestamp().not_null())

// To:
.col(ColumnDef::new(InventoryLocations::CreatedAt).timestamp().not_null().default(Expr::current_timestamp()))
```

**Alternative Fix:** Update the entity to set `created_at` in `before_save`:

**File:** `src/entities/inventory_location.rs` or `src/entities/inventory_locations.rs`

```rust
#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            if self.created_at.is_not_set() {
                self.created_at = Set(chrono::Utc::now());
            }
        }
        self.updated_at = Set(Some(chrono::Utc::now()));
        Ok(self)
    }
}
```

### 1.2 Verify All Entity Timestamps Have Defaults

**Files to check:**
- `src/entities/inventory_location.rs`
- `src/entities/inventory_locations.rs`
- `src/entities/inventory_items.rs`
- `src/entities/suppliers.rs`

**Action:** Ensure all entities with `created_at` NOT NULL have either:
1. Default value in migration, OR
2. `ActiveModelBehavior` that sets the timestamp

---

## Phase 2: Route/Handler Fixes (404 Errors)

### 2.1 Cart Item Routes - Return 404 Instead of 200

**Error:** `left: 404, right: 200` at `cart_integration_test.rs:186`

**Test expects:** `POST /api/v1/carts/{id}/items` returns 200 or 201

**Investigation needed:**

**File:** `src/lib.rs` - Check cart routes registration

```rust
// Expected routes:
// POST   /api/v1/carts/{id}/items      - Add item to cart
// PUT    /api/v1/carts/{id}/items/{item_id} - Update item quantity
// DELETE /api/v1/carts/{id}/items/{item_id} - Remove item
```

**File:** `src/handlers/commerce/carts.rs`

**Action:** Verify these handlers exist and are properly registered:
- `add_item_to_cart`
- `update_cart_item`
- `remove_cart_item`

### 2.2 Order Get by ID - Returns 404

**Error:** `left: 404, right: 200` at `integration_orders_test.rs:229`

**Test expects:** `GET /api/v1/orders/{id}` returns 200

**File:** `src/lib.rs` - Check order routes

**File:** `src/handlers/orders.rs` - Check `get_order` handler

**Possible issues:**
1. Route path mismatch (e.g., `/:id` vs `/{id}`)
2. Handler not returning the order correctly
3. Order not being found due to UUID parsing issue

### 2.3 Order CRUD Get - Returns 404

**Error:** `left: 404, right: 200` at `integration_tests.rs:99`

**Same root cause as 2.2** - Order retrieval by ID fails

---

## Phase 3: Service/Handler Logic Fixes (500 Errors)

### 3.1 Purchase Order Creation - Returns 500

**Error:** `left: 500, right: 201` at `procurement_idempotency_test.rs:66`

**Test expects:** `POST /api/v1/purchase-orders` returns 201

**Files to investigate:**
- `src/handlers/purchase_orders.rs`
- `src/services/procurement.rs`

**Likely issues:**
1. Missing database table or column
2. Service throwing unhandled error
3. Missing required field in request

**Debug action:** Add logging to purchase order handler to see actual error

### 3.2 Payment Processing - Order Creation Fails

**Error:** `Order creation should succeed` at `payment_integration_test.rs:50`

**This is a prerequisite failure** - the test can't create an order to test payments against.

**Root cause:** Same as order GET issues - order operations are failing

---

## Phase 4: Response Format Fixes

### 4.1 Standardize API Response Format

Many tests expect responses in format:
```json
{
  "data": { ... },
  "meta": { ... }
}
```

**Files to check:**
- `src/handlers/orders.rs`
- `src/handlers/commerce/carts.rs`
- `src/handlers/inventory.rs`
- `src/handlers/payments.rs`

**Action:** Ensure all handlers return `ApiResponse<T>` wrapper

---

## Implementation Plan

### Step 1: Fix Database Schema (Day 1)

```bash
# Files to modify:
src/migrator.rs
src/entities/inventory_location.rs
src/entities/inventory_locations.rs
```

**Tasks:**
1. Add `default(Expr::current_timestamp())` to all `created_at` columns in migrations
2. Add `ActiveModelBehavior` to entities that need timestamp auto-population
3. Rebuild and run inventory tests

**Verification:**
```bash
cargo test --test inventory_api_test -- --ignored --nocapture
cargo test --test inventory_adjustment_test -- --ignored --nocapture
```

### Step 2: Fix Order Routes (Day 1-2)

```bash
# Files to modify:
src/lib.rs
src/handlers/orders.rs
```

**Tasks:**
1. Verify route registration for `GET /api/v1/orders/{id}`
2. Check UUID parsing in path parameters
3. Ensure handler returns proper response

**Verification:**
```bash
cargo test --test integration_orders_test -- --ignored --nocapture
cargo test --test integration_tests test_orders_crud -- --ignored --nocapture
```

### Step 3: Fix Cart Item Routes (Day 2)

```bash
# Files to modify:
src/lib.rs
src/handlers/commerce/carts.rs
src/services/commerce/cart_service.rs
```

**Tasks:**
1. Verify cart item routes are registered
2. Check handler implementations
3. Ensure cart items can be added/updated/removed

**Verification:**
```bash
cargo test --test cart_integration_test -- --ignored --nocapture
```

### Step 4: Fix Purchase Order/ASN (Day 2-3)

```bash
# Files to modify:
src/handlers/purchase_orders.rs
src/handlers/asn.rs
src/services/procurement.rs
src/services/asn.rs
```

**Tasks:**
1. Debug 500 error in purchase order creation
2. Add proper error handling
3. Implement idempotency key support

**Verification:**
```bash
cargo test --test procurement_idempotency_test -- --ignored --nocapture
```

### Step 5: Fix Payment Processing (Day 3)

```bash
# Files to modify:
src/handlers/payments.rs
src/services/payments.rs
```

**Tasks:**
1. Fix order creation prerequisite
2. Verify payment processing endpoint
3. Add validation error responses

**Verification:**
```bash
cargo test --test payment_integration_test -- --ignored --nocapture
```

### Step 6: Fix RBAC/Permission Tests (Day 3-4)

```bash
# Files to modify:
src/auth/mod.rs
src/middleware_helpers/
```

**Tasks:**
1. Fix admin role full access
2. Implement fine-grained permissions
3. Add audit logging

**Verification:**
```bash
cargo test --test rbac_permission_test -- --ignored --nocapture
cargo test --test auth_integration_test -- --ignored --nocapture
```

---

## Detailed File Changes

### File: `src/migrator.rs`

#### Change 1: Fix inventory_locations created_at default

```rust
// Around line 1310, change:
.col(ColumnDef::new(InventoryLocations::CreatedAt).timestamp().not_null())

// To:
.col(
    ColumnDef::new(InventoryLocations::CreatedAt)
        .timestamp()
        .not_null()
        .extra("DEFAULT CURRENT_TIMESTAMP".to_owned())
)
```

#### Change 2: Apply same fix to all timestamp columns in migrations

Apply to:
- `m20230101_000014_create_procurement_tables` - suppliers, purchase_order_headers, asns, asn_items
- `m20230101_000015_create_manufacturing_tables` - manufacture_orders, item_master, inventory_items

### File: `src/lib.rs`

#### Verify these routes exist:

```rust
// Orders
.route("/orders/:id", get(handlers::orders::get_order))
.route("/orders/:id", put(handlers::orders::update_order))

// Cart Items
.route("/carts/:id/items", post(handlers::commerce::carts::add_item_to_cart))
.route("/carts/:id/items/:item_id", put(handlers::commerce::carts::update_cart_item))
.route("/carts/:id/items/:item_id", delete(handlers::commerce::carts::remove_cart_item))

// Purchase Orders
.route("/purchase-orders", post(handlers::purchase_orders::create_purchase_order))

// ASN
.route("/asns", post(handlers::asn::create_asn))
```

### File: `src/handlers/orders.rs`

#### Verify get_order handler:

```rust
pub async fn get_order(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,  // Ensure this matches route parameter name
    claims: Claims,
) -> Result<impl IntoResponse, ServiceError> {
    // Handler implementation
}
```

---

## Test Execution Order

Run tests in this order to verify fixes:

```bash
# 1. Schema fixes
cargo test --test inventory_api_test -- --ignored
cargo test --test inventory_adjustment_test -- --ignored

# 2. Order fixes
cargo test --test order_lifecycle_test -- --ignored
cargo test --test integration_orders_test -- --ignored

# 3. Cart fixes
cargo test --test cart_integration_test -- --ignored
cargo test --test checkout_flow_test -- --ignored

# 4. Procurement fixes
cargo test --test procurement_idempotency_test -- --ignored

# 5. Payment fixes
cargo test --test payment_integration_test -- --ignored

# 6. Auth/RBAC fixes
cargo test --test auth_integration_test -- --ignored
cargo test --test rbac_permission_test -- --ignored

# 7. Full suite
cargo test --tests -- --ignored --test-threads=1
```

---

## Success Criteria

| Test Suite | Current | Target |
|------------|---------|--------|
| order_lifecycle_test | 85% | 100% |
| return_workflow_test | 93% | 100% |
| cart_integration_test | 39% | 90%+ |
| checkout_flow_test | 75% | 95%+ |
| payment_integration_test | 69% | 90%+ |
| auth_integration_test | 88% | 100% |
| rbac_permission_test | 56% | 90%+ |
| inventory_api_test | 0% | 90%+ |
| procurement_idempotency_test | 0% | 90%+ |
| **Overall** | **60%** | **90%+** |

---

## Priority Order

1. **P0 (Blocking):** Database schema fixes - affects all inventory tests
2. **P0 (Blocking):** Order GET route - affects many dependent tests
3. **P1 (High):** Cart item operations - affects checkout flow
4. **P1 (High):** Purchase order creation - procurement functionality
5. **P2 (Medium):** Payment processing - depends on orders working
6. **P2 (Medium):** RBAC improvements - security features
7. **P3 (Low):** Response format standardization - polish

---

## Estimated Effort

| Phase | Effort | Dependencies |
|-------|--------|--------------|
| Phase 1: Schema | 2-4 hours | None |
| Phase 2: Routes | 4-6 hours | Phase 1 |
| Phase 3: Services | 4-8 hours | Phase 2 |
| Phase 4: Response Format | 2-4 hours | Phase 3 |
| Testing & Validation | 2-4 hours | All |
| **Total** | **14-26 hours** | |

---

## Appendix: Specific Error Analysis

### Error 1: Cart Add Item 404

**Test:** `test_add_item_to_cart` (cart_integration_test.rs:186)
**Error:** `left: 404, right: 200`

**Route exists:** `src/handlers/commerce/carts.rs:30` - `.route("/{id}/items", post(add_to_cart))`

**Likely cause:** Cart ownership verification fails because:
1. Cart created with no `customer_id`
2. `verify_cart_owner` returns 404 when cart owner doesn't match user

**Fix location:** `src/handlers/commerce/carts.rs:152` - `verify_cart_owner` function

**Suggested fix:** Check if cart ownership verification is too strict for carts created without customer_id

### Error 2: Order Get 404

**Test:** `test_get_order_endpoint` (integration_orders_test.rs:229)
**Error:** `left: 404, right: 200`

**Route exists:** `src/lib.rs:255` - `.route("/orders/{id}", get(handlers::orders::get_order))`

**Likely cause:**
1. Order ID not being found in database after creation
2. Different UUID format/parsing issue

**Fix location:** `src/handlers/orders.rs` - `get_order` function

### Error 3: Inventory Location Timestamp

**Test:** `inventory_item_lifecycle` (inventory_api_test.rs:29)
**Error:** `NOT NULL constraint failed: inventory_locations.created_at`

**Root cause:** Entity doesn't set `created_at` before insert

**Fix location:**
1. `src/migrator.rs` - Add default timestamp to migration
2. `src/entities/inventory_location.rs` - Add `ActiveModelBehavior`

### Error 4: Purchase Order 500

**Test:** `purchase_order_create_is_idempotent` (procurement_idempotency_test.rs:66)
**Error:** `left: 500, right: 201`

**Root cause:** Internal server error during purchase order creation

**Debug needed:** Add logging to identify exact failure point

**Fix location:** `src/handlers/purchase_orders.rs` or `src/services/procurement.rs`

### Error 5: Payment Prerequisite Failure

**Test:** `test_process_payment_success` (payment_integration_test.rs:50)
**Error:** `Order creation should succeed`

**Root cause:** Test can't create order, so payment test fails

**Fix:** Fix order creation issues first (see Error 2)
