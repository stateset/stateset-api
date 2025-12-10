# StateSet API Security Guide

**Version**: 0.2.1
**Last Updated**: December 10, 2025
**Status**: Production Ready (10/10)

---

## Table of Contents

1. [Overview](#overview)
2. [Authentication](#authentication)
3. [Authorization (RBAC)](#authorization-rbac)
4. [OAuth2 Integration](#oauth2-integration)
5. [Multi-Factor Authentication (MFA)](#multi-factor-authentication-mfa)
6. [API Key Management](#api-key-management)
7. [Password Policies](#password-policies)
8. [Rate Limiting](#rate-limiting)
9. [Security Features](#security-features)
10. [Configuration](#configuration)
11. [Best Practices](#best-practices)
12. [API Reference](#api-reference)
13. [Security Checklist](#security-checklist)

---

## Overview

StateSet API implements enterprise-grade security with multiple layers of protection:

### Security Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Client Application                       │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │   Rate Limiting      │
            │   (Redis-backed)     │
            └──────────┬───────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │   Authentication     │
            │   - JWT              │
            │   - API Keys         │
            │   - OAuth2           │
            └──────────┬───────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │   Authorization      │
            │   (RBAC)             │
            └──────────┬───────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │   Input Validation   │
            └──────────┬───────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │   Business Logic     │
            └──────────┬───────────┘
                       │
                       ▼
            ┌──────────────────────┐
            │   Database           │
            │   (Parameterized)    │
            └──────────────────────┘
```

### Security Features at a Glance

✅ **Authentication**: JWT, OAuth2, API Keys
✅ **Authorization**: Role-Based Access Control (RBAC)
✅ **Multi-Factor Authentication**: TOTP, SMS, Email
✅ **Password Security**: Argon2 hashing, policy enforcement
✅ **Rate Limiting**: Per-user, per-API-key, per-endpoint
✅ **Input Validation**: Comprehensive validation on all endpoints
✅ **SQL Injection Protection**: SeaORM parameterized queries
✅ **HMAC Verification**: Webhook signature validation
✅ **Constant-Time Comparisons**: Prevent timing attacks
✅ **Audit Logging**: Comprehensive security event logging
✅ **Memory Safety**: 100% safe Rust (no unsafe code)

---

## Authentication

StateSet API supports three authentication methods:

### 1. JWT Authentication (Primary)

**JWT (JSON Web Tokens)** provide stateless authentication with refresh token support.

#### Token Structure

```json
{
  "sub": "user-uuid",              // Subject (user ID)
  "name": "John Doe",              // User's name
  "email": "john@example.com",     // User's email
  "roles": ["manager"],            // User's roles
  "permissions": ["orders:read"],  // Explicit permissions
  "tenant_id": "tenant-uuid",      // Multi-tenant support
  "jti": "unique-token-id",        // JWT ID
  "iat": 1702000000,               // Issued at
  "exp": 1702003600,               // Expiration (1 hour)
  "nbf": 1702000000,               // Not before
  "iss": "stateset-api",           // Issuer
  "aud": "stateset-client",        // Audience
  "scope": "openid profile"        // OAuth2 scopes
}
```

#### Token Types

**Access Token**:
- Short-lived (default: 15 minutes)
- Used for API requests
- Contains user identity and permissions
- Sent in `Authorization: Bearer <token>` header

**Refresh Token**:
- Long-lived (default: 7 days)
- Used to obtain new access tokens
- Stored securely in database
- Can be revoked individually

#### Login Flow

```
1. POST /api/v1/auth/login
   {
     "email": "user@example.com",
     "password": "secure-password"
   }

2. Response:
   {
     "access_token": "eyJhbGc...",
     "refresh_token": "eyJhbGc...",
     "expires_in": 900,
     "token_type": "Bearer"
   }

3. Use access token:
   GET /api/v1/orders
   Authorization: Bearer eyJhbGc...

4. Refresh when expired:
   POST /api/v1/auth/refresh
   {
     "refresh_token": "eyJhbGc..."
   }
```

#### Token Configuration

```toml
# config/production.toml
jwt_secret = "${JWT_SECRET}"           # 64+ characters required
jwt_expiration = 900                    # 15 minutes (seconds)
refresh_token_expiration = 604800       # 7 days (seconds)
```

**Security Requirements**:
- JWT secret MUST be at least 64 characters
- Secrets under 64 characters are rejected at startup
- Use cryptographically secure random strings
- Rotate secrets periodically

#### Token Revocation

```bash
# Logout (revoke all user's refresh tokens)
POST /api/v1/auth/logout
Authorization: Bearer <access_token>

# Response:
{
  "message": "Successfully logged out"
}
```

### 2. API Key Authentication

**API Keys** provide service-to-service authentication for automated systems.

#### Key Format

```
sk_live_EXAMPLE_KEY_NOT_REAL_DO_NOT_USE
│   │    └─────────────────────────────┘
│   │              Key Value
│   └─── Environment (live/test)
└─────── Prefix (configurable)
```

#### Key Management

**Create API Key**:
```bash
POST /api/v1/auth/api-keys
Authorization: Bearer <access_token>

Request:
{
  "name": "Production Integration",
  "permissions": ["orders:read", "orders:create"],
  "expires_at": "2025-12-31T23:59:59Z"  # Optional
}

Response:
{
  "id": "key-uuid",
  "key": "sk_example_NOT_A_REAL_KEY",  # Only shown once!
  "name": "Production Integration",
  "permissions": ["orders:read", "orders:create"],
  "created_at": "2025-12-10T00:00:00Z"
}
```

**List API Keys**:
```bash
GET /api/v1/auth/api-keys
Authorization: Bearer <access_token>

Response:
{
  "api_keys": [
    {
      "id": "key-uuid",
      "name": "Production Integration",
      "permissions": ["orders:read", "orders:create"],
      "last_used": "2025-12-10T12:00:00Z",
      "created_at": "2025-12-10T00:00:00Z"
      # Note: key value is never returned
    }
  ]
}
```

**Revoke API Key**:
```bash
DELETE /api/v1/auth/api-keys/{id}
Authorization: Bearer <access_token>
```

#### Using API Keys

```bash
# Method 1: X-API-Key header (recommended)
curl -H "X-API-Key: sk_example_NOT_A_REAL_KEY" https://api.stateset.com/api/v1/orders

# Method 2: Authorization header
curl -H "Authorization: ApiKey sk_example_NOT_A_REAL_KEY" https://api.stateset.com/api/v1/orders
```

#### Key Security Features

- ✅ Keys are hashed before storage (SHA-256)
- ✅ Per-key permissions (granular access control)
- ✅ Optional expiration dates
- ✅ Last used timestamp tracking
- ✅ Individual key revocation
- ✅ Rate limiting per API key

### 3. Session-Based Authentication (Optional)

For web applications, session-based authentication is available:

```bash
# Login creates session cookie
POST /api/v1/auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "secure-password",
  "remember_me": true
}

# Response sets secure HTTP-only cookie
Set-Cookie: session=...; HttpOnly; Secure; SameSite=Strict
```

---

## Authorization (RBAC)

StateSet API uses **Role-Based Access Control (RBAC)** for fine-grained authorization.

### Architecture

```
User → Roles → Permissions → Resources
```

- **Users** are assigned one or more **Roles**
- **Roles** contain a set of **Permissions**
- **Permissions** control access to **Resources**

### Built-in Roles

#### 1. Admin Role

**Full system access** - Use sparingly!

```yaml
Role: admin
Permissions:
  - admin:*           # All admin operations
  - users:*           # User management
  - roles:*           # Role management
  - api-keys:*        # API key management
  - orders:*          # All order operations
  - inventory:*       # All inventory operations
  - returns:*         # All returns operations
  - shipments:*       # All shipments operations
  - customers:*       # All customer operations
  - suppliers:*       # All supplier operations
  - warranties:*      # All warranty operations
  - workorders:*      # All work order operations
  - reports:*         # All reporting operations
  - metrics:*         # All metrics access
```

#### 2. Manager Role

**Elevated operational access**

```yaml
Role: manager
Permissions:
  # Orders
  - orders:read
  - orders:create
  - orders:update
  - orders:cancel

  # Inventory
  - inventory:read
  - inventory:adjust
  - inventory:transfer

  # Returns & Warranties
  - returns:*
  - warranties:*

  # Shipments
  - shipments:*

  # Customers & Suppliers
  - customers:read
  - customers:create
  - customers:update
  - suppliers:read
  - suppliers:create
  - suppliers:update

  # Reporting
  - reports:read
  - reports:export

  # Limited API key management
  - api-keys:read
  - api-keys:create
```

#### 3. Warehouse Role

**Warehouse and fulfillment operations**

```yaml
Role: warehouse
Permissions:
  # Orders (read only)
  - orders:read

  # Inventory
  - inventory:read
  - inventory:adjust
  - inventory:transfer

  # Shipments
  - shipments:read
  - shipments:create
  - shipments:update

  # Returns
  - returns:read
  - returns:create

  # Work Orders
  - workorders:read
  - workorders:update
```

#### 4. Sales Role

**Sales and customer service**

```yaml
Role: sales
Permissions:
  # Orders
  - orders:read
  - orders:create
  - orders:update

  # Customers
  - customers:*

  # Products
  - products:read

  # Inventory (read only)
  - inventory:read

  # Returns
  - returns:read
  - returns:create

  # Reporting
  - reports:read
```

#### 5. Customer Service Role

**Support operations**

```yaml
Role: customer_service
Permissions:
  # Orders (read, update status)
  - orders:read
  - orders:update

  # Customers
  - customers:read
  - customers:update

  # Returns
  - returns:*

  # Warranties
  - warranties:read
  - warranties:create

  # Shipments (tracking)
  - shipments:read
```

#### 6. Viewer Role

**Read-only access for reporting**

```yaml
Role: viewer
Permissions:
  - orders:read
  - inventory:read
  - products:read
  - customers:read
  - reports:read
  - metrics:read
```

### Permission Format

Permissions follow the pattern: `resource:action`

**Actions**:
- `read` - View resources
- `create` - Create new resources
- `update` - Modify existing resources
- `delete` - Delete resources
- `manage` - Full CRUD + special operations
- `*` - All actions on resource

**Examples**:
```
orders:read          # View orders
orders:create        # Create orders
orders:*             # All order operations
inventory:adjust     # Adjust inventory quantities
returns:approve      # Approve return requests
admin:outbox         # Access event outbox (admin only)
```

### Resource Types

```
orders              # Order management
products            # Product catalog
inventory           # Inventory management
returns             # Returns processing
shipments           # Shipment tracking
warranties          # Warranty management
workorders          # Work order management
purchaseorders      # Purchase orders
asns                # Advanced shipping notices
customers           # Customer management
suppliers           # Supplier management
boms                # Bill of materials
users               # User management
roles               # Role management
permissions         # Permission management
api-keys            # API key management
reports             # Reporting
metrics             # Metrics and analytics
admin               # Admin operations
settings            # System settings
```

### Custom Permissions

You can assign **custom permissions** to users beyond their role:

```bash
POST /api/v1/users/{id}/permissions
{
  "permissions": [
    "orders:cancel",      # Add specific permission
    "reports:export"      # Add another permission
  ]
}
```

### Permission Checking

**In Code** (Rust):
```rust
// Check single permission
user.has_permission("orders:read")?;

// Check multiple permissions (any)
user.has_any_permission(&["orders:read", "orders:create"])?;

// Check multiple permissions (all)
user.has_all_permissions(&["inventory:read", "inventory:adjust"])?;

// Check resource access
user.can_access_resource("orders", "read")?;
```

**In Middleware**:
```rust
// Protect route with permission requirement
.route("/orders", get(list_orders))
    .layer(RequirePermission::new("orders:read"))

// Multiple permissions (any)
.route("/orders/:id", put(update_order))
    .layer(RequireAnyPermission::new(&["orders:update", "orders:manage"]))
```

### Multi-Tenant Support

StateSet API supports **multi-tenancy** with tenant-level isolation:

```json
{
  "sub": "user-uuid",
  "tenant_id": "tenant-uuid",  // Tenant identifier
  "roles": ["manager"],
  "permissions": ["orders:read"]
}
```

**Automatic tenant filtering**:
- Users can only access resources within their tenant
- Admin role can access all tenants (use with caution)
- API keys are tenant-specific

---

## OAuth2 Integration

StateSet API supports **OAuth2 2.0** for third-party authentication.

### Supported Providers

1. **Google** - Google Sign-In
2. **GitHub** - GitHub OAuth
3. **Microsoft** - Azure AD / Microsoft Account
4. **Custom** - Any OAuth2-compliant provider

### OAuth2 Flow

```
1. Initiate OAuth2 flow
   GET /api/v1/auth/oauth2/{provider}/authorize

   Redirects to:
   https://provider.com/oauth/authorize?
     client_id=...&
     redirect_uri=...&
     scope=openid profile email&
     state=random-state&
     code_challenge=...  # PKCE

2. User authorizes on provider's site

3. Provider redirects back
   GET /api/v1/auth/oauth2/{provider}/callback?
     code=auth-code&
     state=random-state

4. Exchange code for tokens
   (Happens automatically on server)

5. Server returns JWT tokens
   {
     "access_token": "eyJhbGc...",
     "refresh_token": "eyJhbGc...",
     "user": {
       "id": "uuid",
       "email": "user@example.com",
       "name": "John Doe",
       "provider": "google"
     }
   }
```

### Configuration

```toml
# config/production.toml

[oauth2.google]
enabled = true
client_id = "${GOOGLE_CLIENT_ID}"
client_secret = "${GOOGLE_CLIENT_SECRET}"
redirect_uri = "https://api.stateset.com/api/v1/auth/oauth2/google/callback"
scopes = ["openid", "profile", "email"]

[oauth2.github]
enabled = true
client_id = "${GITHUB_CLIENT_ID}"
client_secret = "${GITHUB_CLIENT_SECRET}"
redirect_uri = "https://api.stateset.com/api/v1/auth/oauth2/github/callback"
scopes = ["read:user", "user:email"]

[oauth2.microsoft]
enabled = true
client_id = "${MICROSOFT_CLIENT_ID}"
client_secret = "${MICROSOFT_CLIENT_SECRET}"
redirect_uri = "https://api.stateset.com/api/v1/auth/oauth2/microsoft/callback"
tenant_id = "${MICROSOFT_TENANT_ID}"  # Optional: specific tenant
scopes = ["openid", "profile", "email"]
```

### Security Features

✅ **PKCE (Proof Key for Code Exchange)** - Prevents authorization code interception
✅ **State Parameter Validation** - Prevents CSRF attacks
✅ **Nonce Validation** - Prevents replay attacks
✅ **Secure Token Storage** - Tokens stored with encryption
✅ **Account Linking** - Link multiple OAuth providers to one account

### Account Linking

Users can link multiple OAuth providers:

```bash
# Link new provider to existing account
POST /api/v1/auth/oauth2/link/{provider}
Authorization: Bearer <access_token>

# Unlink provider
DELETE /api/v1/auth/oauth2/unlink/{provider}
Authorization: Bearer <access_token>

# List linked providers
GET /api/v1/auth/oauth2/linked
Authorization: Bearer <access_token>
```

---

## Multi-Factor Authentication (MFA)

StateSet API supports **Multi-Factor Authentication** for enhanced security.

### MFA Methods

1. **TOTP (Time-based One-Time Password)** - Authenticator apps (Google Authenticator, Authy)
2. **SMS** - Text message codes
3. **Email** - Email verification codes
4. **Backup Codes** - One-time recovery codes

### TOTP Setup Flow

```
1. Enable MFA
   POST /api/v1/auth/mfa/totp/enable
   Authorization: Bearer <access_token>

   Response:
   {
     "secret": "JBSWY3DPEHPK3PXP",
     "qr_code": "data:image/png;base64,...",
     "backup_codes": [
       "12345678",
       "87654321",
       ...
     ]
   }

2. User scans QR code with authenticator app

3. Verify setup
   POST /api/v1/auth/mfa/totp/verify
   Authorization: Bearer <access_token>

   {
     "code": "123456"
   }

4. MFA is now enabled
```

### Login with MFA

```
1. Initial login
   POST /api/v1/auth/login
   {
     "email": "user@example.com",
     "password": "secure-password"
   }

   Response (MFA required):
   {
     "mfa_required": true,
     "mfa_token": "temporary-token",
     "methods": ["totp", "sms"]
   }

2. Submit MFA code
   POST /api/v1/auth/mfa/verify
   {
     "mfa_token": "temporary-token",
     "code": "123456",
     "method": "totp"
   }

   Response (success):
   {
     "access_token": "eyJhbGc...",
     "refresh_token": "eyJhbGc..."
   }
```

### Backup Codes

**Generate new backup codes**:
```bash
POST /api/v1/auth/mfa/backup-codes/regenerate
Authorization: Bearer <access_token>

Response:
{
  "backup_codes": [
    "12345678",
    "87654321",
    "24681357",
    "13579246",
    "98765432"
  ]
}
```

**Use backup code**:
```bash
POST /api/v1/auth/mfa/verify
{
  "mfa_token": "temporary-token",
  "code": "12345678",
  "method": "backup_code"
}
```

### MFA Configuration

```toml
[mfa]
enabled = true
totp_issuer = "StateSet API"
totp_period = 30          # seconds
totp_window = 1           # ±1 time step tolerance
backup_code_count = 10    # Number of backup codes
sms_enabled = true        # Requires SMS provider
email_enabled = true
```

### Disable MFA

```bash
POST /api/v1/auth/mfa/disable
Authorization: Bearer <access_token>

{
  "password": "current-password",  # Confirmation required
  "code": "123456"                 # Current MFA code
}
```

---

## API Key Management

API Keys provide **programmatic access** for integrations and automation.

### Key Features

- ✅ **Granular Permissions** - Assign specific permissions per key
- ✅ **Expiration Dates** - Optional automatic expiration
- ✅ **Usage Tracking** - Last used timestamp
- ✅ **Individual Revocation** - Revoke keys without affecting others
- ✅ **Secure Storage** - Keys hashed before storage (SHA-256)
- ✅ **Rate Limiting** - Per-key rate limits

### Key Lifecycle

#### 1. Create API Key

```bash
POST /api/v1/auth/api-keys
Authorization: Bearer <access_token>
Content-Type: application/json

{
  "name": "Production Integration",
  "description": "Main production system integration",
  "permissions": [
    "orders:read",
    "orders:create",
    "inventory:read"
  ],
  "expires_at": "2026-12-31T23:59:59Z",  # Optional
  "rate_limit": {                         # Optional custom limit
    "requests_per_window": 500,
    "window_seconds": 60
  }
}

Response:
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "key": "sk_live_EXAMPLE_KEY_NOT_REAL_DO_NOT_USE",  # Only shown once!
  "name": "Production Integration",
  "permissions": ["orders:read", "orders:create", "inventory:read"],
  "expires_at": "2026-12-31T23:59:59Z",
  "created_at": "2025-12-10T00:00:00Z"
}
```

**⚠️ Important**: The API key value is only shown **once**. Store it securely!

#### 2. List API Keys

```bash
GET /api/v1/auth/api-keys
Authorization: Bearer <access_token>

Response:
{
  "api_keys": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "name": "Production Integration",
      "permissions": ["orders:read", "orders:create"],
      "last_used": "2025-12-10T12:00:00Z",
      "expires_at": "2026-12-31T23:59:59Z",
      "created_at": "2025-12-10T00:00:00Z",
      "status": "active"
    }
  ],
  "total": 1
}
```

#### 3. Get API Key Details

```bash
GET /api/v1/auth/api-keys/{id}
Authorization: Bearer <access_token>

Response:
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "name": "Production Integration",
  "description": "Main production system integration",
  "permissions": ["orders:read", "orders:create", "inventory:read"],
  "last_used": "2025-12-10T12:00:00Z",
  "usage_count": 15234,
  "expires_at": "2026-12-31T23:59:59Z",
  "created_at": "2025-12-10T00:00:00Z",
  "rate_limit": {
    "requests_per_window": 500,
    "window_seconds": 60
  },
  "status": "active"
}
```

#### 4. Update API Key

```bash
PATCH /api/v1/auth/api-keys/{id}
Authorization: Bearer <access_token>

{
  "name": "Updated Integration Name",
  "permissions": ["orders:*"],  # Update permissions
  "expires_at": "2027-12-31T23:59:59Z"
}
```

#### 5. Revoke API Key

```bash
DELETE /api/v1/auth/api-keys/{id}
Authorization: Bearer <access_token>

Response:
{
  "message": "API key revoked successfully",
  "revoked_at": "2025-12-10T15:30:00Z"
}
```

### Using API Keys

**Recommended: X-API-Key Header**
```bash
curl -X GET \
  https://api.stateset.com/api/v1/orders \
  -H "X-API-Key: sk_live_EXAMPLE_KEY_NOT_REAL_DO_NOT_USE"
```

**Alternative: Authorization Header**
```bash
curl -X GET \
  https://api.stateset.com/api/v1/orders \
  -H "Authorization: ApiKey sk_live_EXAMPLE_KEY_NOT_REAL_DO_NOT_USE"
```

### API Key Best Practices

✅ **DO**:
- Use separate keys for different environments (dev, staging, prod)
- Set expiration dates for keys
- Use descriptive names
- Assign minimal required permissions
- Rotate keys periodically (every 90-180 days)
- Store keys securely (secrets manager, environment variables)
- Revoke keys immediately if compromised
- Monitor key usage via `last_used` timestamps

❌ **DON'T**:
- Commit keys to version control
- Share keys between applications
- Use admin keys for automated systems
- Store keys in frontend code
- Log API keys in application logs
- Reuse keys across environments

### Key Prefix Configuration

Customize API key prefixes:

```toml
[api_keys]
prefix = "sk"          # Default prefix
live_env = "live"      # Production environment
test_env = "test"      # Test environment
key_length = 32        # Key length (bytes)
```

Results in keys like:
- `sk_example_NOT_A_REAL_KEY` (production)
- `sk_test_...` (test/development)

---

## Password Policies

StateSet API enforces **strong password policies** to prevent weak passwords.

### Default Policy

```rust
PasswordPolicy {
    min_length: 12,
    max_length: 128,
    require_uppercase: true,
    require_lowercase: true,
    require_numbers: true,
    require_special_chars: true,
    prevent_dictionary_words: true,
    prevent_common_passwords: true,
    prevent_sequential: true,
    prevent_repeated: true,
    max_repeated_chars: 2,
    min_unique_chars: 8
}
```

### Policy Rules

#### Length Requirements
- **Minimum**: 12 characters (default)
- **Maximum**: 128 characters
- **Configurable**: Adjust via config file

#### Complexity Requirements
- ✅ At least **1 uppercase letter** (A-Z)
- ✅ At least **1 lowercase letter** (a-z)
- ✅ At least **1 number** (0-9)
- ✅ At least **1 special character** (!@#$%^&*()_+-=[]{}|;:,.<>?)

#### Security Checks
- ❌ **No dictionary words** - Common words rejected
- ❌ **No common passwords** - Checks against list of 10,000+ common passwords
- ❌ **No sequential characters** - "abc", "123", "qwerty" rejected
- ❌ **No repeated characters** - "aaa", "111" rejected (max 2 repetitions)
- ❌ **Sufficient uniqueness** - Minimum 8 unique characters

#### Similarity Checks
- ❌ **Not similar to username** - Levenshtein distance check
- ❌ **Not similar to email** - Prevents using email as password
- ❌ **Password history** - Cannot reuse last 5 passwords

### Password Validation Response

```json
{
  "valid": false,
  "errors": [
    "Password too short: minimum 12 characters required",
    "Password must contain at least one uppercase letter",
    "Password must contain at least one special character",
    "Password is in the list of commonly used passwords"
  ]
}
```

### Configuration

```toml
[password_policy]
min_length = 12
max_length = 128
require_uppercase = true
require_lowercase = true
require_numbers = true
require_special_chars = true
prevent_dictionary_words = true
prevent_common_passwords = true
prevent_sequential = true
prevent_repeated = true
max_repeated_chars = 2
min_unique_chars = 8
password_history_count = 5    # Number of previous passwords to check
```

### Password Change Flow

```bash
POST /api/v1/auth/password/change
Authorization: Bearer <access_token>

{
  "current_password": "old-password",
  "new_password": "new-secure-password-123!",
  "confirm_password": "new-secure-password-123!"
}

Response (Success):
{
  "message": "Password changed successfully"
}

Response (Policy Violation):
{
  "error": "Password policy violation",
  "errors": [
    "Password too short: minimum 12 characters required"
  ]
}
```

### Password Reset Flow

```bash
# 1. Request reset
POST /api/v1/auth/password/reset/request
{
  "email": "user@example.com"
}

Response:
{
  "message": "If the email exists, a reset link has been sent"
}

# 2. User receives email with reset token

# 3. Reset password
POST /api/v1/auth/password/reset/confirm
{
  "token": "reset-token-from-email",
  "new_password": "new-secure-password-123!",
  "confirm_password": "new-secure-password-123!"
}
```

### Password Hashing

**Algorithm**: Argon2id (winner of Password Hashing Competition)

**Parameters**:
```rust
Argon2::default()
    .memory_cost(15360)      // 15 MB
    .time_cost(2)            // 2 iterations
    .parallelism(1)          // Single thread
```

**Security Features**:
- ✅ Memory-hard algorithm (resistant to GPU attacks)
- ✅ Salted (unique salt per password)
- ✅ Timing-attack resistant
- ✅ Future-proof (easily adjustable parameters)

---

## Rate Limiting

StateSet API implements **comprehensive rate limiting** to prevent abuse and ensure fair usage.

### Rate Limiting Strategy

```
┌─────────────────────────────────────────┐
│      Global Rate Limit (optional)       │
│      Default: 1000 req/min per IP       │
└────────────────┬────────────────────────┘
                 │
     ┌───────────┴───────────┐
     │                       │
     ▼                       ▼
┌─────────────┐     ┌──────────────────┐
│  Per-User   │     │   Per-API-Key    │
│ Rate Limit  │     │   Rate Limit     │
└──────┬──────┘     └────────┬─────────┘
       │                     │
       └──────────┬──────────┘
                  │
                  ▼
      ┌───────────────────────┐
      │   Per-Endpoint Policy │
      │   (most restrictive)  │
      └───────────────────────┘
```

### Configuration

```toml
[rate_limit]
# Global limits
enabled = true
requests_per_window = 1000
window_seconds = 60
use_redis = true
enable_headers = true

# Per-path policies (most restrictive wins)
path_policies = [
  "/api/v1/orders:60:60",           # 60 req/min for orders
  "/api/v1/inventory:120:60",       # 120 req/min for inventory
  "/api/v1/auth/login:5:60"         # 5 req/min for login (brute force protection)
]

# Per-API-key custom limits
api_key_policies = [
  "sk_example_abc:200:60",             # Custom limit for specific key
  "sk_example_xyz:500:60"
]

# Per-user custom limits
user_policies = [
  "user-123:500:60",                # Premium user with higher limit
  "user-456:100:60"                 # Free tier user
]
```

### Rate Limit Headers

**Standard Headers** (RFC 6585):
```
RateLimit-Limit: 1000           # Maximum requests allowed
RateLimit-Remaining: 742        # Requests remaining
RateLimit-Reset: 1702000000     # Unix timestamp when limit resets
```

**X-Headers** (deprecated but included for compatibility):
```
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 742
X-RateLimit-Reset: 1702000000
```

### Response When Limited

**Status Code**: `429 Too Many Requests`

```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded. Try again in 30 seconds.",
    "status": 429,
    "details": {
      "limit": 1000,
      "window": 60,
      "reset_at": "2025-12-10T12:01:00Z",
      "retry_after": 30
    }
  }
}
```

**Headers**:
```
HTTP/1.1 429 Too Many Requests
RateLimit-Limit: 1000
RateLimit-Remaining: 0
RateLimit-Reset: 1702000000
Retry-After: 30
```

### Rate Limiting Tiers

**Free Tier**:
- 100 requests per minute
- No burst capacity
- Standard endpoints only

**Standard Tier**:
- 1,000 requests per minute
- 2x burst capacity (2,000)
- All endpoints

**Premium Tier**:
- 5,000 requests per minute
- 3x burst capacity (15,000)
- All endpoints
- Custom limits available

**Enterprise Tier**:
- Custom limits negotiated
- Dedicated rate limit buckets
- Priority processing
- SLA guarantees

### Monitoring Rate Limits

```bash
# Check current rate limit status
GET /api/v1/auth/rate-limit/status
Authorization: Bearer <access_token>

Response:
{
  "limits": {
    "global": {
      "limit": 1000,
      "remaining": 742,
      "reset_at": "2025-12-10T12:01:00Z"
    },
    "user": {
      "limit": 1000,
      "remaining": 742,
      "reset_at": "2025-12-10T12:01:00Z"
    },
    "api_key": {
      "limit": 500,
      "remaining": 234,
      "reset_at": "2025-12-10T12:01:00Z"
    }
  },
  "active_policies": [
    {
      "path": "/api/v1/orders",
      "limit": 60,
      "remaining": 45,
      "reset_at": "2025-12-10T12:01:00Z"
    }
  ]
}
```

### Best Practices

✅ **DO**:
- Implement exponential backoff when hitting limits
- Monitor `RateLimit-Remaining` header
- Cache responses when possible
- Use webhooks instead of polling
- Batch requests when available
- Request higher limits if needed (contact support)

❌ **DON'T**:
- Ignore rate limit headers
- Retry immediately after 429
- Create multiple API keys to bypass limits (violation of ToS)
- Make unnecessary requests

### Example: Handling Rate Limits (JavaScript)

```javascript
async function makeRequest(url, options) {
  const response = await fetch(url, options);

  // Check rate limit headers
  const remaining = parseInt(response.headers.get('RateLimit-Remaining'));
  const limit = parseInt(response.headers.get('RateLimit-Limit'));

  // Warn if approaching limit
  if (remaining < limit * 0.1) {
    console.warn(`Rate limit warning: ${remaining}/${limit} remaining`);
  }

  // Handle rate limit exceeded
  if (response.status === 429) {
    const retryAfter = parseInt(response.headers.get('Retry-After'));
    console.log(`Rate limited. Retrying after ${retryAfter} seconds`);

    await new Promise(resolve => setTimeout(resolve, retryAfter * 1000));
    return makeRequest(url, options); // Retry
  }

  return response;
}
```

---

## Security Features

### 1. Input Validation

**Comprehensive validation** on all endpoints using the `validator` crate.

**Validation Types**:
- ✅ **Email format** - RFC 5322 compliant
- ✅ **URL format** - Valid URL structure
- ✅ **Length constraints** - Min/max length checks
- ✅ **Range validation** - Numeric range checks
- ✅ **Pattern matching** - Regex validation
- ✅ **Custom validators** - Business logic validation

**Example**:
```rust
#[derive(Deserialize, Validate)]
pub struct CreateOrderRequest {
    #[validate(length(min = 1, max = 100))]
    pub customer_name: String,

    #[validate(email)]
    pub email: String,

    #[validate(range(min = 0.01, max = 1000000.00))]
    pub total: Decimal,

    #[validate(length(min = 1))]
    pub items: Vec<OrderItem>,
}
```

### 2. SQL Injection Protection

**SeaORM** provides automatic protection via parameterized queries.

**Safe**:
```rust
// SeaORM automatically parameterizes
Order::find()
    .filter(order::Column::CustomerId.eq(customer_id))
    .all(&db)
    .await?
```

**NOT USED** (raw SQL is avoided):
```rust
// This pattern is NOT used in StateSet API
db.execute(&format!("SELECT * FROM orders WHERE customer_id = {}", customer_id))
```

### 3. CSRF Protection

**Token-based CSRF protection** for web applications:

```rust
// CSRF token included in forms
<form method="POST">
  <input type="hidden" name="_csrf" value="{{ csrf_token }}" />
  ...
</form>

// Validated on server
if !verify_csrf_token(request) {
    return Error::CsrfValidationFailed;
}
```

### 4. XSS Prevention

**Output encoding** and **Content Security Policy**:

```rust
// Automatic HTML encoding
pub fn encode_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

// Content Security Policy header
Content-Security-Policy: default-src 'self'; script-src 'self'; ...
```

### 5. HMAC Webhook Verification

**Constant-time comparison** prevents timing attacks:

```rust
pub fn verify_webhook_signature(
    payload: &[u8],
    signature: &str,
    secret: &str,
) -> Result<(), AuthError> {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| AuthError::HmacError)?;

    mac.update(payload);

    let expected = mac.finalize();
    let expected_hex = hex::encode(expected.into_bytes());

    // Constant-time comparison prevents timing attacks
    if constant_time_eq(signature.as_bytes(), expected_hex.as_bytes()) {
        Ok(())
    } else {
        Err(AuthError::InvalidSignature)
    }
}
```

### 6. Secure Headers

**Security headers** automatically added to all responses:

```
X-Content-Type-Options: nosniff
X-Frame-Options: DENY
X-XSS-Protection: 1; mode=block
Strict-Transport-Security: max-age=31536000; includeSubDomains; preload
Content-Security-Policy: default-src 'self'
Referrer-Policy: strict-origin-when-cross-origin
Permissions-Policy: geolocation=(), microphone=(), camera=()
```

### 7. Audit Logging

**Comprehensive security event logging**:

```rust
// Authentication events
log_security_event(SecurityEvent {
    event_type: "AUTH_SUCCESS",
    user_id: Some(user.id),
    ip_address: request.ip(),
    user_agent: request.user_agent(),
    timestamp: Utc::now(),
    details: json!({
        "method": "jwt",
        "roles": user.roles,
    }),
});

// Authorization failures
log_security_event(SecurityEvent {
    event_type: "AUTH_PERMISSION_DENIED",
    user_id: Some(user.id),
    resource: "orders",
    action: "delete",
    reason: "Insufficient permissions",
});

// API key usage
log_security_event(SecurityEvent {
    event_type: "API_KEY_USED",
    api_key_id: key.id,
    endpoint: request.uri(),
});
```

**Logged Events**:
- Login attempts (success/failure)
- Password changes
- MFA enrollment/disablement
- API key creation/revocation
- Permission denials
- Rate limit violations
- Suspicious activity patterns

### 8. Session Security

**Session cookies** (when used) are secure by default:

```
Set-Cookie: session=...;
  HttpOnly;                # Prevents JavaScript access
  Secure;                  # HTTPS only
  SameSite=Strict;        # CSRF protection
  Max-Age=86400;          # 24 hours
  Path=/api;              # Limited scope
```

### 9. CORS Configuration

**Configurable CORS** for browser-based applications:

```toml
[cors]
allowed_origins = [
  "https://app.stateset.com",
  "https://dashboard.stateset.com"
]
allowed_methods = ["GET", "POST", "PUT", "DELETE", "PATCH"]
allowed_headers = ["Content-Type", "Authorization", "X-API-Key"]
allow_credentials = true
max_age = 3600
```

### 10. Secrets Management

**Environment-based secrets**:

```bash
# NEVER commit secrets to git
export JWT_SECRET="$(openssl rand -base64 64)"
export DATABASE_URL="postgresql://user:$(openssl rand -base64 32)@host:5432/db"
export API_KEY_ENCRYPTION_KEY="$(openssl rand -base64 32)"
```

**Secrets validation** at startup:
```rust
// Reject weak secrets
if config.jwt_secret.len() < 64 {
    panic!("JWT_SECRET must be at least 64 characters");
}
```

---

## Configuration

### Security Configuration File

```toml
# config/production.toml

[security]
environment = "production"

# JWT Configuration
[jwt]
secret = "${JWT_SECRET}"                    # 64+ characters required!
access_expiration = 900                     # 15 minutes
refresh_expiration = 604800                 # 7 days
algorithm = "HS256"
issuer = "stateset-api"
audience = "stateset-client"

# OAuth2 Configuration
[oauth2]
[oauth2.google]
enabled = true
client_id = "${GOOGLE_CLIENT_ID}"
client_secret = "${GOOGLE_CLIENT_SECRET}"
redirect_uri = "https://api.stateset.com/api/v1/auth/oauth2/google/callback"

[oauth2.github]
enabled = true
client_id = "${GITHUB_CLIENT_ID}"
client_secret = "${GITHUB_CLIENT_SECRET}"
redirect_uri = "https://api.stateset.com/api/v1/auth/oauth2/github/callback"

# MFA Configuration
[mfa]
enabled = true
totp_issuer = "StateSet API"
totp_period = 30
totp_window = 1
backup_code_count = 10

# Password Policy
[password_policy]
min_length = 12
max_length = 128
require_uppercase = true
require_lowercase = true
require_numbers = true
require_special_chars = true
password_history_count = 5

# Rate Limiting
[rate_limit]
enabled = true
requests_per_window = 1000
window_seconds = 60
use_redis = true
enable_headers = true

# API Keys
[api_keys]
enabled = true
prefix = "sk"
key_length = 32
hash_algorithm = "sha256"

# CORS
[cors]
allowed_origins = ["https://app.stateset.com"]
allowed_methods = ["GET", "POST", "PUT", "DELETE", "PATCH"]
allow_credentials = true
max_age = 3600

# Security Headers
[security_headers]
enable_hsts = true
hsts_max_age = 31536000
enable_csp = true
csp_policy = "default-src 'self'"
```

### Environment Variables

```bash
# Required
export JWT_SECRET="your-very-long-secret-at-least-64-characters-long-generated-securely"
export DATABASE_URL="postgresql://user:password@localhost:5432/stateset"
export REDIS_URL="redis://localhost:6379"

# OAuth2 (if enabled)
export GOOGLE_CLIENT_ID="your-google-client-id"
export GOOGLE_CLIENT_SECRET="your-google-client-secret"
export GITHUB_CLIENT_ID="your-github-client-id"
export GITHUB_CLIENT_SECRET="your-github-client-secret"

# Encryption
export API_KEY_ENCRYPTION_KEY="32-byte-encryption-key"

# Optional
export APP__JWT_EXPIRATION="900"
export APP__RATE_LIMIT_REQUESTS_PER_WINDOW="1000"
```

---

## Best Practices

### For Developers

#### Authentication

✅ **DO**:
- Use JWT tokens for user authentication
- Use API keys for service-to-service communication
- Implement token refresh before expiration
- Store refresh tokens securely (HttpOnly cookies or secure storage)
- Revoke tokens on logout
- Implement token rotation

❌ **DON'T**:
- Store JWT tokens in localStorage (XSS risk)
- Send sensitive data in JWT payload
- Use long-lived access tokens
- Share tokens between applications
- Include tokens in URLs

#### Authorization

✅ **DO**:
- Check permissions for every operation
- Use principle of least privilege
- Implement role-based access control
- Validate resource ownership
- Log authorization failures

❌ **DON'T**:
- Trust client-side permission checks
- Bypass authorization for "admin" users
- Use overly permissive roles
- Grant * permissions unnecessarily

#### API Keys

✅ **DO**:
- Use environment variables for keys
- Rotate keys regularly (90-180 days)
- Set expiration dates
- Assign minimal permissions
- Monitor key usage
- Revoke immediately if compromised

❌ **DON'T**:
- Commit keys to version control
- Share keys between environments
- Log API keys
- Expose keys in frontend code
- Reuse keys across applications

#### Passwords

✅ **DO**:
- Enforce strong password policies
- Use Argon2 for hashing
- Implement password history
- Provide password strength meter
- Require MFA for sensitive accounts

❌ **DON'T**:
- Store plaintext passwords
- Email passwords to users
- Allow common passwords
- Skip password complexity checks
- Use weak hashing algorithms (MD5, SHA1)

### For DevOps

#### Deployment

✅ **DO**:
- Use secrets management (AWS Secrets Manager, HashiCorp Vault)
- Rotate secrets regularly
- Use TLS/SSL for all connections
- Enable rate limiting
- Configure CORS properly
- Implement DDoS protection
- Use Web Application Firewall (WAF)
- Monitor security logs

❌ **DON'T**:
- Use default credentials
- Expose internal endpoints
- Skip security headers
- Disable HTTPS in production
- Use self-signed certificates in production

#### Monitoring

✅ **DO**:
- Monitor authentication failures
- Track API key usage
- Alert on suspicious patterns
- Log security events
- Monitor rate limit hits
- Track permission denials
- Review audit logs regularly

❌ **DON'T**:
- Log sensitive data (passwords, tokens)
- Ignore security alerts
- Skip log analysis
- Disable audit logging

---

## API Reference

### Authentication Endpoints

#### Login

```
POST /api/v1/auth/login
Content-Type: application/json

{
  "email": "user@example.com",
  "password": "secure-password"
}

Response:
{
  "access_token": "eyJhbGc...",
  "refresh_token": "eyJhbGc...",
  "expires_in": 900,
  "token_type": "Bearer",
  "user": {
    "id": "uuid",
    "email": "user@example.com",
    "name": "John Doe",
    "roles": ["manager"]
  }
}
```

#### Refresh Token

```
POST /api/v1/auth/refresh
Content-Type: application/json

{
  "refresh_token": "eyJhbGc..."
}

Response:
{
  "access_token": "eyJhbGc...",
  "refresh_token": "eyJhbGc...",
  "expires_in": 900
}
```

#### Logout

```
POST /api/v1/auth/logout
Authorization: Bearer <access_token>

Response:
{
  "message": "Successfully logged out"
}
```

### OAuth2 Endpoints

#### Initiate OAuth2 Flow

```
GET /api/v1/auth/oauth2/{provider}/authorize
  ?redirect_uri=https://app.stateset.com/callback
  &state=random-state

Redirects to provider's authorization page
```

#### OAuth2 Callback

```
GET /api/v1/auth/oauth2/{provider}/callback
  ?code=authorization-code
  &state=random-state

Response:
{
  "access_token": "eyJhbGc...",
  "refresh_token": "eyJhbGc...",
  "user": { ... }
}
```

### MFA Endpoints

#### Enable TOTP

```
POST /api/v1/auth/mfa/totp/enable
Authorization: Bearer <access_token>

Response:
{
  "secret": "JBSWY3DPEHPK3PXP",
  "qr_code": "data:image/png;base64,...",
  "backup_codes": ["12345678", ...]
}
```

#### Verify MFA Setup

```
POST /api/v1/auth/mfa/totp/verify
Authorization: Bearer <access_token>

{
  "code": "123456"
}
```

#### Verify MFA Login

```
POST /api/v1/auth/mfa/verify

{
  "mfa_token": "temporary-token",
  "code": "123456",
  "method": "totp"
}
```

### API Key Endpoints

#### Create API Key

```
POST /api/v1/auth/api-keys
Authorization: Bearer <access_token>

{
  "name": "Production Integration",
  "permissions": ["orders:read", "orders:create"],
  "expires_at": "2026-12-31T23:59:59Z"
}
```

#### List API Keys

```
GET /api/v1/auth/api-keys
Authorization: Bearer <access_token>
```

#### Revoke API Key

```
DELETE /api/v1/auth/api-keys/{id}
Authorization: Bearer <access_token>
```

### User Management Endpoints

#### Register User

```
POST /api/v1/auth/register

{
  "email": "user@example.com",
  "password": "secure-password-123!",
  "name": "John Doe"
}
```

#### Change Password

```
POST /api/v1/auth/password/change
Authorization: Bearer <access_token>

{
  "current_password": "old-password",
  "new_password": "new-secure-password-123!",
  "confirm_password": "new-secure-password-123!"
}
```

#### Request Password Reset

```
POST /api/v1/auth/password/reset/request

{
  "email": "user@example.com"
}
```

#### Confirm Password Reset

```
POST /api/v1/auth/password/reset/confirm

{
  "token": "reset-token",
  "new_password": "new-secure-password-123!",
  "confirm_password": "new-secure-password-123!"
}
```

---

## Security Checklist

### Pre-Deployment

- [ ] Change all default credentials
- [ ] Generate strong JWT secret (64+ characters)
- [ ] Configure OAuth2 providers (if used)
- [ ] Set up rate limiting
- [ ] Configure CORS for production domains
- [ ] Enable HTTPS/TLS
- [ ] Set up secrets management
- [ ] Configure security headers
- [ ] Review password policy settings
- [ ] Set up audit logging
- [ ] Configure WAF rules
- [ ] Enable DDoS protection

### Post-Deployment

- [ ] Verify HTTPS is enforced
- [ ] Test authentication flows
- [ ] Test authorization rules
- [ ] Verify rate limiting works
- [ ] Test CORS configuration
- [ ] Review security logs
- [ ] Set up monitoring alerts
- [ ] Conduct security scan
- [ ] Review API key usage
- [ ] Test password reset flow
- [ ] Verify MFA works (if enabled)
- [ ] Test OAuth2 flows (if enabled)

### Ongoing

- [ ] Rotate JWT secrets (quarterly)
- [ ] Rotate API keys (90-180 days)
- [ ] Review audit logs (weekly)
- [ ] Monitor failed auth attempts
- [ ] Review permission grants
- [ ] Update password policies as needed
- [ ] Patch security vulnerabilities
- [ ] Conduct security audits (annually)
- [ ] Review and revoke unused API keys
- [ ] Monitor for suspicious patterns
- [ ] Update dependencies regularly
- [ ] Train team on security practices

---

## Support & Resources

### Documentation
- [Production Deployment Guide](production_api_deployment.md) - Complete deployment documentation
- [API Overview](API_OVERVIEW.md) - API reference and architecture
- [Troubleshooting Guide](TROUBLESHOOTING.md) - Common issues and solutions
- [Best Practices](BEST_PRACTICES.md) - Production patterns and anti-patterns

### Security Reporting
- **Email**: security@stateset.com
- **PGP Key**: Available on request
- **Response Time**: Within 48 hours
- See [SECURITY.md](../SECURITY.md) for full policy

### Support Channels
- **Documentation**: https://docs.stateset.com
- **GitHub Issues**: https://github.com/stateset/stateset-api/issues
- **Email**: support@stateset.io

---

**Document Version**: 1.0
**Last Updated**: December 10, 2025
**Status**: Production Ready (10/10)

**Security is a continuous process. Stay vigilant! 🔒**
