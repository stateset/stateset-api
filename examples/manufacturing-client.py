#!/usr/bin/env python3
"""
StateSet Manufacturing API - Python Client Examples

This module demonstrates comprehensive manufacturing operations using
the StateSet API, including:
- Bill of Materials (BOM) management
- Work order lifecycle
- Component reservation and tracking
- Batch production
- Quality control integration
- Production analytics

Requirements:
    pip install requests python-dateutil

Usage:
    python manufacturing-client.py
"""

import requests
import json
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, asdict
from decimal import Decimal


class ManufacturingClient:
    """
    Comprehensive client for StateSet Manufacturing API
    """

    def __init__(self, base_url: str = "http://localhost:8080/api/v1"):
        self.base_url = base_url
        self.token: Optional[str] = None
        self.session = requests.Session()

    def login(self, email: str, password: str) -> Dict:
        """Authenticate and get JWT token"""
        response = self.session.post(
            f"{self.base_url}/auth/login",
            json={"email": email, "password": password}
        )
        response.raise_for_status()
        data = response.json()
        self.token = data['access_token']
        self.session.headers.update({
            'Authorization': f'Bearer {self.token}'
        })
        print(f"✓ Logged in as {email}")
        return data

    # ========== BOM Management ==========

    def create_bom(
        self,
        bom_name: str,
        item_id: str,
        organization_id: int,
        revision: Optional[str] = None
    ) -> Dict:
        """Create a Bill of Materials"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/boms",
            json={
                "bom_name": bom_name,
                "item_id": item_id,
                "organization_id": organization_id,
                "revision": revision
            }
        )
        response.raise_for_status()
        return response.json()

    def add_bom_component(
        self,
        bom_id: str,
        component_item_id: str,
        quantity_per_assembly: float,
        uom_code: str,
        operation_seq_num: Optional[int] = None
    ) -> Dict:
        """Add a component to a BOM"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/boms/{bom_id}/components",
            json={
                "component_item_id": component_item_id,
                "quantity_per_assembly": quantity_per_assembly,
                "uom_code": uom_code,
                "operation_seq_num": operation_seq_num
            }
        )
        response.raise_for_status()
        return response.json()

    def get_bom(self, bom_id: str) -> Dict:
        """Get BOM details"""
        response = self.session.get(
            f"{self.base_url}/manufacturing/boms/{bom_id}"
        )
        response.raise_for_status()
        return response.json()

    def explode_bom(
        self,
        item_id: str,
        quantity: float,
        level: int = 0
    ) -> List[Dict]:
        """Explode multi-level BOM"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/boms/explode",
            json={
                "item_id": item_id,
                "quantity": quantity,
                "level": level
            }
        )
        response.raise_for_status()
        return response.json()

    # ========== Work Order Management ==========

    def create_work_order(
        self,
        work_order_number: str,
        item_id: str,
        quantity_to_build: float,
        scheduled_start_date: str,
        scheduled_completion_date: str,
        location_id: int,
        organization_id: int = 1,
        priority: str = "MEDIUM"
    ) -> Dict:
        """Create a manufacturing work order"""
        response = self.session.post(
            f"{self.base_url}/work-orders",
            json={
                "work_order_number": work_order_number,
                "item_id": item_id,
                "organization_id": organization_id,
                "quantity_to_build": quantity_to_build,
                "scheduled_start_date": scheduled_start_date,
                "scheduled_completion_date": scheduled_completion_date,
                "location_id": location_id,
                "priority": priority
            }
        )
        response.raise_for_status()
        return response.json()

    def start_work_order(
        self,
        work_order_id: str,
        location_id: int,
        operator_id: Optional[str] = None
    ) -> Dict:
        """Start production on a work order"""
        response = self.session.post(
            f"{self.base_url}/work-orders/{work_order_id}/start",
            json={
                "location_id": location_id,
                "operator_id": operator_id
            }
        )
        response.raise_for_status()
        return response.json()

    def complete_work_order(
        self,
        work_order_id: str,
        completed_quantity: float,
        location_id: int
    ) -> Dict:
        """Complete a work order"""
        response = self.session.post(
            f"{self.base_url}/work-orders/{work_order_id}/complete",
            json={
                "completed_quantity": completed_quantity,
                "location_id": location_id
            }
        )
        response.raise_for_status()
        return response.json()

    def hold_work_order(
        self,
        work_order_id: str,
        reason: Optional[str] = None
    ) -> Dict:
        """Put work order on hold"""
        response = self.session.put(
            f"{self.base_url}/work-orders/{work_order_id}/hold",
            json={"reason": reason}
        )
        response.raise_for_status()
        return response.json()

    def resume_work_order(self, work_order_id: str) -> Dict:
        """Resume a held work order"""
        response = self.session.put(
            f"{self.base_url}/work-orders/{work_order_id}/resume"
        )
        response.raise_for_status()
        return response.json()

    def cancel_work_order(
        self,
        work_order_id: str,
        location_id: int
    ) -> Dict:
        """Cancel a work order"""
        response = self.session.delete(
            f"{self.base_url}/work-orders/{work_order_id}",
            json={"location_id": location_id}
        )
        response.raise_for_status()
        return response.json()

    def get_work_order(self, work_order_id: str) -> Dict:
        """Get work order details"""
        response = self.session.get(
            f"{self.base_url}/work-orders/{work_order_id}"
        )
        response.raise_for_status()
        return response.json()

    def list_work_orders(
        self,
        status: Optional[str] = None,
        page: int = 1,
        limit: int = 20
    ) -> Dict:
        """List work orders with optional filtering"""
        params = {"page": page, "limit": limit}
        if status:
            params["status"] = status

        response = self.session.get(
            f"{self.base_url}/work-orders",
            params=params
        )
        response.raise_for_status()
        return response.json()

    # ========== Batch Production ==========

    def create_batch(
        self,
        batch_number: str,
        product_id: str,
        batch_size: int,
        production_date: str,
        expiry_date: str,
        location_id: int
    ) -> Dict:
        """Create a production batch record"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/batches",
            json={
                "batch_number": batch_number,
                "product_id": product_id,
                "batch_size": batch_size,
                "production_date": production_date,
                "expiry_date": expiry_date,
                "location_id": location_id,
                "status": "PLANNED"
            }
        )
        response.raise_for_status()
        return response.json()

    def add_batch_materials(
        self,
        batch_id: str,
        materials: List[Dict]
    ) -> Dict:
        """Record materials used in batch production"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/batches/{batch_id}/materials",
            json={"materials": materials}
        )
        response.raise_for_status()
        return response.json()

    def start_batch(
        self,
        batch_id: str,
        operator_id: str,
        equipment_id: Optional[str] = None
    ) -> Dict:
        """Start batch production"""
        response = self.session.put(
            f"{self.base_url}/manufacturing/batches/{batch_id}/start",
            json={
                "started_by": operator_id,
                "equipment_id": equipment_id,
                "start_time": datetime.utcnow().isoformat() + "Z"
            }
        )
        response.raise_for_status()
        return response.json()

    def add_quality_check(
        self,
        batch_id: str,
        test_name: str,
        test_type: str,
        results: Dict,
        status: str,
        tested_by: str
    ) -> Dict:
        """Add quality control test result"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/batches/{batch_id}/quality-checks",
            json={
                "test_name": test_name,
                "test_type": test_type,
                "results": results,
                "status": status,
                "tested_by": tested_by,
                "test_time": datetime.utcnow().isoformat() + "Z"
            }
        )
        response.raise_for_status()
        return response.json()

    def complete_batch(
        self,
        batch_id: str,
        actual_quantity: int,
        completed_by: str,
        yield_percentage: float
    ) -> Dict:
        """Complete batch production"""
        response = self.session.put(
            f"{self.base_url}/manufacturing/batches/{batch_id}/complete",
            json={
                "completed_by": completed_by,
                "completion_time": datetime.utcnow().isoformat() + "Z",
                "actual_quantity_produced": actual_quantity,
                "yield_percentage": yield_percentage
            }
        )
        response.raise_for_status()
        return response.json()

    def release_batch(
        self,
        batch_id: str,
        released_by: str,
        review_notes: str
    ) -> Dict:
        """Release batch for distribution"""
        response = self.session.put(
            f"{self.base_url}/manufacturing/batches/{batch_id}/release",
            json={
                "released_by": released_by,
                "release_date": datetime.utcnow().isoformat() + "Z",
                "release_status": "APPROVED",
                "review_notes": review_notes
            }
        )
        response.raise_for_status()
        return response.json()

    def get_batch_genealogy(self, batch_id: str) -> Dict:
        """Get complete batch genealogy report"""
        response = self.session.get(
            f"{self.base_url}/manufacturing/batches/{batch_id}/genealogy"
        )
        response.raise_for_status()
        return response.json()

    # ========== Component & Robot Tracking ==========

    def create_component_serial(
        self,
        serial_number: str,
        component_type: str,
        component_sku: str,
        supplier_id: str,
        supplier_lot_number: str,
        manufacture_date: str,
        receive_date: str,
        location: str
    ) -> Dict:
        """Create component serial number record"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/components/serials",
            json={
                "serial_number": serial_number,
                "component_type": component_type,
                "component_sku": component_sku,
                "supplier_id": supplier_id,
                "supplier_lot_number": supplier_lot_number,
                "manufacture_date": manufacture_date,
                "receive_date": receive_date,
                "location": location
            }
        )
        response.raise_for_status()
        return response.json()

    def create_robot_serial(
        self,
        serial_number: str,
        robot_model: str,
        robot_type: str,
        product_id: str,
        work_order_id: str,
        manufacturing_date: str
    ) -> Dict:
        """Create robot serial number"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/robots/serials",
            json={
                "serial_number": serial_number,
                "robot_model": robot_model,
                "robot_type": robot_type,
                "product_id": product_id,
                "work_order_id": work_order_id,
                "manufacturing_date": manufacturing_date
            }
        )
        response.raise_for_status()
        return response.json()

    def install_component(
        self,
        robot_serial_id: str,
        component_serial_id: str,
        position: str,
        installed_by: str
    ) -> Dict:
        """Install component in robot"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/components/install",
            json={
                "robot_serial_id": robot_serial_id,
                "component_serial_id": component_serial_id,
                "position": position,
                "installed_by": installed_by
            }
        )
        response.raise_for_status()
        return response.json()

    def add_test_result(
        self,
        test_protocol_id: str,
        robot_serial_id: str,
        tested_by: str,
        status: str,
        measurements: Dict,
        notes: Optional[str] = None
    ) -> Dict:
        """Add test result for robot"""
        response = self.session.post(
            f"{self.base_url}/manufacturing/test-results",
            json={
                "test_protocol_id": test_protocol_id,
                "robot_serial_id": robot_serial_id,
                "tested_by": tested_by,
                "status": status,
                "measurements": measurements,
                "notes": notes
            }
        )
        response.raise_for_status()
        return response.json()

    def get_robot_genealogy(self, robot_serial_id: str) -> Dict:
        """Get complete robot genealogy"""
        response = self.session.get(
            f"{self.base_url}/manufacturing/robots/serials/{robot_serial_id}/genealogy"
        )
        response.raise_for_status()
        return response.json()

    # ========== Analytics ==========

    def get_production_metrics(
        self,
        start_date: str,
        end_date: str
    ) -> Dict:
        """Get production metrics for date range"""
        response = self.session.get(
            f"{self.base_url}/analytics/manufacturing/production",
            params={
                "start_date": start_date,
                "end_date": end_date
            }
        )
        response.raise_for_status()
        return response.json()

    def get_work_order_analytics(self) -> Dict:
        """Get work order analytics"""
        response = self.session.get(
            f"{self.base_url}/analytics/manufacturing/work-orders"
        )
        response.raise_for_status()
        return response.json()

    def get_yield_analysis(self, product_id: Optional[str] = None) -> Dict:
        """Get yield analysis"""
        params = {}
        if product_id:
            params["product_id"] = product_id

        response = self.session.get(
            f"{self.base_url}/analytics/manufacturing/yield",
            params=params
        )
        response.raise_for_status()
        return response.json()


# ========== Example Usage ==========

def example_work_order_lifecycle():
    """Example: Complete work order lifecycle"""
    print("\n" + "="*60)
    print("Example 1: Work Order Lifecycle")
    print("="*60 + "\n")

    client = ManufacturingClient()
    client.login("admin@stateset.com", "your-password")

    # Create BOM
    print("Creating BOM...")
    bom = client.create_bom(
        bom_name="BOM-WIDGET-001",
        item_id="item-widget-001",
        organization_id=1,
        revision="1.0"
    )
    print(f"✓ BOM created: {bom['bom_id']}")

    # Add components
    print("\nAdding components to BOM...")
    client.add_bom_component(
        bom_id=bom['bom_id'],
        component_item_id="item-screw-001",
        quantity_per_assembly=4.0,
        uom_code="EA",
        operation_seq_num=10
    )
    print("✓ Added screws (4 per unit)")

    client.add_bom_component(
        bom_id=bom['bom_id'],
        component_item_id="item-plastic-001",
        quantity_per_assembly=0.250,
        uom_code="LB",
        operation_seq_num=20
    )
    print("✓ Added plastic (0.250 lbs per unit)")

    # Create work order
    print("\nCreating work order...")
    today = datetime.now()
    wo = client.create_work_order(
        work_order_number=f"WO-{today.strftime('%Y%m%d')}-001",
        item_id="item-widget-001",
        quantity_to_build=100.0,
        scheduled_start_date=today.strftime("%Y-%m-%d"),
        scheduled_completion_date=(today + timedelta(days=5)).strftime("%Y-%m-%d"),
        location_id=100,
        priority="HIGH"
    )
    print(f"✓ Work order created: {wo['work_order_number']}")
    print(f"  Status: {wo['status_code']}")
    print(f"  Quantity: {wo['quantity_to_build']}")

    # Start work order
    print("\nStarting work order...")
    wo = client.start_work_order(
        work_order_id=wo['work_order_id'],
        location_id=100,
        operator_id="operator-001"
    )
    print(f"✓ Work order started")
    print(f"  Status: {wo['status_code']}")

    # Complete work order
    print("\nCompleting work order...")
    wo = client.complete_work_order(
        work_order_id=wo['work_order_id'],
        completed_quantity=100.0,
        location_id=100
    )
    print(f"✓ Work order completed")
    print(f"  Status: {wo['status_code']}")
    print(f"  Completed: {wo['quantity_completed']}/{wo['quantity_to_build']}")

    print("\n✓ Work order lifecycle complete!")


def example_batch_production():
    """Example: Batch production with quality control"""
    print("\n" + "="*60)
    print("Example 2: Batch Production with Quality Control")
    print("="*60 + "\n")

    client = ManufacturingClient()
    client.login("admin@stateset.com", "your-password")

    # Create batch
    print("Creating batch...")
    today = datetime.now()
    expiry = today + timedelta(days=730)  # 2 years

    batch = client.create_batch(
        batch_number=f"BATCH-{today.strftime('%Y%m%d')}-001",
        product_id="prod-tablet-001",
        batch_size=100000,
        production_date=today.strftime("%Y-%m-%d"),
        expiry_date=expiry.strftime("%Y-%m-%d"),
        location_id=100
    )
    print(f"✓ Batch created: {batch['batch_number']}")

    # Add materials
    print("\nRecording batch materials...")
    materials = [
        {
            "material_id": "rm-001",
            "material_name": "Active Ingredient",
            "lot_number": "LOT-2024-001",
            "quantity_used": 25.0,
            "unit": "kg"
        },
        {
            "material_id": "rm-002",
            "material_name": "Excipient",
            "lot_number": "LOT-2024-002",
            "quantity_used": 45.0,
            "unit": "kg"
        }
    ]
    client.add_batch_materials(batch['id'], materials)
    print("✓ Materials recorded")

    # Start batch
    print("\nStarting batch production...")
    client.start_batch(
        batch_id=batch['id'],
        operator_id="operator-001",
        equipment_id="PRESS-01"
    )
    print("✓ Batch started")

    # Add quality checks
    print("\nPerforming quality checks...")
    client.add_quality_check(
        batch_id=batch['id'],
        test_name="Weight Variation",
        test_type="in_process",
        results={
            "average_weight_mg": 626.3,
            "within_spec": True
        },
        status="PASS",
        tested_by="qc-analyst-001"
    )
    print("✓ Weight variation test: PASS")

    client.add_quality_check(
        batch_id=batch['id'],
        test_name="Assay",
        test_type="final_release",
        results={
            "assay_percent": 101.2,
            "within_spec": True
        },
        status="PASS",
        tested_by="qc-analyst-002"
    )
    print("✓ Assay test: PASS")

    # Complete batch
    print("\nCompleting batch...")
    client.complete_batch(
        batch_id=batch['id'],
        actual_quantity=98500,
        completed_by="operator-001",
        yield_percentage=98.5
    )
    print(f"✓ Batch completed (98.5% yield)")

    # Release batch
    print("\nReleasing batch...")
    client.release_batch(
        batch_id=batch['id'],
        released_by="qa-manager-001",
        review_notes="All tests passed. Approved for distribution."
    )
    print("✓ Batch released")

    # Get genealogy
    print("\nRetrieving batch genealogy...")
    genealogy = client.get_batch_genealogy(batch['id'])
    print(f"✓ Genealogy retrieved")
    print(f"  Materials: {len(genealogy.get('materials', []))}")
    print(f"  Quality checks: {len(genealogy.get('quality_checks', []))}")

    print("\n✓ Batch production complete!")


def example_component_traceability():
    """Example: Component traceability for robot manufacturing"""
    print("\n" + "="*60)
    print("Example 3: Component Traceability")
    print("="*60 + "\n")

    client = ManufacturingClient()
    client.login("admin@stateset.com", "your-password")

    # Create component serials
    print("Creating component serial numbers...")
    motor = client.create_component_serial(
        serial_number="MOTOR-2024-12345",
        component_type="servo_motor",
        component_sku="MTR-6000",
        supplier_id="sup-motors-001",
        supplier_lot_number="LOT-2024-Q4-001",
        manufacture_date="2024-11-15",
        receive_date="2024-11-25",
        location="Warehouse-A-Bin-42"
    )
    print(f"✓ Motor created: {motor['serial_number']}")

    controller = client.create_component_serial(
        serial_number="CTRL-2024-98765",
        component_type="controller",
        component_sku="CTRL-6000",
        supplier_id="sup-electronics-001",
        supplier_lot_number="LOT-2024-Q4-010",
        manufacture_date="2024-11-20",
        receive_date="2024-11-28",
        location="Warehouse-A-Bin-15"
    )
    print(f"✓ Controller created: {controller['serial_number']}")

    # Create robot
    print("\nCreating robot serial...")
    robot = client.create_robot_serial(
        serial_number="IR6000-202412-00042",
        robot_model="IR-6000",
        robot_type="articulated_arm",
        product_id="prod-robot-001",
        work_order_id="wo-robot-001",
        manufacturing_date=datetime.now().strftime("%Y-%m-%d")
    )
    print(f"✓ Robot created: {robot['serial_number']}")

    # Install components
    print("\nInstalling components...")
    client.install_component(
        robot_serial_id=robot['id'],
        component_serial_id=motor['id'],
        position="joint_1",
        installed_by="technician-001"
    )
    print("✓ Motor installed at joint_1")

    client.install_component(
        robot_serial_id=robot['id'],
        component_serial_id=controller['id'],
        position="main_controller",
        installed_by="technician-001"
    )
    print("✓ Controller installed")

    # Add test results
    print("\nRunning tests...")
    client.add_test_result(
        test_protocol_id="tp-001",
        robot_serial_id=robot['id'],
        tested_by="qa-engineer-001",
        status="pass",
        measurements={
            "joint_1_torque_nm": 185.5,
            "all_within_spec": True
        },
        notes="Torque test passed"
    )
    print("✓ Torque test: PASS")

    # Get genealogy
    print("\nRetrieving robot genealogy...")
    genealogy = client.get_robot_genealogy(robot['id'])
    print(f"✓ Genealogy retrieved")
    print(f"  Robot: {genealogy['serial_number']}")
    print(f"  Components installed: {len(genealogy.get('components', []))}")
    print(f"  Tests completed: {len(genealogy.get('test_results', []))}")

    print("\n✓ Component traceability complete!")


def example_production_analytics():
    """Example: Production analytics and reporting"""
    print("\n" + "="*60)
    print("Example 4: Production Analytics")
    print("="*60 + "\n")

    client = ManufacturingClient()
    client.login("admin@stateset.com", "your-password")

    # Get production metrics
    print("Fetching production metrics...")
    today = datetime.now()
    start_date = (today - timedelta(days=30)).strftime("%Y-%m-%d")
    end_date = today.strftime("%Y-%m-%d")

    metrics = client.get_production_metrics(start_date, end_date)
    print(f"✓ Production metrics (last 30 days):")
    print(f"  Work orders created: {metrics.get('work_orders_created', 0)}")
    print(f"  Work orders completed: {metrics.get('work_orders_completed', 0)}")
    print(f"  Units produced: {metrics.get('total_units_produced', 0)}")
    print(f"  Average yield: {metrics.get('average_yield_percent', 0):.1f}%")

    # Get work order analytics
    print("\nFetching work order analytics...")
    wo_analytics = client.get_work_order_analytics()
    print(f"✓ Work order analytics:")
    print(f"  Active orders: {wo_analytics.get('active_count', 0)}")
    print(f"  On hold: {wo_analytics.get('on_hold_count', 0)}")
    print(f"  Average cycle time: {wo_analytics.get('avg_cycle_time_days', 0):.1f} days")

    # Get yield analysis
    print("\nFetching yield analysis...")
    yield_analysis = client.get_yield_analysis()
    print(f"✓ Yield analysis:")
    print(f"  Overall yield: {yield_analysis.get('overall_yield_percent', 0):.1f}%")
    print(f"  Best product: {yield_analysis.get('best_product', 'N/A')}")
    print(f"  Improvement areas: {len(yield_analysis.get('improvement_areas', []))}")

    print("\n✓ Analytics retrieved successfully!")


if __name__ == "__main__":
    """Run all examples"""
    print("\n" + "="*60)
    print("StateSet Manufacturing API - Python Examples")
    print("="*60)

    try:
        example_work_order_lifecycle()
        example_batch_production()
        example_component_traceability()
        example_production_analytics()

        print("\n" + "="*60)
        print("All examples completed successfully!")
        print("="*60 + "\n")

    except requests.exceptions.HTTPError as e:
        print(f"\n❌ HTTP Error: {e}")
        print(f"Response: {e.response.text}")
    except Exception as e:
        print(f"\n❌ Error: {e}")
