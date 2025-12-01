"""
StateSet Manufacturing API - Python Client Package

A comprehensive Python client library for interacting with the StateSet Manufacturing API.
"""

from .stateset_manufacturing import (
    # Main client
    StateSetManufacturing,

    # Enums
    RobotType,
    RobotStatus,
    ComponentStatus,
    TestStatus,
    NcrSeverity,

    # Exceptions
    StateSetManufacturingError,
    APIError,
)

__version__ = "1.0.0"
__author__ = "StateSet"
__all__ = [
    "StateSetManufacturing",
    "RobotType",
    "RobotStatus",
    "ComponentStatus",
    "TestStatus",
    "NcrSeverity",
    "StateSetManufacturingError",
    "APIError",
]
