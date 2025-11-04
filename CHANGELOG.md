# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Test coverage reporting with tarpaulin
- Semantic release automation
- Performance benchmarking infrastructure
- Security scanning with cargo-audit and trivy
- Comprehensive deployment documentation
- Database backup and recovery procedures
- Monitoring and alerting guide

### Changed
- Enhanced CI/CD workflows with additional quality gates
- Improved documentation structure

## [0.1.6] - 2024-10-30

### Added
- Work order management endpoints and handlers
- Analytics endpoints with permission-based access
- Postman collection for API testing
- Advanced shipping notice (ASN) management
- Manufacturing work order tracking
- Bill of materials (BOM) endpoints

### Changed
- Updated API structure for better organization
- Improved error handling and logging
- Enhanced authentication middleware

### Fixed
- Various bug fixes in order processing
- Inventory allocation edge cases
- Return workflow improvements

## [0.1.5] - 2024-09-29

### Added
- Agentic operations platform features
- StablePay crypto payment integration
- AI-powered checkout implementation
- Product feed specifications
- ROI calculator for cost analysis

### Changed
- Enhanced inventory management with lot tracking
- Improved shipment tracking capabilities
- Updated warranty claim processing

### Fixed
- Database migration issues
- Redis connection stability
- Rate limiting edge cases

## [0.1.4] - 2024-08-24

### Added
- Advanced caching with Redis and in-memory strategies
- Cache warming capabilities
- Multi-factor authentication (MFA) support
- Enhanced RBAC with granular permissions
- Password policy enforcement

### Changed
- Migrated to SeaORM from Diesel
- Improved API versioning strategy
- Enhanced OpenAPI documentation

### Fixed
- JWT token refresh issues
- Memory leaks in event processing
- Concurrent inventory reservation bugs

## [0.1.3] - 2024-08-11

### Added
- AI agent commands and automation
- Circuit breaker pattern implementation
- Request idempotency support
- Comprehensive health check endpoints
- OpenTelemetry integration

### Changed
- Refactored service layer for better testability
- Improved error messages and codes
- Enhanced rate limiting with per-path policies

### Fixed
- Database connection pool exhaustion
- Event outbox processing failures
- gRPC service initialization

## [0.1.2] - 2024-07-24

### Added
- gRPC protocol support with 30+ proto definitions
- Swagger UI integration
- Prometheus metrics endpoints
- CLI tool (stateset-cli) for development

### Changed
- Switched to Axum web framework
- Implemented async/await throughout
- Improved request tracing

### Fixed
- Memory usage in large result sets
- Timeout handling in external services

## [0.1.1] - 2024-07-18

### Added
- Initial order management system
- Inventory control with allocations
- Returns processing workflow
- Warranty management
- Shipment tracking

### Changed
- Project structure reorganization
- Improved documentation

### Fixed
- Initial bug fixes and stability improvements

## [0.1.0] - 2024-07-01

### Added
- Initial release of StateSet API
- Basic CRUD operations for orders, inventory, returns
- JWT authentication
- PostgreSQL and SQLite support
- Docker containerization
- Basic CI/CD with GitHub Actions
