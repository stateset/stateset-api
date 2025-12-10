# Manufacturing API Routes - Implementation Guide

## Routes Configuration

Add these routes to your Axum router (typically in `src/api.rs` or `src/main.rs`):

```rust
use axum::{
    routing::{get, post, put, delete},
    Router,
};
use crate::handlers::manufacturing;

pub fn manufacturing_routes() -> Router<AppState> {
    Router::new()
        // ======================
        // Robot Serial Numbers
        // ======================
        .route("/robots/serials", post(manufacturing::create_robot_serial))
        .route("/robots/serials", get(manufacturing::list_robot_serials))
        .route("/robots/serials/:id", get(manufacturing::get_robot_serial))
        .route("/robots/serials/:id", put(manufacturing::update_robot_serial))
        .route("/robots/serials/:id/genealogy", get(manufacturing::get_robot_genealogy))

        // ==========================
        // Component Serial Numbers
        // ==========================
        .route("/components/serials", post(manufacturing::create_component_serial))
        .route("/components/install", post(manufacturing::install_component))

        // =================
        // Test Protocols
        // =================
        .route("/test-protocols", post(manufacturing::create_test_protocol))
        .route("/test-protocols", get(manufacturing::list_test_protocols))

        // ==============
        // Test Results
        // ==============
        .route("/test-results", post(manufacturing::create_test_result))
        .route("/robots/:robot_id/test-results", get(manufacturing::get_robot_test_results))

        // =================================
        // Non-Conformance Reports (NCRs)
        // =================================
        .route("/ncrs", post(manufacturing::create_ncr))
        .route("/ncrs", get(manufacturing::list_ncrs))
        .route("/ncrs/:id/close", post(manufacturing::close_ncr))
}

// Add to main API router:
pub fn create_router(state: AppState) -> Router {
    Router::new()
        // ... existing routes ...
        .nest("/api/v1/manufacturing", manufacturing_routes())
        .with_state(state)
}
```

---

## Complete API Reference

### Base URL
```
/api/v1/manufacturing
```

---

## 🤖 Robot Serial Numbers

### Create Robot Serial
**POST** `/robots/serials`

Create a new robot serial number.

**Request Body:**
```json
{
  "serial_number": "IR6000-202412-00001",
  "product_id": "uuid",
  "work_order_id": "uuid",
  "robot_model": "IR-6000",
  "robot_type": "articulated_arm",
  "manufacturing_date": "2024-12-01T10:00:00Z",
  "customer_id": "uuid",
  "order_id": "uuid"
}
```

**Response:** `201 Created`
```json
{
  "id": "uuid",
  "serial_number": "IR6000-202412-00001",
  "robot_model": "IR-6000",
  "robot_type": "articulated_arm",
  "status": "in_production",
  "is_under_warranty": false,
  "warranty_remaining_days": null,
  "created_at": "2024-12-01T10:00:00Z",
  ...
}
```

---

### Get Robot Serial
**GET** `/robots/serials/:id`

Get robot serial number details by ID.

**Response:** `200 OK`
```json
{
  "id": "uuid",
  "serial_number": "IR6000-202412-00001",
  "robot_model": "IR-6000",
  "robot_type": "articulated_arm",
  "status": "in_production",
  "warranty_start_date": "2024-12-15T00:00:00Z",
  "warranty_end_date": "2027-12-15T00:00:00Z",
  "is_under_warranty": true,
  "warranty_remaining_days": 1095,
  ...
}
```

---

### List Robot Serials
**GET** `/robots/serials?status=in_production&robot_type=articulated_arm&limit=50&offset=0`

List robot serials with optional filters.

**Query Parameters:**
- `status` - Filter by status: `in_production`, `testing`, `ready`, `shipped`, `in_service`, `returned`, `decommissioned`
- `robot_type` - Filter by type: `articulated_arm`, `cobot`, `amr`, `specialized`
- `robot_model` - Filter by model name
- `customer_id` - Filter by customer
- `limit` - Results per page (default 50, max 100)
- `offset` - Pagination offset

