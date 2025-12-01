"""
StateSet Manufacturing API - Python Client Library

A comprehensive Python client for interacting with the StateSet Manufacturing API.
Supports both synchronous and asynchronous operations.

Example Usage:
    from stateset_manufacturing import StateSetManufacturing

    client = StateSetManufacturing(
        api_base="http://localhost:3000/api/v1/manufacturing",
        api_token="your_jwt_token"
    )

    # Create a robot
    robot = client.robots.create(
        serial_number="IR6000-202412-00100",
        robot_model="IR-6000",
        robot_type="articulated_arm"
    )
"""

import requests
from typing import Dict, List, Optional, Any, Union
from datetime import datetime, date
from enum import Enum
import json


class RobotType(Enum):
    """Robot types supported by StateSet Manufacturing"""
    ARTICULATED_ARM = "articulated_arm"
    COBOT = "cobot"
    AMR = "amr"
    SPECIALIZED = "specialized"


class RobotStatus(Enum):
    """Robot manufacturing status"""
    IN_PRODUCTION = "in_production"
    TESTING = "testing"
    READY = "ready"
    SHIPPED = "shipped"
    IN_SERVICE = "in_service"
    RETURNED = "returned"
    DECOMMISSIONED = "decommissioned"


class ComponentStatus(Enum):
    """Component status"""
    IN_STOCK = "in_stock"
    ALLOCATED = "allocated"
    INSTALLED = "installed"
    FAILED = "failed"
    RETURNED = "returned"


class TestStatus(Enum):
    """Test result status"""
    PASS = "pass"
    FAIL = "fail"
    RETEST = "retest"


class NcrSeverity(Enum):
    """NCR severity levels"""
    CRITICAL = "critical"
    MAJOR = "major"
    MINOR = "minor"


class StateSetManufacturingError(Exception):
    """Base exception for StateSet Manufacturing API errors"""
    pass


class APIError(StateSetManufacturingError):
    """API request failed"""
    def __init__(self, status_code: int, message: str):
        self.status_code = status_code
        self.message = message
        super().__init__(f"API Error {status_code}: {message}")


class RobotsAPI:
    """Robot serial numbers API endpoints"""

    def __init__(self, client):
        self.client = client

    def create(
        self,
        serial_number: str,
        robot_model: str,
        robot_type: Union[RobotType, str],
        product_id: str,
        work_order_id: Optional[str] = None,
        manufacturing_date: Optional[datetime] = None,
        customer_id: Optional[str] = None,
        order_id: Optional[str] = None,
        **kwargs
    ) -> Dict[str, Any]:
        """
        Create a new robot serial number

        Args:
            serial_number: Unique serial number for the robot
            robot_model: Model designation (e.g., "IR-6000")
            robot_type: Type of robot (articulated_arm, cobot, amr, specialized)
            product_id: UUID of the product
            work_order_id: Optional UUID of work order
            manufacturing_date: Optional manufacturing date
            customer_id: Optional customer UUID
            order_id: Optional order UUID

        Returns:
            Dict containing the created robot data

        Example:
            robot = client.robots.create(
                serial_number="IR6000-202412-00100",
                robot_model="IR-6000",
                robot_type=RobotType.ARTICULATED_ARM,
                product_id="aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"
            )
        """
        if isinstance(robot_type, RobotType):
            robot_type = robot_type.value

        data = {
            "serial_number": serial_number,
            "robot_model": robot_model,
            "robot_type": robot_type,
            "product_id": product_id,
            "work_order_id": work_order_id,
            "customer_id": customer_id,
            "order_id": order_id,
            **kwargs
        }

        if manufacturing_date:
            data["manufacturing_date"] = manufacturing_date.isoformat()

        return self.client._post("/robots/serials", data)

    def get(self, robot_id: str) -> Dict[str, Any]:
        """
        Get robot by ID

        Args:
            robot_id: UUID of the robot

        Returns:
            Dict containing robot data
        """
        return self.client._get(f"/robots/serials/{robot_id}")

    def list(
        self,
        status: Optional[Union[RobotStatus, str]] = None,
        robot_type: Optional[Union[RobotType, str]] = None,
        robot_model: Optional[str] = None,
        customer_id: Optional[str] = None,
        limit: int = 50,
        offset: int = 0
    ) -> Dict[str, Any]:
        """
        List robots with optional filters

        Args:
            status: Filter by robot status
            robot_type: Filter by robot type
            robot_model: Filter by model name
            customer_id: Filter by customer
            limit: Results per page (max 100)
            offset: Pagination offset

        Returns:
            Dict with 'data' (list of robots) and 'total' (count)
        """
        params = {
            "limit": limit,
            "offset": offset
        }

        if status:
            params["status"] = status.value if isinstance(status, RobotStatus) else status
        if robot_type:
            params["robot_type"] = robot_type.value if isinstance(robot_type, RobotType) else robot_type
        if robot_model:
            params["robot_model"] = robot_model
        if customer_id:
            params["customer_id"] = customer_id

        return self.client._get("/robots/serials", params=params)

    def update(self, robot_id: str, **kwargs) -> Dict[str, Any]:
        """
        Update robot serial number

        Args:
            robot_id: UUID of the robot
            **kwargs: Fields to update (status, manufacturing_date, ship_date, etc.)

        Returns:
            Dict containing updated robot data
        """
        return self.client._put(f"/robots/serials/{robot_id}", kwargs)

    def get_genealogy(self, robot_id: str) -> Dict[str, Any]:
        """
        Get complete component traceability for a robot

        Args:
            robot_id: UUID of the robot

        Returns:
            Dict with robot info and list of installed components
        """
        return self.client._get(f"/robots/serials/{robot_id}/genealogy")


