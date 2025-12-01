#!/usr/bin/env python3
"""
Example 1: Complete Robot Build Workflow

This example demonstrates building a robot from start to finish:
1. Create robot serial number
2. Create and install components
3. Run test suite
4. Add certifications
5. Mark ready for shipment
"""

from stateset_manufacturing import (
    StateSetManufacturing,
    RobotType,
    RobotStatus,
    TestStatus,
    APIError
)
from datetime import datetime, date
import uuid


def main():
    # Initialize the client
    client = StateSetManufacturing(
        api_base="http://localhost:3000/api/v1/manufacturing",
        api_token="your_jwt_token_here"
    )

    print("=" * 60)
    print("StateSet Manufacturing - Robot Build Example")
    print("=" * 60)
    print()

    # Step 1: Create robot serial number
    print("[1] Creating robot serial number...")
    try:
        robot = client.robots.create(
            serial_number="IR6000-202412-00100",
            robot_model="IR-6000",
            robot_type=RobotType.ARTICULATED_ARM,
            product_id=str(uuid.uuid4()),  # Replace with actual product ID
            work_order_id=str(uuid.uuid4()),  # Replace with actual work order ID
            manufacturing_date=datetime.now()
        )
        robot_id = robot["id"]
        print(f"✓ Robot created: {robot['serial_number']}")
        print(f"  ID: {robot_id}")
        print(f"  Status: {robot['status']}")
        print()
    except APIError as e:
        print(f"✗ Error creating robot: {e}")
        return

    # Step 2: Create and install components
    print("[2] Creating and installing components...")
    components_to_install = [
        {
            "serial_number": "MOTOR-2024-10001",
            "type": "servo_motor",
            "sku": "MTR-6000-J1",
            "position": "joint_1"
        },
        {
            "serial_number": "MOTOR-2024-10002",
            "type": "servo_motor",
            "sku": "MTR-6000-J2",
            "position": "joint_2"
        },
        {
            "serial_number": "MOTOR-2024-10003",
            "type": "servo_motor",
            "sku": "MTR-6000-J3",
            "position": "joint_3"
        },
        {
            "serial_number": "CTRL-2024-50001",
            "type": "controller",
            "sku": "CTRL-6000",
            "position": "main_controller"
        },
        {
            "serial_number": "ENC-2024-20001",
            "type": "encoder",
            "sku": "ENC-6000-J1",
            "position": "joint_1_encoder"
        }
    ]

    installed_components = []
    for component_data in components_to_install:
        try:
            # Create component serial number
            component = client.components.create(
                serial_number=component_data["serial_number"],
                component_type=component_data["type"],
                component_sku=component_data["sku"],
                supplier_lot_number="LOT-2024-Q4-001",
                receive_date=date.today()
            )
            component_id = component["id"]

            # Install component
            client.components.install(
                robot_serial_id=robot_id,
                component_serial_id=component_id,
                position=component_data["position"],
                installed_by=str(uuid.uuid4())  # Replace with actual user ID
            )

            installed_components.append(component_data)
            print(f"✓ Installed {component_data['type']} at {component_data['position']}")

        except APIError as e:
            print(f"✗ Error installing {component_data['serial_number']}: {e}")

    print(f"\n  Total components installed: {len(installed_components)}")
    print()

    # Step 3: Run test suite
    print("[3] Running test suite...")

    # Test protocols to run (assuming these exist in the system)
    test_protocols = [
        ("tp-001", "Joint Torque Test"),
        ("tp-002", "Positioning Accuracy Test"),
        ("tp-003", "Safety Systems Test"),
        ("tp-004", "Controller Communication Test"),
        ("tp-005", "Software Integration Test")
    ]

    test_results = []
    for protocol_id, protocol_name in test_protocols:
        try:
            result = client.test_results.create(
                test_protocol_id=protocol_id,
                robot_serial_id=robot_id,
                tested_by=str(uuid.uuid4()),  # Replace with actual user ID
                status=TestStatus.PASS,
                measurements={
                    "result": "pass",
                    "timestamp": datetime.now().isoformat()
                },
                notes=f"{protocol_name} completed successfully"
            )
            test_results.append(result)
            print(f"✓ {protocol_name}: PASS")

        except APIError as e:
            print(f"✗ {protocol_name}: FAIL ({e})")

    print(f"\n  Tests passed: {len(test_results)}/{len(test_protocols)}")
    print()

    # Step 4: Update robot status to ready
    print("[4] Marking robot ready for shipment...")
    try:
        updated_robot = client.robots.update(
            robot_id,
            status=RobotStatus.READY.value,
            manufacturing_date=datetime.now().isoformat()
        )
        print(f"✓ Robot status updated to: {updated_robot['status']}")
        print()
    except APIError as e:
        print(f"✗ Error updating robot status: {e}")
        print()

    # Step 5: Get complete genealogy
    print("[5] Retrieving complete genealogy...")
    try:
        genealogy = client.robots.get_genealogy(robot_id)
        print(f"✓ Genealogy retrieved")
        print(f"  Robot: {genealogy['robot_serial_number']}")
        print(f"  Model: {genealogy['robot_model']}")
        print(f"  Status: {genealogy['robot_status']}")
        print(f"  Components: {len(genealogy['components'])}")
        print()
        print("  Installed components:")
        for comp in genealogy['components']:
            print(f"    - {comp['component_serial_number']} ({comp['component_type']}) at {comp['position']}")
        print()
    except APIError as e:
        print(f"✗ Error retrieving genealogy: {e}")
        print()

    # Summary
    print("=" * 60)
    print("Build Complete!")
    print("=" * 60)
    print(f"Robot Serial: {robot['serial_number']}")
    print(f"Robot ID: {robot_id}")
    print(f"Components Installed: {len(installed_components)}")
    print(f"Tests Passed: {len(test_results)}/{len(test_protocols)}")
    print(f"Status: READY FOR SHIPMENT")
    print()


if __name__ == "__main__":
    main()