**Response:** `200 OK`
```json
{
  "data": [
    {
      "id": "uuid",
      "serial_number": "IR6000-202412-00001",
      "robot_model": "IR-6000",
      "status": "in_production",
      ...
    }
  ],
  "total": 42
}
```

---

### Update Robot Serial
**PUT** `/robots/serials/:id`

Update robot serial number information.

**Request Body:**
```json
{
  "status": "ready",
  "manufacturing_date": "2024-12-01T10:00:00Z",
  "ship_date": "2024-12-15T14:00:00Z",
  "warranty_start_date": "2024-12-15T00:00:00Z",
  "warranty_end_date": "2027-12-15T00:00:00Z"
}
```

**Response:** `200 OK` (same structure as GET)

---

### Get Robot Genealogy
**GET** `/robots/serials/:id/genealogy`

Get complete component traceability for a robot.

**Response:** `200 OK`
```json
{
  "robot_serial_number": "IR6000-202412-00001",
  "robot_model": "IR-6000",
  "robot_status": "ready",
  "components": [
    {
      "component_serial_number": "MOTOR-2024-12345",
      "component_type": "servo_motor",
      "component_sku": "MTR-6000-J1",
      "position": "joint_1",
      "installed_at": "2024-12-01T12:30:00Z",
      "supplier_lot_number": "LOT-2024-Q4-001"
    },
    {
      "component_serial_number": "CTRL-2024-98765",
      "component_type": "controller",
      "component_sku": "CTRL-6000",
      "position": "main_controller",
      "installed_at": "2024-12-01T11:00:00Z",
      "supplier_lot_number": "LOT-2024-Q4-010"
    }
  ]
}
```

---

## 🔧 Component Serial Numbers

### Create Component Serial
**POST** `/components/serials`

Create a new component serial number.

**Request Body:**
```json
{
  "serial_number": "MOTOR-2024-12345",
  "component_type": "servo_motor",
  "component_sku": "MTR-6000-J1",
  "supplier_id": "uuid",
  "supplier_lot_number": "LOT-2024-Q4-001",
  "manufacture_date": "2024-11-15",
  "receive_date": "2024-11-25",
  "location": "Warehouse-A-Bin-42"
}
```

**Response:** `201 Created`
```json
{
  "id": "uuid",
  "serial_number": "MOTOR-2024-12345",
  "component_type": "servo_motor",
  "component_sku": "MTR-6000-J1",
  "status": "in_stock",
  "age_in_days": 16,
  ...
}
```

---

### Install Component
**POST** `/components/install`

Install a component into a robot.

**Request Body:**
```json
{
  "robot_serial_id": "uuid",
  "component_serial_id": "uuid",
  "position": "joint_1",
  "installed_by": "user-uuid"
}
```

**Response:** `200 OK`
```json
{
  "message": "Component installed successfully",
  "robot_serial_id": "uuid",
  "component_serial_id": "uuid"
}
```

---

## 🧪 Test Protocols

### Create Test Protocol
**POST** `/test-protocols`

Create a new test protocol.

**Request Body:**
```json
{
  "protocol_number": "TP-006",
  "name": "Payload Capacity Test",
  "description": "Verify robot can handle maximum payload",
  "test_type": "mechanical",
  "applicable_models": ["IR-6000", "IR-8000"],
  "test_equipment_required": ["load_cell", "calibration_weights"],
  "estimated_duration_minutes": 45,
  "pass_criteria": {
    "max_payload_kg": 50,
    "positioning_error_mm": 0.5,
    "vibration_threshold": 0.1
  },
  "procedure_steps": {
    "steps": [
      "Mount load cell on end effector",
      "Apply incremental loads from 10kg to 50kg",
      "Measure positioning accuracy at each load",
      "Record vibration levels",
      "Verify no mechanical stress indicators"
    ]
  },
  "revision": "A"
}
```