class ComponentsAPI:
    """Component serial numbers API endpoints"""

    def __init__(self, client):
        self.client = client

    def create(
        self,
        serial_number: str,
        component_type: str,
        component_sku: str,
        supplier_id: Optional[str] = None,
        supplier_lot_number: Optional[str] = None,
        manufacture_date: Optional[date] = None,
        receive_date: Optional[date] = None,
        location: Optional[str] = None,
        **kwargs
    ) -> Dict[str, Any]:
        """
        Create a new component serial number

        Args:
            serial_number: Unique serial number
            component_type: Type (servo_motor, controller, encoder, etc.)
            component_sku: SKU/part number
            supplier_id: Optional supplier UUID
            supplier_lot_number: Optional lot number
            manufacture_date: Optional manufacture date
            receive_date: Optional receive date
            location: Optional warehouse location

        Returns:
            Dict containing the created component data
        """
        data = {
            "serial_number": serial_number,
            "component_type": component_type,
            "component_sku": component_sku,
            "supplier_id": supplier_id,
            "supplier_lot_number": supplier_lot_number,
            "location": location,
            **kwargs
        }

        if manufacture_date:
            data["manufacture_date"] = manufacture_date.isoformat()
        if receive_date:
            data["receive_date"] = receive_date.isoformat()

        return self.client._post("/components/serials", data)

    def install(
        self,
        robot_serial_id: str,
        component_serial_id: str,
        position: str,
        installed_by: str
    ) -> Dict[str, Any]:
        """
        Install a component into a robot

        Args:
            robot_serial_id: UUID of the robot
            component_serial_id: UUID of the component
            position: Installation position (e.g., "joint_1")
            installed_by: UUID of installer

        Returns:
            Dict with success message
        """
        data = {
            "robot_serial_id": robot_serial_id,
            "component_serial_id": component_serial_id,
            "position": position,
            "installed_by": installed_by
        }
        return self.client._post("/components/install", data)


