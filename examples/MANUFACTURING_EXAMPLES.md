# Manufacturing & Production Examples

Comprehensive examples demonstrating manufacturing and production workflows using the StateSet API.

## Table of Contents

- [Overview](#overview)
- [Shell Script Demos](#shell-script-demos)
- [Python Client Examples](#python-client-examples)
- [TypeScript Client Examples](#typescript-client-examples)
- [Manufacturing Workflows](#manufacturing-workflows)
- [Quick Start](#quick-start)

## Overview

These examples demonstrate real-world manufacturing scenarios including:

- **Production Scheduling** - Multi-work order planning with material constraints
- **Batch Production** - Lot tracking and traceability for regulated industries
- **Supply Chain Integration** - End-to-end procurement to fulfillment
- **Quality Management** - Robot build with component tracking and testing
- **Work Order Lifecycle** - Complete production execution workflows
- **Manufacturing Analytics** - Production metrics and KPI tracking

## Shell Script Demos

Located in `/demos/`, these interactive demos show complete manufacturing workflows.

### Demo 1: Robot Build Workflow
**File**: `demo_1_robot_build.sh`

Complete lifecycle of building an industrial robot with full component traceability.

**Features:**
- Receive components from suppliers with lot tracking
- Create robot serial numbers
- Install components with traceability
- Run comprehensive test protocols
- Add safety certifications
- Mark ready for shipment
- Generate complete genealogy report

**Run it:**
```bash
cd demos
chmod +x demo_1_robot_build.sh
./demo_1_robot_build.sh
```

**What it demonstrates:**
- Component serial number tracking
- Assembly genealogy
- Quality testing protocols
- Certification management
- Traceability reporting

---

### Demo 2: Quality Issue Management
**File**: `demo_2_quality_issue.sh`

Complete Non-Conformance Report (NCR) workflow for handling quality issues.

**Features:**
- Test failure detection
- NCR creation and tracking
- Root cause analysis
- Corrective action planning
- Rework execution
- Retest and verification
- NCR closure

**Run it:**
```bash
cd demos
chmod +x demo_2_quality_issue.sh
./demo_2_quality_issue.sh
```

**What it demonstrates:**
- Quality issue resolution
- Root cause investigation
- Corrective/preventive actions
- Rework workflows
- Supplier quality management

---

### Demo 3: Production Dashboard
**File**: `demo_3_production_dashboard.sh`

Real-time production monitoring and metrics visualization.

**Features:**
- Work order status tracking
- Production KPI monitoring
- Inventory levels
- Quality metrics
- Efficiency analysis

**Run it:**
```bash
cd demos
chmod +x demo_3_production_dashboard.sh
./demo_3_production_dashboard.sh
```

**What it demonstrates:**
- Real-time production visibility
- KPI tracking
- Performance analytics
- Operational metrics

---

### Demo 4: Production Scheduling
**File**: `demo_4_production_scheduling.sh`

Advanced production scheduling with material constraints and priority management.

**Features:**
- Multi-work order creation with priorities
- Material availability analysis
- Production constraint identification
- Optimized scheduling
- Material shortage handling
- Capacity planning
- Timeline visualization

**Run it:**
```bash
cd demos
chmod +x demo_4_production_scheduling.sh
./demo_4_production_scheduling.sh
```

**What it demonstrates:**
- Priority-based scheduling
- Material requirements planning (MRP)
- Capacity optimization
- Emergency procurement
- Production timeline management

---

### Demo 5: Batch Production & Lot Tracking
**File**: `demo_5_batch_production.sh`

Pharmaceutical-style batch production with strict lot traceability.

**Features:**
- Batch record creation
- Raw material lot tracking
- In-process quality checks
- Final release testing
- Batch approval workflow
- Complete genealogy
- Forward/backward traceability

**Run it:**
```bash
cd demos
chmod +x demo_5_batch_production.sh
./demo_5_batch_production.sh
```

**What it demonstrates:**
- GMP-compliant batch tracking
- Lot genealogy
- Quality release process
- Certificate of Analysis (COA)
- Recall management

---

### Demo 6: Supply Chain Integration
**File**: `demo_6_supply_chain_integration.sh`

End-to-end supply chain from purchase order to customer shipment.

**Features:**
- Purchase order creation
- ASN (Advanced Shipping Notice) processing
- Receiving and quality inspection
- Warehouse putaway
- Component picking and kitting
- Production execution
- Finished goods receipt
- Customer order fulfillment
- Shipment creation and tracking

**Run it:**
```bash
cd demos
chmod +x demo_6_supply_chain_integration.sh
./demo_6_supply_chain_integration.sh
```

**What it demonstrates:**
- Complete supply chain visibility
- Procurement to cash flow
- Quality inspection workflows
- Lot traceability across supply chain
- Customer fulfillment

---

## Python Client Examples

**File**: `examples/manufacturing-client.py`

Comprehensive Python client with 4 complete examples.

### Installation
```bash
pip install requests python-dateutil
```

### Examples Included

#### 1. Work Order Lifecycle
Complete work order from BOM creation to completion:
- Create BOM
- Add components
- Create work order (with automatic material reservation)
- Start production
- Complete production

#### 2. Batch Production with Quality Control
Regulated batch manufacturing:
- Create batch record
- Add raw materials with lot numbers
- Start production
- Perform in-process QC
- Complete batch
- Release for distribution
- Generate genealogy

#### 3. Component Traceability
Robot manufacturing with full component tracking:
- Create component serials
- Create robot serial
- Install components
- Run tests
- Generate genealogy report

#### 4. Production Analytics
Retrieve and analyze production data:
- Production metrics (30-day)
- Work order analytics
- Yield analysis

### Usage

Run all examples:
```bash
python examples/manufacturing-client.py
```

Import as module:
```python
from manufacturing_client import ManufacturingClient

client = ManufacturingClient()
client.login("admin@stateset.com", "password")

# Create work order
wo = client.create_work_order(
    work_order_number="WO-2024-001",
    item_id="item-001",
    quantity_to_build=100.0,
    scheduled_start_date="2024-12-01",
    scheduled_completion_date="2024-12-05",
    location_id=100
)
```

---

## TypeScript Client Examples

**File**: `examples/manufacturing-client.ts`

Fully-typed TypeScript client with comprehensive examples.

### Installation
```bash
npm install axios uuid
```

### Examples Included

#### 1. Work Order Lifecycle
Complete production workflow with TypeScript type safety

#### 2. Batch Production
Batch manufacturing with quality control integration

#### 3. Production Analytics
Real-time production metrics and KPIs

### Usage

Run all examples:
```bash
npx ts-node examples/manufacturing-client.ts
```

Import as module:
```typescript
import ManufacturingClient from './manufacturing-client';

const client = new ManufacturingClient();
await client.login('admin@stateset.com', 'password');

// Create work order with full type checking
const wo = await client.createWorkOrder({
  work_order_number: 'WO-2024-001',
  item_id: 'item-001',
  quantity_to_build: 100.0,
  scheduled_start_date: '2024-12-01',
  scheduled_completion_date: '2024-12-05',
  location_id: 100,
  priority: 'HIGH'
});
```

---

## Manufacturing Workflows

### Work Order Lifecycle

```
Create BOM → Add Components → Create Work Order → Reserve Materials
     ↓
Start Production → Consume Components → Track Progress
     ↓
Complete Production → Add to Inventory → Update Status
```

### Batch Production Flow

```
Create Batch → Record Materials → Start Production
     ↓
In-Process QC → Continue Production → Final Testing
     ↓
Review Results → QA Approval → Release Batch
```

### Supply Chain Flow

```
Purchase Order → ASN → Receive & Inspect → Warehouse
     ↓
Pick Components → Production → Finished Goods
     ↓
Customer Order → Pick & Pack → Ship
```

---

## Quick Start

### Prerequisites

1. StateSet API running locally or remotely
2. Valid user credentials
3. Test data (products, BOMs, locations)

### Setup

1. **Configure API endpoint:**
   ```bash
   # Edit scripts and update:
   API_BASE="http://localhost:8080/api/v1"
   TOKEN="your_jwt_token_here"
   ```

2. **Authenticate:**
   ```bash
   # Get JWT token
   curl -X POST http://localhost:8080/api/v1/auth/login \
     -H "Content-Type: application/json" \
     -d '{"email": "admin@stateset.com", "password": "your-password"}'
   ```

3. **Run examples:**
   ```bash
   # Shell scripts
   cd demos && ./demo_1_robot_build.sh

   # Python
   python examples/manufacturing-client.py

   # TypeScript
   npx ts-node examples/manufacturing-client.ts
   ```

### Example: Create Your First Work Order

```bash
# 1. Create BOM
curl -X POST http://localhost:8080/api/v1/manufacturing/boms \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "bom_name": "BOM-WIDGET-001",
    "item_id": "item-001",
    "organization_id": 1
  }'

# 2. Add component
curl -X POST http://localhost:8080/api/v1/manufacturing/boms/{bom_id}/components \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "component_item_id": "comp-001",
    "quantity_per_assembly": 2.0,
    "uom_code": "EA"
  }'

# 3. Create work order
curl -X POST http://localhost:8080/api/v1/work-orders \
  -H "Authorization: Bearer $TOKEN" \
  -d '{
    "work_order_number": "WO-2024-001",
    "item_id": "item-001",
    "quantity_to_build": 100,
    "scheduled_start_date": "2024-12-01",
    "scheduled_completion_date": "2024-12-10",
    "location_id": 100
  }'
```

---

## Key Features Demonstrated

### 1. Traceability
- **Component Tracking**: Serial numbers from supplier to final product
- **Lot Tracking**: Raw material lots through production
- **Genealogy**: Complete family tree of components and assemblies

### 2. Quality Management
- **In-Process Testing**: Quality checks during production
- **Final Release**: QA approval workflows
- **Non-Conformance**: Issue tracking and resolution
- **Test Protocols**: Standardized testing procedures

### 3. Production Planning
- **Material Requirements Planning**: BOM explosion and component needs
- **Capacity Planning**: Production scheduling and optimization
- **Priority Management**: Urgent vs. routine orders
- **Constraint Management**: Material shortages and bottlenecks

### 4. Analytics & Reporting
- **Production Metrics**: Yield, cycle time, throughput
- **Quality Metrics**: Defect rates, test pass rates
- **Inventory Metrics**: Stock levels, turnover
- **Custom Reports**: Flexible querying and analysis

### 5. Compliance
- **GMP Ready**: Good Manufacturing Practice workflows
- **FDA 21 CFR Part 11**: Audit trail and electronic signatures
- **ISO 9001**: Quality management system support
- **Traceability**: Recall and investigation support

---

## Use Cases

### Discrete Manufacturing
- Electronics assembly
- Industrial equipment
- Automotive components
- Aerospace parts

### Process Manufacturing
- Pharmaceuticals
- Food & beverage
- Chemicals
- Cosmetics

### Regulated Industries
- Medical devices
- Pharmaceuticals
- Food production
- Defense manufacturing

---

## API Documentation

For detailed API documentation, see:
- **[Manufacturing API Guide](../docs/manufacturing-api-guide.md)** - Complete API reference
- **[API Overview](../docs/API_OVERVIEW.md)** - Platform overview
- **[Integration Guide](../docs/INTEGRATION_GUIDE.md)** - Integration patterns

---

## Support

Need help? Check:
- **GitHub Issues**: https://github.com/stateset/stateset-api/issues
- **Documentation**: https://docs.stateset.com
- **Email**: support@stateset.io

---

## Contributing

We welcome contributions! To add new examples:

1. Fork the repository
2. Create your example following existing patterns
3. Test thoroughly
4. Submit a pull request

See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

---

## License

MIT License - see [LICENSE](../LICENSE) file for details.

---

**Built with ❤️ by the StateSet team**
