# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### Testing Infrastructure
- Property-based testing with proptest and quickcheck
- Fuzz testing infrastructure with libfuzzer (3 fuzz targets: JSON parsing, sanitization, validation)
- Mutation testing CI workflow with cargo-mutants
- Load testing with k6 and performance thresholds
- Mock testing utilities (mockall, wiremock)
- Test utilities (fake, rstest, test-case, assert_matches)
- Comprehensive property-based tests for order validation, email validation, quantity checks

#### Security Enhancements
- Comprehensive security headers middleware (CSP, HSTS 1-year, Permissions-Policy)
- Enhanced XSS prevention with 50+ pattern detection (event handlers, protocols, HTML injection)
- Path traversal attack detection and blocking
- Comprehensive audit logging middleware with action categorization
- Security metrics (auth failures, rate limits, suspicious requests, blocked requests)
- SBOM generation with CycloneDX
- Rust-specific security analysis with clippy SARIF output
- Cargo-geiger unsafe code detection in CI
- Bulk operation rate limiting with configurable limits per operation type
- API key and user-specific rate limit policies

#### Observability
- Database metrics (queries, connections, slow queries, transaction duration, errors)
- Security metrics (auth success/failure, rate limiting, active sessions, permission denials)
- Business metrics (orders, payments, inventory reservations, warranties, shipments delivered)
- HTTP endpoint metrics (latency by endpoint, status code distribution: 2xx/4xx/5xx)
- Circuit breaker metrics with Prometheus export format
- Correlation ID propagation across service boundaries

#### Middleware
- API versioning middleware with deprecation warnings
- Correlation ID middleware for distributed tracing
- Bulk operation rate limiter with per-operation-type configurations
- Enhanced request ID propagation

#### CI/CD
- Release automation workflow with multi-platform builds (linux/darwin, amd64/arm64)
- Docker image building and registry push (GHCR)
- Load testing workflow on PR with performance thresholds
- Mutation testing weekly schedule
- SBOM generation and artifact upload
- Clippy SARIF output for GitHub Security integration

#### Configuration
- clippy.toml for stricter linting rules
- Cognitive complexity threshold configuration
- Disallowed methods/types for security enforcement

### Changed
- HSTS max-age increased from 6 months to 1 year with preload directive
- Enhanced input sanitization with comprehensive XSS pattern matching
- Improved documentation structure
- Security workflow now includes Rust-specific analysis
- Circuit breaker metrics now include total calls, failures, successes, and state transitions
- Rate limiter now supports RFC 9447 headers (RateLimit-*)

### Security
- Added Content-Security-Policy header for API endpoints
- Added X-Permitted-Cross-Domain-Policies header
- Added Permissions-Policy header to disable unnecessary browser features
- Enhanced referrer-policy to strict-origin-when-cross-origin
- Added cache-control headers to prevent caching of sensitive data
- Added X-XSS-Protection header for legacy browser support
- Server header now returns generic "StateSet-API" instead of framework details

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