**Response:** `201 Created`
```json
{
  "id": "uuid",
  "protocol_number": "TP-006",
  "name": "Payload Capacity Test",
  "test_type": "mechanical",
  "status": "draft",
  "is_active": false,
  ...
}
```

---

### List Test Protocols
**GET** `/test-protocols`

List all test protocols.

**Response:** `200 OK`
```json
[
  {
    "id": "uuid",
    "protocol_number": "TP-001",
    "name": "Joint Torque Test",
    "test_type": "mechanical",
    "status": "active",
    "is_active": true,
    ...
  }
]
```

---

## 📊 Test Results

### Create Test Result
**POST** `/test-results`

Record a test result.

**Request Body:**
```json
{
  "test_protocol_id": "uuid",
  "robot_serial_id": "uuid",
  "work_order_id": "uuid",
  "tested_by": "user-uuid",
  "status": "pass",
  "measurements": {
    "joint_1_torque": 185.5,
    "joint_2_torque": 178.2,
    "joint_3_torque": 192.1,
    "all_within_spec": true
  },
  "notes": "All joints within specification. No issues observed."
}
```

**Response:** `201 Created`
```json
{
  "id": "uuid",
  "test_protocol_id": "uuid",
  "robot_serial_id": "uuid",
  "tested_by": "uuid",
  "test_date": "2024-12-01T14:30:00Z",
  "status": "pass",
  "passed": true,
  "needs_retest": false,
  ...
}
```

---

### Get Robot Test Results
**GET** `/robots/:robot_id/test-results`

Get all test results for a specific robot.

**Response:** `200 OK`
```json
[
  {
    "id": "uuid",
    "test_protocol_name": "Joint Torque Test",
    "test_date": "2024-12-01T14:30:00Z",
    "status": "pass",
    "passed": true,
    "needs_retest": false,
    ...
  },
  {
    "id": "uuid",
    "test_protocol_name": "Positioning Accuracy Test",
    "test_date": "2024-12-01T15:45:00Z",
    "status": "pass",
    "passed": true,
    "needs_retest": false,
    ...
  }
]
```

---

## ⚠️ Non-Conformance Reports (NCRs)

### Create NCR
**POST** `/ncrs`

Create a non-conformance report.

**Request Body:**
```json
{
  "ncr_number": "NCR-202412-00001",
  "robot_serial_id": "uuid",
  "work_order_id": "uuid",
  "reported_by": "user-uuid",
  "issue_type": "dimensional",
  "severity": "major",
  "description": "Joint 3 positioning accuracy exceeds tolerance by 0.15mm",
  "assigned_to": "engineer-uuid"
}
```

**Response:** `201 Created`
```json
{
  "id": "uuid",
  "ncr_number": "NCR-202412-00001",
  "severity": "major",
  "status": "open",
  "is_open": true,
  "is_critical": false,
  ...
}
```

---

### List NCRs
**GET** `/ncrs?status=open&severity=major&limit=50`

List NCRs with optional filters.

**Query Parameters:**
- `status` - Filter by status: `open`, `investigating`, `action_required`, `resolved`, `closed`
- `severity` - Filter by severity: `critical`, `major`, `minor`
- `robot_serial_id` - Filter by robot
- `assigned_to` - Filter by assignee
- `limit` - Results per page
- `offset` - Pagination offset

**Response:** `200 OK`
```json
[
  {
    "id": "uuid",
    "ncr_number": "NCR-202412-00001",
    "severity": "major",
    "status": "open",
    "is_open": true,
    "is_critical": false,
    ...
  }
]
```

---

### Close NCR
**POST** `/ncrs/:id/close`

Close a non-conformance report.

**Request Body:**
```json
{
  "resolution_notes": "Joint 3 recalibrated. Positioning now within spec at 0.03mm error.",
  "disposition": "rework"
}
```

