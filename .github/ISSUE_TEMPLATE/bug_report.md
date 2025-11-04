---
name: Bug Report
about: Create a report to help us improve
title: '[BUG] '
labels: bug
assignees: ''
---

## Bug Description

A clear and concise description of what the bug is.

## To Reproduce

Steps to reproduce the behavior:
1. Send request to '...'
2. With payload '...'
3. Observe error '...'

## Expected Behavior

A clear and concise description of what you expected to happen.

## Actual Behavior

What actually happened instead.

## Environment

- **OS**: [e.g., Ubuntu 22.04, macOS 14.0, Windows 11]
- **Rust version**: [e.g., 1.75.0 - run `rustc --version`]
- **API version**: [e.g., 0.1.6 - from `/health/version`]
- **Database**: [e.g., PostgreSQL 15, SQLite]
- **Deployment**: [e.g., Docker, bare metal, Kubernetes]

## Logs

Please provide relevant log output. Include the `X-Request-Id` header value if available.

```
Paste logs here
```

## API Request/Response

If applicable, provide the full request and response (sanitize sensitive data):

**Request:**
```http
POST /api/v1/orders HTTP/1.1
Authorization: Bearer <redacted>
Content-Type: application/json

{
  "customer_id": "...",
  ...
}
```

**Response:**
```json
{
  "error": {
    "code": "...",
    "message": "..."
  }
}
```

## Additional Context

Add any other context about the problem here (screenshots, related issues, etc.).

## Possible Solution

If you have suggestions on how to fix the bug, please share them here.