class TestProtocolsAPI:
    """Test protocols API endpoints"""

    def __init__(self, client):
        self.client = client

    def create(
        self,
        protocol_number: str,
        name: str,
        test_type: str,
        description: Optional[str] = None,
        applicable_models: Optional[List[str]] = None,
        pass_criteria: Optional[Dict] = None,
        procedure_steps: Optional[Dict] = None,
        **kwargs
    ) -> Dict[str, Any]:
        """
        Create a new test protocol

        Args:
            protocol_number: Protocol identifier (e.g., "TP-001")
            name: Protocol name
            test_type: Type (mechanical, electrical, software, safety)
            description: Optional description
            applicable_models: Optional list of robot models
            pass_criteria: Optional dict of pass criteria
            procedure_steps: Optional dict of procedure steps

        Returns:
            Dict containing the created protocol data
        """
        data = {
            "protocol_number": protocol_number,
            "name": name,
            "test_type": test_type,
            "description": description,
            "applicable_models": applicable_models,
            "pass_criteria": pass_criteria,
            "procedure_steps": procedure_steps,
            **kwargs
        }
        return self.client._post("/test-protocols", data)

    def list(self) -> List[Dict[str, Any]]:
        """
        List all test protocols

        Returns:
            List of test protocol dicts
        """
        return self.client._get("/test-protocols")


class TestResultsAPI:
    """Test results API endpoints"""

    def __init__(self, client):
        self.client = client

    def create(
        self,
        test_protocol_id: str,
        tested_by: str,
        status: Union[TestStatus, str],
        robot_serial_id: Optional[str] = None,
        component_serial_id: Optional[str] = None,
        work_order_id: Optional[str] = None,
        measurements: Optional[Dict] = None,
        notes: Optional[str] = None,
        **kwargs
    ) -> Dict[str, Any]:
        """
        Record a test result

        Args:
            test_protocol_id: UUID of test protocol
            tested_by: UUID of tester
            status: Test status (pass, fail, retest)
            robot_serial_id: Optional robot UUID
            component_serial_id: Optional component UUID
            work_order_id: Optional work order UUID
            measurements: Optional dict of measurement data
            notes: Optional notes

        Returns:
            Dict containing the test result data
        """
        if isinstance(status, TestStatus):
            status = status.value

        data = {
            "test_protocol_id": test_protocol_id,
            "tested_by": tested_by,
            "status": status,
            "robot_serial_id": robot_serial_id,
            "component_serial_id": component_serial_id,
            "work_order_id": work_order_id,
            "measurements": measurements,
            "notes": notes,
            **kwargs
        }
        return self.client._post("/test-results", data)

    def get_robot_results(self, robot_id: str) -> List[Dict[str, Any]]:
        """
        Get all test results for a robot

        Args:
            robot_id: UUID of the robot

        Returns:
            List of test result dicts
        """
        return self.client._get(f"/robots/{robot_id}/test-results")


class NCRsAPI:
    """Non-Conformance Reports API endpoints"""

    def __init__(self, client):
        self.client = client

    def create(
        self,
        ncr_number: str,
        reported_by: str,
        issue_type: str,
        severity: Union[NcrSeverity, str],
        description: str,
        robot_serial_id: Optional[str] = None,
        component_serial_id: Optional[str] = None,
        work_order_id: Optional[str] = None,
        assigned_to: Optional[str] = None,
        **kwargs
    ) -> Dict[str, Any]:
        """
        Create a non-conformance report

        Args:
            ncr_number: NCR number (e.g., "NCR-202412-00001")
            reported_by: UUID of reporter
            issue_type: Type (dimensional, material, process, documentation)
            severity: Severity level (critical, major, minor)
            description: Issue description
            robot_serial_id: Optional robot UUID
            component_serial_id: Optional component UUID
            work_order_id: Optional work order UUID
            assigned_to: Optional assignee UUID

        Returns:
            Dict containing the NCR data
        """
        if isinstance(severity, NcrSeverity):
            severity = severity.value

        data = {
            "ncr_number": ncr_number,
            "reported_by": reported_by,
            "issue_type": issue_type,
            "severity": severity,
            "description": description,
            "robot_serial_id": robot_serial_id,
            "component_serial_id": component_serial_id,
            "work_order_id": work_order_id,
            "assigned_to": assigned_to,
            **kwargs
        }
        return self.client._post("/ncrs", data)

    def list(
        self,
        status: Optional[str] = None,
        severity: Optional[Union[NcrSeverity, str]] = None,
        robot_serial_id: Optional[str] = None,
        assigned_to: Optional[str] = None,
        limit: int = 50,
        offset: int = 0
    ) -> List[Dict[str, Any]]:
        """
        List NCRs with optional filters

        Args:
            status: Filter by status
            severity: Filter by severity
            robot_serial_id: Filter by robot
            assigned_to: Filter by assignee
            limit: Results per page
            offset: Pagination offset

        Returns:
            List of NCR dicts
        """
        params = {
            "limit": limit,
            "offset": offset
        }

        if status:
            params["status"] = status
        if severity:
            params["severity"] = severity.value if isinstance(severity, NcrSeverity) else severity
        if robot_serial_id:
            params["robot_serial_id"] = robot_serial_id
        if assigned_to:
            params["assigned_to"] = assigned_to

        return self.client._get("/ncrs", params=params)

    def update(self, ncr_id: str, **kwargs) -> Dict[str, Any]:
        """
        Update an NCR

        Args:
            ncr_id: UUID of the NCR
            **kwargs: Fields to update

        Returns:
            Dict containing updated NCR data
        """
        return self.client._put(f"/ncrs/{ncr_id}", kwargs)

    def close(
        self,
        ncr_id: str,
        resolution_notes: str,
        disposition: str,
        verification_notes: Optional[str] = None
    ) -> Dict[str, Any]:
        """
        Close an NCR

        Args:
            ncr_id: UUID of the NCR
            resolution_notes: Resolution description
            disposition: Disposition (rework, scrap, use_as_is, return_to_supplier)
            verification_notes: Optional verification notes

        Returns:
            Dict containing closed NCR data
        """
        data = {
            "resolution_notes": resolution_notes,
            "disposition": disposition,
            "verification_notes": verification_notes
        }
        return self.client._post(f"/ncrs/{ncr_id}/close", data)


