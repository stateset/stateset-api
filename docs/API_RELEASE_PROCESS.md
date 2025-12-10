# Stateset API Release Process

This document outlines the process for releasing new API versions and features in the Stateset API.

## Release Cadence

* **Patch Releases** (vX.Y.Z): As needed for bug fixes and minor improvements
* **Minor Releases** (vX.Y.0): Monthly for new features (backward compatible)
* **Major Releases** (vX.0.0): Scheduled 2-3 times per year for breaking changes

## Release Process

### 1. Planning Phase

1. Identify features, improvements, and fixes for the release
2. Determine the appropriate version number based on semantic versioning
3. Create a release plan with target dates
4. Assign owners to each feature or fix

### 2. Development Phase

1. Create a release branch from main (`release/vX.Y.Z`)
2. Implement features and fixes in feature branches
3. Submit pull requests to the release branch
4. Review and merge approved changes
5. Update OpenAPI documentation for all changes
6. Update changelog with all significant changes

### 3. Testing Phase

1. Perform comprehensive testing of the release branch
   * Unit tests
   * Integration tests
   * Load testing
   * Security scanning
   * Compatibility testing with previous versions
2. Generate test reports and update release documentation

### 4. Documentation Phase

1. Update API documentation for the new version
2. Create migration guides for breaking changes
3. Update example code and SDK documentation
4. Prepare release notes

### 5. Pre-Release Phase

1. Deploy to staging environment
2. Perform final testing and validation
3. Send release announcement to beta testers
4. Gather and address feedback

### 6. Release Phase

1. Deploy to production
2. Tag the release in the repository
3. Publish release notes and updated documentation
4. Send release announcement to users

### 7. Post-Release Phase

1. Monitor for issues
2. Provide support for the new release
3. Collect feedback for future improvements
4. Schedule patch releases if critical issues are discovered

## API Version Stages

Each API version progresses through these stages:

### Alpha Stage

* Early development preview
* Not recommended for production use
* Breaking changes may occur without notice
* Limited documentation
* Available on developer sandbox only

### Beta Stage

* Feature complete but undergoing testing
* Breaking changes may occur with notice
* Comprehensive documentation available
* Available on staging environment
* Suitable for integration testing but not full production

### Stable Stage

* Production-ready
* Follows semantic versioning rules
* No breaking changes without a major version bump
* Full documentation and support
* Available in production environment

### Deprecated Stage

* Still operational but scheduled for removal
* No new features added
* Bug fixes only for security or critical issues
* Migration guides available to newer versions
* Deprecation warnings in API responses

### Retired Stage

* No longer available
* Returns 410 Gone status code
* Documentation archived for reference

## Change Classification

### Breaking Changes (Major Version)

* Removing endpoints, parameters, or response fields
* Changing field types or formats that require client parsing changes
* Modifying error codes or response structure
* Changing required fields
* Altering authentication requirements

### Non-Breaking Changes (Minor Version)

* Adding new endpoints or resources
* Adding optional fields to requests
* Adding new fields to responses
* Expanding accepted values without changing meaning
* Performance improvements

### Bug Fixes (Patch Version)

* Fixing incorrect behavior
* Security patches
* Performance improvements
* Documentation corrections

## Backwards Compatibility

For Minor and Patch releases:

* All existing clients should continue to work without modification
* New optional features should degrade gracefully when not used
* Changes should be tested against previous client versions

## Supporting Multiple API Versions

* We support the current stable version and one previous major version
* A deprecated version will continue to be available for at least 6 months
* Version-specific documentation remains available even for deprecated versions

## Versioning in Headers

All API responses include version information in headers:

```
X-API-Version: v1.2.3
```

Deprecated API versions include additional headers:

```
Warning: 299 - "API v1 is deprecated and will be removed on 2023-12-31. Please migrate to v2"
X-API-Deprecated: true
```

## Emergency Releases

For critical security issues or major bugs:

1. Assess severity and impact
2. Create a hotfix branch from the affected release branch
3. Implement the necessary fixes
4. Fast-track the testing and approval process
5. Deploy as an emergency patch release
6. Send specific notification about the security or bug fix

## Version Status API

You can check the status of API versions at any time with:

```
GET /api/versions
```