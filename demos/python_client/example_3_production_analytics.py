#!/usr/bin/env python3
"""
Example 3: Production Analytics & Dashboards

This example demonstrates production monitoring and analytics:
1. Create production lines
2. Record production metrics
3. Calculate OEE (Overall Equipment Effectiveness)
4. Generate production reports
5. Analyze quality trends
"""

from stateset_manufacturing import (
    StateSetManufacturing,
    APIError
)
from datetime import date, timedelta
from decimal import Decimal
import uuid


def calculate_oee(metrics):
    """Calculate OEE from production metrics"""
    availability = (metrics['planned_hours'] - metrics['downtime_hours']) / metrics['planned_hours']
    performance = metrics['actual_quantity'] / metrics['planned_quantity']
    quality = metrics['quantity_passed'] / metrics['actual_quantity'] if metrics['actual_quantity'] > 0 else 0
    oee = availability * performance * quality * 100
    return {
        'availability': availability * 100,
        'performance': performance * 100,
        'quality': quality * 100,
        'oee': oee
    }


def main():
    # Initialize the client
    client = StateSetManufacturing(
        api_base="http://localhost:3000/api/v1/manufacturing",
        api_token="your_jwt_token_here"
    )

    print("=" * 70)
    print("StateSet Manufacturing - Production Analytics Example")
    print("=" * 70)
    print()

    today = date.today()

    # Step 1: Create production lines
    print("[1] Setting up production lines...")
    production_lines = []

    try:
        # Assembly line
        assembly_line = client.production.create_line(
            line_code="ASSY-LINE-01",
            line_name="Main Assembly Line 1",
            line_type="assembly",
            capacity_per_shift=8,
            status="operational"
        )
        production_lines.append(assembly_line)
        print(f"✓ Created: {assembly_line['line_name']}")

        # Test line
        test_line = client.production.create_line(
            line_code="TEST-LINE-01",
            line_name="Final Test Cell 1",
            line_type="testing",
            capacity_per_shift=12,
            status="operational"
        )
        production_lines.append(test_line)
        print(f"✓ Created: {test_line['line_name']}")
        print()

    except APIError as e:
        print(f"Error creating production lines: {e}")
        return

    # Step 2: Record production metrics
    print("[2] Recording production metrics for today...")
    work_order_id = str(uuid.uuid4())  # Replace with actual work order ID

    metrics_data = [
        {
            'line': assembly_line,
            'shift': 'morning',
            'planned_quantity': 8,
            'actual_quantity': 7,
            'quantity_passed': 6,
            'quantity_failed': 1,
            'planned_hours': 8.0,
            'actual_hours': 8.5,
            'downtime_hours': 0.5,
            'downtime_reason': 'Component delivery delay'
        },
        {
            'line': assembly_line,
            'shift': 'afternoon',
            'planned_quantity': 8,
            'actual_quantity': 8,
            'quantity_passed': 8,
            'quantity_failed': 0,
            'planned_hours': 8.0,
            'actual_hours': 7.8,
            'downtime_hours': 0.2,
            'downtime_reason': 'Planned maintenance'
        },
        {
            'line': test_line,
            'shift': 'morning',
            'planned_quantity': 12,
            'actual_quantity': 11,
            'quantity_passed': 10,
            'quantity_failed': 1,
            'planned_hours': 8.0,
            'actual_hours': 8.2,
            'downtime_hours': 0.3,
            'downtime_reason': 'Test equipment calibration'
        },
        {
            'line': test_line,
            'shift': 'afternoon',
            'planned_quantity': 12,
            'actual_quantity': 12,
            'quantity_passed': 12,
            'quantity_failed': 0,
            'planned_hours': 8.0,
            'actual_hours': 7.9,
            'downtime_hours': 0.1,
            'downtime_reason': 'None'
        }
    ]

    recorded_metrics = []
    for data in metrics_data:
        try:
            metrics = client.production.create_metrics(
                production_line_id=data['line']['id'],
                production_date=today,
                shift=data['shift'],
                work_order_id=work_order_id,
                robot_model="IR-6000",
                planned_quantity=data['planned_quantity'],
                actual_quantity=data['actual_quantity'],
                quantity_passed=data['quantity_passed'],
                quantity_failed=data['quantity_failed'],
                planned_hours=data['planned_hours'],
                actual_hours=data['actual_hours'],
                downtime_hours=data['downtime_hours'],
                downtime_reason=data['downtime_reason']
            )
            recorded_metrics.append({**metrics, **data})
            print(f"✓ {data['line']['line_name']} - {data['shift']}: {data['actual_quantity']} units")
        except APIError as e:
            print(f"Error recording metrics: {e}")

    print()

    # Step 3: Generate production dashboard
    print("[3] Production Dashboard")
    print("=" * 70)
    print()

    # Overall production summary
    total_produced = sum(m['actual_quantity'] for m in recorded_metrics)
    total_passed = sum(m['quantity_passed'] for m in recorded_metrics)
    total_failed = sum(m['quantity_failed'] for m in recorded_metrics)
    first_pass_yield = (total_passed / total_produced * 100) if total_produced > 0 else 0

    print("PRODUCTION SUMMARY")
    print("-" * 70)
    print(f"Date: {today}")
    print(f"Total Robots Produced: {total_produced} units")
    print(f"Robots Passed QA: {total_passed} units")
    print(f"Robots Failed QA: {total_failed} units")
    print(f"First-Pass Yield: {first_pass_yield:.1f}%")
    print()

    # Production by line
    print("PRODUCTION BY LINE")
    print("-" * 70)

    for line in production_lines:
        line_metrics = [m for m in recorded_metrics if m['line']['id'] == line['id']]
        line_produced = sum(m['actual_quantity'] for m in line_metrics)
        line_passed = sum(m['quantity_passed'] for m in line_metrics)
        line_failed = sum(m['quantity_failed'] for m in line_metrics)
        line_planned = sum(m['planned_quantity'] for m in line_metrics)
        utilization = (line_produced / line_planned * 100) if line_planned > 0 else 0

        print(f"\n{line['line_name']} ({line['line_code']})")
        print(f"  Planned: {line_planned} units | Actual: {line_produced} units")
        print(f"  Passed: {line_passed} | Failed: {line_failed}")
        print(f"  Utilization: {utilization:.1f}%")

    print()
    print()

    # Step 4: Calculate OEE
    print("[4] OEE Analysis (Overall Equipment Effectiveness)")
    print("=" * 70)
    print()

    # Assembly line OEE
    assembly_metrics = [m for m in recorded_metrics if m['line']['id'] == assembly_line['id']]

    assembly_oee_data = {
        'planned_quantity': sum(m['planned_quantity'] for m in assembly_metrics),
        'actual_quantity': sum(m['actual_quantity'] for m in assembly_metrics),
        'quantity_passed': sum(m['quantity_passed'] for m in assembly_metrics),
        'planned_hours': sum(m['planned_hours'] for m in assembly_metrics),
        'downtime_hours': sum(m['downtime_hours'] for m in assembly_metrics)
    }

    assembly_oee = calculate_oee(assembly_oee_data)

    print("Assembly Line OEE:")
    print(f"  Availability: {assembly_oee['availability']:.1f}%")
    print(f"  Performance:  {assembly_oee['performance']:.1f}%")
    print(f"  Quality:      {assembly_oee['quality']:.1f}%")
    print(f"  Overall OEE:  {assembly_oee['oee']:.1f}%", end="")
    if assembly_oee['oee'] >= 85:
        print(" ✓ World Class!")
    else:
        print(f" (Target: 85%+)")
    print()

    # Test line OEE
    test_metrics = [m for m in recorded_metrics if m['line']['id'] == test_line['id']]

    test_oee_data = {
        'planned_quantity': sum(m['planned_quantity'] for m in test_metrics),
        'actual_quantity': sum(m['actual_quantity'] for m in test_metrics),
        'quantity_passed': sum(m['quantity_passed'] for m in test_metrics),
        'planned_hours': sum(m['planned_hours'] for m in test_metrics),
        'downtime_hours': sum(m['downtime_hours'] for m in test_metrics)
    }

    test_oee = calculate_oee(test_oee_data)

    print("Test Line OEE:")
    print(f"  Availability: {test_oee['availability']:.1f}%")
    print(f"  Performance:  {test_oee['performance']:.1f}%")
    print(f"  Quality:      {test_oee['quality']:.1f}%")
    print(f"  Overall OEE:  {test_oee['oee']:.1f}%", end="")
    if test_oee['oee'] >= 85:
        print(" ✓ World Class!")
    else:
        print(f" (Target: 85%+)")
    print()
    print()

    # Step 5: Downtime analysis
    print("[5] Downtime Analysis")
    print("=" * 70)
    print()

    total_downtime = sum(m['downtime_hours'] for m in recorded_metrics)
    downtime_breakdown = {}

    for m in recorded_metrics:
        reason = m['downtime_reason']
        downtime_breakdown[reason] = downtime_breakdown.get(reason, 0) + m['downtime_hours']

    print(f"Total Downtime: {total_downtime:.1f} hours")
    print("\nBreakdown:")
    for reason, hours in sorted(downtime_breakdown.items(), key=lambda x: x[1], reverse=True):
        percentage = (hours / total_downtime * 100) if total_downtime > 0 else 0
        print(f"  {reason}: {hours:.1f}h ({percentage:.1f}%)")
    print()
    print()

    # Step 6: Quality metrics
    print("[6] Quality Metrics")
    print("=" * 70)
    print()

    defect_rate = (total_failed / total_produced * 100) if total_produced > 0 else 0
    scrap_rate = 0.0  # Assuming no scrap in this example
    rework_rate = defect_rate  # All failures were reworked

    print("Quality Performance:")
    print(f"  First-Pass Yield: {first_pass_yield:.1f}%")
    print(f"  Defect Rate: {defect_rate:.1f}%")
    print(f"  Rework Rate: {rework_rate:.1f}%")
    print(f"  Scrap Rate: {scrap_rate:.1f}%")
    print()
    print()

    # Step 7: Recommendations
    print("[7] AI-Driven Recommendations")
    print("=" * 70)
    print()

    # Analyze and provide recommendations
    recommendations = []

    # OEE recommendations
    if assembly_oee['oee'] < 85:
        gap = 85 - assembly_oee['oee']
        recommendations.append({
            'priority': 'MEDIUM',
            'item': f"Assembly line OEE is {gap:.1f}% below world-class (85%)",
            'action': "Focus on reducing downtime and improving quality"
        })

    if test_oee['oee'] >= 85:
        recommendations.append({
            'priority': 'SUCCESS',
            'item': f"Test line exceeds world-class OEE ({test_oee['oee']:.1f}%)",
            'action': "Document best practices and share with other lines"
        })

    # Downtime recommendations
    if total_downtime > 0:
        top_downtime = max(downtime_breakdown.items(), key=lambda x: x[1])
        if top_downtime[0] != 'None':
            recommendations.append({
                'priority': 'HIGH',
                'item': f"{top_downtime[0]} accounts for largest downtime ({top_downtime[1]:.1f}h)",
                'action': "Investigate root cause and implement preventive measures"
            })

    # Quality recommendations
    if first_pass_yield < 95:
        recommendations.append({
            'priority': 'MEDIUM',
            'item': f"First-pass yield is {first_pass_yield:.1f}% (target: 95%+)",
            'action': "Review quality control procedures and test protocols"
        })

    # Print recommendations
    for rec in sorted(recommendations, key=lambda x: {'HIGH': 0, 'MEDIUM': 1, 'SUCCESS': 2}[rec['priority']]):
        priority_symbol = {
            'HIGH': '⚠',
            'MEDIUM': '⚡',
            'SUCCESS': '✓'
        }[rec['priority']]

        print(f"{priority_symbol} {rec['priority']}: {rec['item']}")
        print(f"   Action: {rec['action']}")
        print()

    # Summary
    print("=" * 70)
    print("Production Dashboard Complete!")
    print("=" * 70)
    print()
    print("Key Metrics:")
    print(f"  Production: {total_produced} robots ({(total_produced/sum(m['planned_quantity'] for m in recorded_metrics)*100):.0f}% of plan)")
    print(f"  Quality: {first_pass_yield:.1f}% FPY")
    print(f"  Average OEE: {(assembly_oee['oee'] + test_oee['oee'])/2:.1f}%")
    print(f"  Downtime: {total_downtime:.1f} hours ({total_downtime/sum(m['planned_hours'] for m in recorded_metrics)*100:.1f}% of scheduled)")
    print()


if __name__ == "__main__":
    main()