class ProductionAPI:
    """Production metrics and lines API endpoints"""

    def __init__(self, client):
        self.client = client

    def create_metrics(
        self,
        production_line_id: str,
        production_date: date,
        shift: str,
        work_order_id: str,
        robot_model: str,
        planned_quantity: int,
        actual_quantity: int,
        quantity_passed: Optional[int] = None,
        quantity_failed: Optional[int] = None,
        planned_hours: Optional[float] = None,
        actual_hours: Optional[float] = None,
        downtime_hours: Optional[float] = None,
        downtime_reason: Optional[str] = None,
        **kwargs
    ) -> Dict[str, Any]:
        """
        Create production metrics

        Args:
            production_line_id: UUID of production line
            production_date: Date of production
            shift: Shift name (morning, afternoon, night)
            work_order_id: Work order UUID
            robot_model: Robot model being produced
            planned_quantity: Planned units
            actual_quantity: Actual units produced
            quantity_passed: Units that passed QA
            quantity_failed: Units that failed QA
            planned_hours: Planned hours
            actual_hours: Actual hours
            downtime_hours: Downtime hours
            downtime_reason: Reason for downtime

        Returns:
            Dict containing the metrics data
        """
        data = {
            "production_line_id": production_line_id,
            "production_date": production_date.isoformat(),
            "shift": shift,
            "work_order_id": work_order_id,
            "robot_model": robot_model,
            "planned_quantity": planned_quantity,
            "actual_quantity": actual_quantity,
            "quantity_passed": quantity_passed,
            "quantity_failed": quantity_failed,
            "planned_hours": planned_hours,
            "actual_hours": actual_hours,
            "downtime_hours": downtime_hours,
            "downtime_reason": downtime_reason,
            **kwargs
        }
        return self.client._post("/production-metrics", data)

    def get_metrics(
        self,
        production_date: Optional[date] = None,
        production_line_id: Optional[str] = None
    ) -> List[Dict[str, Any]]:
        """
        Get production metrics

        Args:
            production_date: Optional date filter
            production_line_id: Optional production line filter

        Returns:
            List of metrics dicts
        """
        params = {}
        if production_date:
            params["production_date"] = production_date.isoformat()
        if production_line_id:
            params["production_line_id"] = production_line_id

        return self.client._get("/production-metrics", params=params)

    def create_line(
        self,
        line_code: str,
        line_name: str,
        line_type: str,
        capacity_per_shift: int,
        status: str = "operational",
        **kwargs
    ) -> Dict[str, Any]:
        """
        Create a production line

        Args:
            line_code: Line code (e.g., "ASSY-LINE-01")
            line_name: Line name
            line_type: Type (assembly, testing, packaging)
            capacity_per_shift: Units per shift capacity
            status: Line status (operational, maintenance, offline)

        Returns:
            Dict containing the line data
        """
        data = {
            "line_code": line_code,
            "line_name": line_name,
            "line_type": line_type,
            "capacity_per_shift": capacity_per_shift,
            "status": status,
            **kwargs
        }
        return self.client._post("/production-lines", data)

    def list_lines(self) -> List[Dict[str, Any]]:
        """
        List all production lines

        Returns:
            List of production line dicts
        """
        return self.client._get("/production-lines")