**Response:** `200 OK`
```json
{
  "id": "uuid",
  "ncr_number": "NCR-202412-00001",
  "status": "closed",
  "resolution_date": "2024-12-02T10:00:00Z",
  "disposition": "rework",
  "is_open": false,
  ...
}
```

---

## Example Workflows

### Workflow 1: Build and Test a Robot

```bash
# 1. Create robot serial number
POST /api/v1/manufacturing/robots/serials
{
  "serial_number": "IR6000-202412-00042",
  "robot_model": "IR-6000",
  "robot_type": "articulated_arm",
  ...
}

# 2. Install components
POST /api/v1/manufacturing/components/install
{
  "robot_serial_id": "{robot_id}",
  "component_serial_id": "{motor_1_id}",
  "position": "joint_1"
}

# 3. Run torque test
POST /api/v1/manufacturing/test-results
{
  "test_protocol_id": "{TP-001_id}",
  "robot_serial_id": "{robot_id}",
  "status": "pass",
  ...
}

# 4. Run positioning test
POST /api/v1/manufacturing/test-results
{
  "test_protocol_id": "{TP-002_id}",
  "robot_serial_id": "{robot_id}",
  "status": "pass",
  ...
}

# 5. Run safety test
POST /api/v1/manufacturing/test-results
{
  "test_protocol_id": "{TP-003_id}",
  "robot_serial_id": "{robot_id}",
  "status": "pass",
  ...
}

# 6. Update robot status to ready
PUT /api/v1/manufacturing/robots/serials/{robot_id}
{
  "status": "ready",
  "manufacturing_date": "2024-12-01T10:00:00Z"
}

# 7. Get complete genealogy for documentation
GET /api/v1/manufacturing/robots/serials/{robot_id}/genealogy
```

---

### Workflow 2: Handle Quality Issue

```bash
# 1. Create NCR when issue is found
POST /api/v1/manufacturing/ncrs
{
  "ncr_number": "NCR-202412-00042",
  "robot_serial_id": "{robot_id}",
  "severity": "major",
  "issue_type": "dimensional",
  "description": "Positioning accuracy out of spec",
  ...
}

# 2. Investigate and update NCR
PUT /api/v1/manufacturing/ncrs/{ncr_id}
{
  "root_cause": "Joint 3 encoder misalignment",
  "corrective_action": "Recalibrate joint 3 encoder",
  "status": "action_required"
}

# 3. Perform rework and retest
POST /api/v1/manufacturing/test-results
{
  "test_protocol_id": "{TP-002_id}",
  "robot_serial_id": "{robot_id}",
  "status": "pass",
  "notes": "After recalibration - all specs met"
}

# 4. Close NCR
POST /api/v1/manufacturing/ncrs/{ncr_id}/close
{
  "resolution_notes": "Recalibration successful",
  "disposition": "rework"
}
```

---

## Authentication & Authorization

All endpoints require authentication. Include JWT token in Authorization header:

```
Authorization: Bearer {your_jwt_token}
```

Recommended permission levels:
- **Production Operator**: Create test results, view robots, view components
- **Quality Engineer**: All test operations, create/update/close NCRs
- **Manufacturing Engineer**: All robot/component operations, test protocols
- **Manager**: Full access to all endpoints

---

## Error Responses

All endpoints return standard error responses:

**400 Bad Request**
```json
{
  "error": "Invalid request",
  "message": "Component is not available for installation"
}
```

**404 Not Found**
```json
{
  "error": "Not found",
  "message": "Robot serial not found"
}
```

**500 Internal Server Error**
```json
{
  "error": "Internal server error",
  "message": "Database error: ..."
}
```

---

## Rate Limiting

API endpoints are rate-limited to:
- 100 requests per minute for read operations
- 50 requests per minute for write operations

---

## Next Steps

1. Add these routes to your router configuration
2. Test endpoints with Postman or curl
3. Implement frontend UI for production workflows
4. Set up automated testing
5. Configure monitoring and alerting for critical NCRs
