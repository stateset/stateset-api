#!/usr/bin/env python3
"""
Example 2: Quality Issue Management Workflow

This example demonstrates handling a quality issue:
1. Detect test failure
2. Create NCR (Non-Conformance Report)
3. Update NCR with investigation findings
4. Perform rework
5. Retest
6. Close NCR
"""

from stateset_manufacturing import (
    StateSetManufacturing,
    TestStatus,
    NcrSeverity,
    APIError
)
from datetime import datetime
import uuid
import time


def main():
    # Initialize the client
    client = StateSetManufacturing(
        api_base="http://localhost:3000/api/v1/manufacturing",
        api_token="your_jwt_token_here"
    )

    print("=" * 60)
    print("StateSet Manufacturing - Quality Management Example")
    print("=" * 60)
    print()

    # Assuming robot already exists
    robot_id = "existing-robot-uuid"  # Replace with actual robot ID
    component_id = "faulty-component-uuid"  # Replace with actual component ID
    operator_id = str(uuid.uuid4())  # Replace with actual user ID
    qa_engineer_id = str(uuid.uuid4())  # Replace with actual user ID

    # Step 1: Run test - detect failure
    print("[1] Running positioning accuracy test...")
    try:
        test_result = client.test_results.create(
            test_protocol_id="tp-002",  # Positioning test
            robot_serial_id=robot_id,
            tested_by=operator_id,
            status=TestStatus.FAIL,
            measurements={
                "joint_1_error_mm": 0.03,
                "joint_2_error_mm": 0.04,
                "joint_3_error_mm": 0.85,  # FAILURE
                "joint_4_error_mm": 0.02,
                "joint_5_error_mm": 0.03,
                "joint_6_error_mm": 0.02,
                "max_allowed_error_mm": 0.5,
                "failed_joint": "joint_3"
            },
            notes="Joint 3 positioning error exceeds tolerance. Error measured at 0.85mm (spec: 0.5mm max)."
        )
        print("✗ TEST FAILED: Positioning Accuracy Test")
        print(f"  Joint 3 error: 0.85mm (spec: ≤ 0.5mm)")
        print(f"  Test ID: {test_result['id']}")
        print()
    except APIError as e:
        print(f"Error creating test result: {e}")
        return

    # Step 2: Create NCR
    print("[2] Creating Non-Conformance Report...")
    try:
        ncr = client.ncrs.create(
            ncr_number=f"NCR-{datetime.now().strftime('%Y%m')}-{uuid.uuid4().hex[:5].upper()}",
            robot_serial_id=robot_id,
            component_serial_id=component_id,
            reported_by=operator_id,
            issue_type="dimensional",
            severity=NcrSeverity.MAJOR,
            description=(
                "Joint 3 positioning accuracy exceeds tolerance by 0.35mm during positioning test. "
                "Measured error: 0.85mm (specification: 0.5mm maximum). Issue isolated to joint 3 encoder. "
                "All other joints within specification."
            ),
            detected_at_stage="final_testing",
            assigned_to=qa_engineer_id
        )
        ncr_id = ncr["id"]
        print(f"✓ NCR Created: {ncr['ncr_number']}")
        print(f"  NCR ID: {ncr_id}")
        print(f"  Severity: {ncr['severity']}")
        print(f"  Status: {ncr['status']}")
        print()
    except APIError as e:
        print(f"Error creating NCR: {e}")
        return

    # Step 3: Investigation
    print("[3] Investigating root cause...")
    time.sleep(1)  # Simulate investigation time

    try:
        # Update NCR with investigation findings
        ncr = client.ncrs.update(
            ncr_id,
            status="investigating",
            root_cause=(
                "Joint 3 encoder from supplier lot LOT-2024-Q4-FAULTY arrived with incorrect "
                "factory calibration. Encoder offset values do not match specification datasheet. "
                "Manufacturing inspection records show this lot was flagged for expedited delivery "
                "and may have bypassed final supplier QC."
            ),
            investigation_notes=(
                "Diagnostic testing revealed encoder calibration offset error of +0.42mm. "
                "Compared encoder EEPROM values against reference unit - significant deviation "
                "in zero-position offset. Contacted supplier - they confirmed this lot had "
                "calibration issues and issued recall notice. Two other encoders from same lot "
                "are in inventory and have been quarantined."
            )
        )
        print("✓ Root cause identified:")
        print("  Faulty encoder calibration from supplier")
        print("  Supplier lot: LOT-2024-Q4-FAULTY")
        print("  Status updated to: investigating")
        print()
    except APIError as e:
        print(f"Error updating NCR: {e}")
        return

    # Step 4: Corrective action plan
    print("[4] Creating corrective action plan...")
    time.sleep(1)

    try:
        ncr = client.ncrs.update(
            ncr_id,
            status="action_required",
            corrective_action=(
                "1. Remove faulty encoder from joint 3\n"
                "2. Install replacement encoder from verified good lot (LOT-2024-Q4-GOOD)\n"
                "3. Perform encoder calibration procedure per WI-CAL-003\n"
                "4. Verify calibration with reference gauge block\n"
                "5. Re-run complete positioning accuracy test\n"
                "6. If pass, proceed to full test suite"
            ),
            preventive_action=(
                "Update incoming inspection checklist to include encoder calibration spot-check "
                "for 10% of lots. Implement supplier scorecard review for quality issues. "
                "Schedule supplier audit within 30 days."
            )
        )
        print("✓ Corrective action plan created:")
        print("  1. Replace faulty encoder")
        print("  2. Calibrate replacement")
        print("  3. Retest positioning accuracy")
        print("  4. Run full test suite if pass")
        print()
        print("  Preventive actions:")
        print("  - Enhanced incoming inspection")
        print("  - Supplier quality audit scheduled")
        print()
    except APIError as e:
        print(f"Error updating NCR: {e}")
        return

    # Step 5: Execute rework (simulated)
    print("[5] Executing rework...")
    print("  - Removing faulty encoder...")
    time.sleep(1)
    print("  - Installing replacement encoder...")
    time.sleep(1)
    print("  - Performing calibration...")
    time.sleep(1)
    print("✓ Rework complete")
    print()

    # Step 6: Retest
    print("[6] Retesting after rework...")
    try:
        retest_result = client.test_results.create(
            test_protocol_id="tp-002",  # Positioning test
            robot_serial_id=robot_id,
            tested_by=qa_engineer_id,
            status=TestStatus.PASS,
            measurements={
                "joint_1_error_mm": 0.03,
                "joint_2_error_mm": 0.04,
                "joint_3_error_mm": 0.02,  # PASS
                "joint_4_error_mm": 0.02,
                "joint_5_error_mm": 0.03,
                "joint_6_error_mm": 0.02,
                "max_allowed_error_mm": 0.5,
                "all_joints_pass": True
            },
            notes="Retest after encoder replacement and calibration. All joints now within specification. Joint 3 error reduced from 0.85mm to 0.02mm. Test PASSED."
        )
        print("✓ TEST PASSED: Positioning Accuracy Test")
        print(f"  Joint 3 error: 0.02mm (spec: ≤ 0.5mm)")
        print(f"  Previous: 0.85mm → Current: 0.02mm")
        print(f"  Improvement: 97.6%")
        print()
    except APIError as e:
        print(f"Error creating retest result: {e}")
        return

    # Step 7: Close NCR
    print("[7] Closing Non-Conformance Report...")
    try:
        closed_ncr = client.ncrs.close(
            ncr_id,
            resolution_notes=(
                "Issue successfully resolved through component replacement and calibration.\n\n"
                "Actions Completed:\n"
                "- Faulty encoder removed and quarantined\n"
                "- Replacement encoder installed at joint 3\n"
                "- Calibration procedure WI-CAL-003 completed successfully\n"
                "- All calibration checks passed\n"
                "- Positioning accuracy test re-run: PASS (joint 3 error 0.85mm → 0.02mm)\n"
                "- Verification test suite completed: All tests PASS\n\n"
                "Preventive Actions Implemented:\n"
                "- Incoming inspection updated to include encoder calibration spot-checks\n"
                "- All units from lot LOT-2024-Q4-FAULTY quarantined (2 units)\n"
                "- Supplier corrective action request submitted\n"
                "- Supplier audit scheduled for 2025-01-15\n\n"
                "Robot approved for continued production and final testing."
            ),
            disposition="rework",
            verification_notes="Verified by QA Engineer. All corrective actions completed. Retest results confirm issue resolution. Robot meets all specifications."
        )
        print(f"✓ NCR Closed: {closed_ncr['ncr_number']}")
        print(f"  Status: {closed_ncr['status']}")
        print(f"  Disposition: {closed_ncr['disposition']}")
        print()
    except APIError as e:
        print(f"Error closing NCR: {e}")
        return

    # Summary
    print("=" * 60)
    print("Quality Issue Resolution Complete!")
    print("=" * 60)
    print()
    print("Timeline:")
    print("  1. Test Failure Detected (Joint 3: 0.85mm error)")
    print(f"  2. NCR Created ({ncr['ncr_number']})")
    print("  3. Root Cause Identified (Faulty encoder calibration)")
    print("  4. Corrective Action Planned")
    print("  5. Rework Executed (Encoder replaced)")
    print("  6. Retest: PASS (0.02mm error, 97.6% improvement)")
    print("  7. NCR Closed")
    print()
    print("Impact:")
    print("  Customer Impact: None (caught before shipment)")
    print("  Resolution Time: ~2 hours")
    print("  Preventive Actions: Implemented")
    print()


if __name__ == "__main__":
    main()