class StateSetManufacturing:
    """
    StateSet Manufacturing API Client

    Provides a high-level interface to the StateSet Manufacturing API.

    Args:
        api_base: Base URL for the API (e.g., "http://localhost:3000/api/v1/manufacturing")
        api_token: JWT authentication token
        timeout: Request timeout in seconds (default: 30)

    Example:
        client = StateSetManufacturing(
            api_base="http://localhost:3000/api/v1/manufacturing",
            api_token="your_jwt_token_here"
        )

        # Create a robot
        robot = client.robots.create(
            serial_number="IR6000-202412-00100",
            robot_model="IR-6000",
            robot_type="articulated_arm",
            product_id="product-uuid"
        )

        # Get robot details
        robot_details = client.robots.get(robot["id"])

        # Create a test result
        result = client.test_results.create(
            test_protocol_id="protocol-uuid",
            robot_serial_id=robot["id"],
            tested_by="user-uuid",
            status="pass"
        )
    """

    def __init__(self, api_base: str, api_token: str, timeout: int = 30):
        self.api_base = api_base.rstrip("/")
        self.api_token = api_token
        self.timeout = timeout

        # Initialize API endpoints
        self.robots = RobotsAPI(self)
        self.components = ComponentsAPI(self)
        self.test_protocols = TestProtocolsAPI(self)
        self.test_results = TestResultsAPI(self)
        self.ncrs = NCRsAPI(self)
        self.production = ProductionAPI(self)

    def _get_headers(self) -> Dict[str, str]:
        """Get HTTP headers for API requests"""
        return {
            "Authorization": f"Bearer {self.api_token}",
            "Content-Type": "application/json",
            "Accept": "application/json"
        }

    def _handle_response(self, response: requests.Response) -> Any:
        """Handle API response"""
        try:
            response.raise_for_status()
            return response.json() if response.content else {}
        except requests.exceptions.HTTPError as e:
            try:
                error_data = response.json()
                message = error_data.get("message", str(e))
            except:
                message = str(e)
            raise APIError(response.status_code, message)

    def _get(self, endpoint: str, params: Optional[Dict] = None) -> Any:
        """Make GET request"""
        url = f"{self.api_base}{endpoint}"
        response = requests.get(
            url,
            headers=self._get_headers(),
            params=params,
            timeout=self.timeout
        )
        return self._handle_response(response)

    def _post(self, endpoint: str, data: Dict) -> Any:
        """Make POST request"""
        url = f"{self.api_base}{endpoint}"
        # Remove None values
        data = {k: v for k, v in data.items() if v is not None}
        response = requests.post(
            url,
            headers=self._get_headers(),
            json=data,
            timeout=self.timeout
        )
        return self._handle_response(response)

    def _put(self, endpoint: str, data: Dict) -> Any:
        """Make PUT request"""
        url = f"{self.api_base}{endpoint}"
        # Remove None values
        data = {k: v for k, v in data.items() if v is not None}
        response = requests.put(
            url,
            headers=self._get_headers(),
            json=data,
            timeout=self.timeout
        )
        return self._handle_response(response)

    def _delete(self, endpoint: str) -> Any:
        """Make DELETE request"""
        url = f"{self.api_base}{endpoint}"
        response = requests.delete(
            url,
            headers=self._get_headers(),
            timeout=self.timeout
        )
        return self._handle_response(response)
