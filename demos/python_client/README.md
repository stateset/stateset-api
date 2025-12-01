# StateSet Manufacturing API - Python Client

A comprehensive Python client library for interacting with the StateSet Manufacturing API.

## Features

- **Object-oriented API**: Clean, intuitive interface for all manufacturing endpoints
- **Type safety**: Enum types for robot types, statuses, test statuses, etc.
- **Error handling**: Custom exceptions for API errors
- **Full coverage**: Supports robots, components, tests, NCRs, production metrics, and more
- **Easy to use**: Simple methods for common workflows

## Installation

```bash
pip install -r requirements.txt
```

## Quick Start

```python
from stateset_manufacturing import StateSetManufacturing, RobotType

# Initialize client
client = StateSetManufacturing(
    api_base="http://localhost:3000/api/v1/manufacturing",
    api_token="your_jwt_token_here"
)

# Create a robot
robot = client.robots.create(
    serial_number="IR6000-202412-00100",
    robot_model="IR-6000",
    robot_type=RobotType.ARTICULATED_ARM,
    product_id="product-uuid"
)

# Get robot details
robot_details = client.robots.get(robot["id"])

# List all robots
robots = client.robots.list(status="in_production", limit=10)

# Get component genealogy
genealogy = client.robots.get_genealogy(robot["id"])
```

## API Reference

### Robot Operations

```python
# Create robot serial number
robot = client.robots.create(
    serial_number="IR6000-202412-00100",
    robot_model="IR-6000",
    robot_type=RobotType.ARTICULATED_ARM,
    product_id="uuid",
    work_order_id="uuid",
    manufacturing_date=datetime.now()
)

# Get robot by ID
robot = client.robots.get("robot-uuid")

# List robots with filters
robots = client.robots.list(
    status=RobotStatus.IN_PRODUCTION,
    robot_type=RobotType.ARTICULATED_ARM,
    robot_model="IR-6000",
    limit=50,
    offset=0
)

# Update robot
updated_robot = client.robots.update(
    "robot-uuid",
    status=RobotStatus.READY.value,
    ship_date=datetime.now().isoformat()
)

# Get complete genealogy
genealogy = client.robots.get_genealogy("robot-uuid")
```

### Component Operations

```python
# Create component serial number
component = client.components.create(
    serial_number="MOTOR-2024-12345",
    component_type="servo_motor",
    component_sku="MTR-6000-J1",
    supplier_lot_number="LOT-2024-Q4-001",
    receive_date=date.today()
)

# Install component into robot
client.components.install(
    robot_serial_id="robot-uuid",
    component_serial_id="component-uuid",
    position="joint_1",
    installed_by="user-uuid"
)
```

### Test Operations

```python
# Create test protocol
protocol = client.test_protocols.create(
    protocol_number="TP-001",
    name="Joint Torque Test",
    test_type="mechanical",
    description="Verify joint torque specifications",
    applicable_models=["IR-6000", "IR-8000"]
)

# List all test protocols
protocols = client.test_protocols.list()

# Record test result
result = client.test_results.create(
    test_protocol_id="protocol-uuid",
    robot_serial_id="robot-uuid",
    tested_by="user-uuid",
    status=TestStatus.PASS,
    measurements={
        "joint_1_torque": 185.5,
        "joint_2_torque": 178.2
    },
    notes="All joints within specification"
)

# Get all test results for a robot
results = client.test_results.get_robot_results("robot-uuid")
```

### Quality Management (NCR)

```python
# Create NCR
ncr = client.ncrs.create(
    ncr_number="NCR-202412-00001",
    robot_serial_id="robot-uuid",
    reported_by="user-uuid",
    issue_type="dimensional",
    severity=NcrSeverity.MAJOR,
    description="Joint 3 positioning accuracy exceeds tolerance",
    assigned_to="engineer-uuid"
)

# List NCRs with filters
ncrs = client.ncrs.list(
    status="open",
    severity=NcrSeverity.CRITICAL,
    limit=50
)

# Update NCR
ncr = client.ncrs.update(
    "ncr-uuid",
    status="investigating",
    root_cause="Encoder calibration issue"
)

# Close NCR
closed_ncr = client.ncrs.close(
    "ncr-uuid",
    resolution_notes="Issue resolved through component replacement",
    disposition="rework"
)
```

### Production Metrics

```python
# Create production line
line = client.production.create_line(
    line_code="ASSY-LINE-01",
    line_name="Main Assembly Line 1",
    line_type="assembly",
    capacity_per_shift=8,
    status="operational"
)

# List production lines
lines = client.production.list_lines()

# Record production metrics
metrics = client.production.create_metrics(
    production_line_id="line-uuid",
    production_date=date.today(),
    shift="morning",
    work_order_id="wo-uuid",
    robot_model="IR-6000",
    planned_quantity=8,
    actual_quantity=7,
    quantity_passed=6,
    quantity_failed=1,
    planned_hours=8.0,
    actual_hours=8.5,
    downtime_hours=0.5,
    downtime_reason="Component delivery delay"
)

# Get production metrics
metrics = client.production.get_metrics(
    production_date=date.today(),
    production_line_id="line-uuid"
)
```

## Enums

The library provides type-safe enums for common values:

