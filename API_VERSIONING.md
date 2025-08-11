# Stateset API Versioning Guide

This document outlines how API versioning works in the Stateset API and provides guidelines for
API consumers and developers.

## Versioning Policy

Stateset API uses semantic versioning to manage changes. The versioning format is:

`vX.Y.Z`

Where:
- **X** (Major): Incremented for incompatible API changes that require client modification
- **Y** (Minor): Incremented for backwards-compatible feature additions
- **Z** (Patch): Incremented for backwards-compatible bug fixes and minor changes

## API Version Lifecycle

Each API version follows this lifecycle:

1. **Alpha**: Early preview, subject to breaking changes without notice
2. **Beta**: Feature complete but undergoing testing, breaking changes with notice
3. **Stable**: Production-ready, follows semantic versioning rules
4. **Deprecated**: Still operational but scheduled for removal
5. **Retired**: No longer available

## How to Specify API Version

You can specify which API version to use in three ways:

### 1. URL Path

Include the version in the URL path:

```
https://api.stateset.io/api/v1/orders
```

### 2. Accept Header

Use a versioned media type in the Accept header:

```
Accept: application/vnd.stateset.v1+json
```

### 3. Custom Header

Use the X-API-Version header:

```
X-API-Version: 1
```

If no version is specified, the API defaults to the latest stable version.

## Deprecation Process

When an API version is deprecated:

1. We will announce the deprecation at least 6 months before retiring the version
2. Deprecation warnings will be included in API responses via HTTP headers
3. Documentation will be updated to reflect the deprecation status
4. New features will not be added to deprecated versions

## Breaking vs. Non-Breaking Changes

### Examples of Breaking Changes (Major Version Change)

- Removing or renaming fields in request/response objects
- Changing field types or formats that require client-side parsing changes
- Removing API endpoints
- Changing authentication requirements
- Altering error codes or response formats

### Examples of Non-Breaking Changes (Minor or Patch Version Change)

- Adding new optional fields to request objects
- Adding new fields to response objects (with well-defined defaults for existing clients)
- Adding new API endpoints
- Performance improvements
- Bug fixes that maintain backward compatibility
- Expanding accepted values for existing fields

## Currently Available Versions

| Version | Status | Released | End-of-Life |
|---------|--------|----------|-------------|
| v1      | Stable | 2023-01  | -           |
| v2      | Alpha  | 2024-06  | -           |

## API Version Documentation

Documentation for each API version is available at:

- Latest Version: [/docs](https://api.stateset.io/docs)
- Version 1: [/docs/v1](https://api.stateset.io/docs/v1)
- Version 2 (Alpha): [/docs/v2](https://api.stateset.io/docs/v2)

## For API Developers

When developing new features:

1. Determine if the change is breaking or non-breaking
2. Add new functionality to the appropriate version endpoint
3. Update OpenAPI documentation for the affected version
4. Add appropriate tests for each supported API version
5. Consider migration paths for clients when introducing breaking changes

## Checking Version Status

You can check the status of all API versions at any time by making a GET request to:

```
GET /api/versions
```

Example response:

```json
[
  {
    "version": "v1",
    "status": "stable",
    "documentation_url": "/docs/v1",
    "release_date": "2023-01-01",
    "end_of_life": null
  },
  {
    "version": "v2",
    "status": "alpha",
    "documentation_url": "/docs/v2",
    "release_date": "2024-06-01",
    "end_of_life": null
  }
]
```

## Additional Resources

- [API Change Log](https://stateset.io/changelog)
- [Migration Guides](https://stateset.io/docs/migrations)
- [Breaking Changes Policy](https://stateset.io/docs/breaking-changes)