```python
from stateset_manufacturing import (
    RobotType,
    RobotStatus,
    ComponentStatus,
    TestStatus,
    NcrSeverity
)

# Robot types
RobotType.ARTICULATED_ARM
RobotType.COBOT
RobotType.AMR
RobotType.SPECIALIZED

# Robot statuses
RobotStatus.IN_PRODUCTION
RobotStatus.TESTING
RobotStatus.READY
RobotStatus.SHIPPED
RobotStatus.IN_SERVICE

# Test statuses
TestStatus.PASS
TestStatus.FAIL
TestStatus.RETEST

# NCR severity levels
NcrSeverity.CRITICAL
NcrSeverity.MAJOR
NcrSeverity.MINOR
```

## Error Handling

```python
from stateset_manufacturing import APIError, StateSetManufacturingError

try:
    robot = client.robots.create(
        serial_number="IR6000-202412-00100",
        robot_model="IR-6000",
        robot_type=RobotType.ARTICULATED_ARM,
        product_id="product-uuid"
    )
except APIError as e:
    print(f"API Error {e.status_code}: {e.message}")
except StateSetManufacturingError as e:
    print(f"Error: {e}")
```

## Examples

The `demos/python_client/` directory contains complete examples:

### Example 1: Complete Robot Build Workflow

Demonstrates building a robot from start to finish:
- Create robot serial number
- Create and install components
- Run test suite
- Mark ready for shipment

```bash
python example_1_build_robot.py
```

### Example 2: Quality Issue Management

Shows handling a quality issue:
- Detect test failure
- Create NCR
- Investigation and root cause analysis
- Rework and retest
- Close NCR

```bash
python example_2_quality_management.py
```

### Example 3: Production Analytics

Demonstrates production monitoring:
- Create production lines
- Record production metrics
- Calculate OEE (Overall Equipment Effectiveness)
- Generate production reports
- Analyze quality trends

```bash
python example_3_production_analytics.py
```

## Configuration

The client requires two configuration parameters:

1. **api_base**: The base URL of your StateSet Manufacturing API
   - Example: `http://localhost:3000/api/v1/manufacturing`

2. **api_token**: Your JWT authentication token
   - Obtain from your authentication system

Optional parameters:

3. **timeout**: Request timeout in seconds (default: 30)

```python
client = StateSetManufacturing(
    api_base="http://localhost:3000/api/v1/manufacturing",
    api_token="your_jwt_token_here",
    timeout=60  # 60 seconds
)
```

## Requirements

- Python 3.7+
- requests library

See `requirements.txt` for full dependencies.

## License

This client library is part of the StateSet API project.

## Support

For issues and questions:
- GitHub Issues: https://github.com/stateset/stateset-api/issues
- Documentation: https://docs.stateset.io

## Development

### Running Tests

```bash
# Install development dependencies
pip install -r requirements-dev.txt

# Run tests
pytest tests/

# Run with coverage
pytest --cov=stateset_manufacturing tests/
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## Changelog

### Version 1.0.0 (2024-12-01)
- Initial release
- Support for robots, components, tests, NCRs, and production metrics
- Type-safe enums
- Comprehensive error handling
- Complete examples

## API Coverage

This client library provides complete coverage of the StateSet Manufacturing API:

- ✅ Robot Serial Numbers (create, get, list, update, genealogy)
- ✅ Component Serial Numbers (create, install)
- ✅ Test Protocols (create, list)
- ✅ Test Results (create, get robot results)
- ✅ Non-Conformance Reports (create, list, update, close)
- ✅ Production Metrics (create, get)
- ✅ Production Lines (create, list)
- ✅ Certifications (via robots API)
- ✅ Service History (via robots API)

## Best Practices

1. **Always use error handling**: Wrap API calls in try-except blocks
2. **Use enums for type safety**: Prefer `RobotType.ARTICULATED_ARM` over `"articulated_arm"`
3. **Store credentials securely**: Never hardcode tokens, use environment variables
4. **Validate UUIDs**: Ensure UUIDs are valid before making API calls
5. **Log API interactions**: Log requests and responses for debugging
6. **Handle rate limits**: Implement retry logic for rate-limited requests

## Examples of Common Workflows

### Building Multiple Robots

```python
for i in range(10):
    robot = client.robots.create(
        serial_number=f"IR6000-202412-{i:05d}",
        robot_model="IR-6000",
        robot_type=RobotType.ARTICULATED_ARM,
        product_id="product-uuid"
    )
    print(f"Created robot {robot['serial_number']}")
```

### Batch Testing

```python
robots = client.robots.list(status="in_production")

for robot in robots['data']:
    for protocol in protocols:
        result = client.test_results.create(
            test_protocol_id=protocol['id'],
            robot_serial_id=robot['id'],
            tested_by="user-uuid",
            status=TestStatus.PASS
        )
```

### Daily Production Report

```python
from datetime import date

metrics = client.production.get_metrics(
    production_date=date.today()
)

total_produced = sum(m['actual_quantity'] for m in metrics)
total_passed = sum(m['quantity_passed'] for m in metrics)
first_pass_yield = (total_passed / total_produced * 100) if total_produced > 0 else 0

print(f"Today's Production: {total_produced} units")
print(f"First-Pass Yield: {first_pass_yield:.1f}%")
```
