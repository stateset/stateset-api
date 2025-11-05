# StateSet API - Troubleshooting Guide

Common issues and their solutions. Use Ctrl+F to find your specific error.

## Table of Contents

- [Installation & Setup Issues](#installation--setup-issues)
- [Database Issues](#database-issues)
- [Authentication Issues](#authentication-issues)
- [API Request Issues](#api-request-issues)
- [Performance Issues](#performance-issues)
- [Integration Issues](#integration-issues)
- [Deployment Issues](#deployment-issues)
- [Error Code Reference](#error-code-reference)

---

## Installation & Setup Issues

### Build Fails with "linker not found"

**Symptom:**
```
error: linker `cc` not found
```

**Solution:**
```bash
# macOS
xcode-select --install

# Ubuntu/Debian
sudo apt-get install build-essential

# Fedora/CentOS
sudo dnf install gcc
```

### "protoc not found" Error

**Symptom:**
```
error: failed to run custom build command for `stateset-api`
  Could not find `protoc` installation
```

**Solution:**
```bash
# macOS
brew install protobuf

# Ubuntu/Debian
sudo apt-get install protobuf-compiler

# Or download from: https://github.com/protocolbuffers/protobuf/releases
```

### Port 8080 Already in Use

**Symptom:**
```
Error: Address already in use (os error 98)
```

**Solution 1: Change Port**
```bash
export APP__PORT=8081
cargo run
```

**Solution 2: Kill Existing Process**
```bash
# Find process using port 8080
lsof -i :8080

# Kill the process
kill -9 <PID>
```

### Cargo Build Takes Too Long

**Symptom:**
First build takes 10+ minutes

**Solution:**
This is normal! Rust compiles all dependencies from source.

**Speed up future builds:**
```bash
# Use cargo-chef for Docker builds
# Use sccache for caching
cargo install sccache
export RUSTC_WRAPPER=sccache
```

**Use release mode only when needed:**
```bash
# Debug mode (faster compilation)
cargo build

# Release mode (optimized, slower compilation)
cargo build --release
```

---

## Database Issues

### Migration Failed - Table Already Exists

**Symptom:**
```
Error: Database error: table 'orders' already exists
```

**Solution 1: Reset Database (Development)**
```bash
# SQLite
rm stateset.db
cargo run --bin migration

# PostgreSQL
dropdb stateset
createdb stateset
cargo run --bin migration
```

**Solution 2: Check Migration Status**
```bash
# View applied migrations
cargo run --bin migration -- status

# Rollback last migration
cargo run --bin migration -- down
```

### Cannot Connect to PostgreSQL

**Symptom:**
```
Error: Connection refused (os error 111)
```

**Solution:**
```bash
# Check PostgreSQL is running
systemctl status postgresql

# Start PostgreSQL
sudo systemctl start postgresql

# Check connection string
export APP__DATABASE_URL="postgres://user:password@localhost:5432/stateset"

# Test connection
psql -h localhost -U user -d stateset

# Check pg_hba.conf for access rules
sudo nano /etc/postgresql/14/main/pg_hba.conf
```

### Database Connection Pool Exhausted

**Symptom:**
```
Error: Timeout acquiring connection from pool
```

**Solution:**
```toml
# config/default.toml
[database]
max_connections = 50  # Increase pool size
min_connections = 5
connect_timeout = 30
idle_timeout = 600
```

**Or check for connection leaks:**
```rust
// Ensure connections are properly dropped
// Check for long-running transactions
// Monitor active connections:
SELECT count(*) FROM pg_stat_activity WHERE datname = 'stateset';
```

### SQLite "Database Locked" Error

**Symptom:**
```
Error: database is locked
```

**Solution:**
```bash
# Increase timeout
export APP__DATABASE__CONNECT_TIMEOUT=30000

# Or switch to PostgreSQL for production
export APP__DATABASE_URL="postgres://..."
```

**Note:** SQLite is not recommended for production with concurrent writes.

### Foreign Key Constraint Failed

**Symptom:**
```
Error: FOREIGN KEY constraint failed
```

**Solution:**
```bash
# The referenced entity doesn't exist
# Check that customer_id, product_id, etc. are valid
# Ensure you create parent records before children

# Example: Create customer before order
curl -X POST /api/v1/customers -d '{...}'
# Get customer_id from response
curl -X POST /api/v1/orders -d '{"customer_id": "...", ...}'
```

---

## Authentication Issues

### "Invalid Credentials" on Login

**Symptom:**
```json
{
  "error": {
    "code": "INVALID_CREDENTIALS",
    "message": "Invalid email or password"
  }
}
```

**Solution:**
1. Verify email and password are correct
2. Check user exists: `SELECT * FROM users WHERE email = 'user@example.com'`
3. Ensure password was hashed correctly on registration
4. Check caps lock / leading/trailing spaces

### JWT Token Expired

**Symptom:**
```json
{
  "error": {
    "code": "TOKEN_EXPIRED",
    "message": "Access token has expired",
    "status": 401
  }
}
```

**Solution:**
Use refresh token to get new access token:
```bash
curl -X POST http://localhost:8080/api/v1/auth/refresh \
  -H "Content-Type: application/json" \
  -d '{
    "refresh_token": "your-refresh-token"
  }'
```

**Prevent in client code:**
```javascript
async function getValidToken() {
  if (tokenExpiresSoon()) {
    await refreshToken();
  }
  return accessToken;
}
```

### "Invalid Token" Error

**Symptom:**
```json
{
  "error": {
    "code": "INVALID_TOKEN",
    "message": "Token is invalid or malformed"
  }
}
```

**Checklist:**
- [ ] Token format: `Bearer <token>` (note the space)
- [ ] Token not truncated or modified
- [ ] Using access token, not refresh token
- [ ] Token from the correct environment (dev/staging/prod)
- [ ] Server and client using same JWT secret

**Debug:**
```bash
# Decode JWT to inspect
echo "your-token" | base64 -d

# Or use jwt.io
```

### API Key Not Working

**Symptom:**
```json
{
  "error": {
    "code": "INVALID_API_KEY",
    "message": "API key is invalid or revoked"
  }
}
```

**Checklist:**
- [ ] Using `X-API-Key` header (not `Authorization`)
- [ ] Key not expired: `SELECT * FROM api_keys WHERE key = 'your-key'`
- [ ] Key has required permissions: `SELECT permissions FROM api_keys WHERE key = 'your-key'`
- [ ] Key not revoked: `SELECT revoked_at FROM api_keys WHERE key = 'your-key'`

**Create new key:**
```bash
./target/debug/stateset-cli auth api-keys create \
  --name "New Key" \
  --permissions "orders:read,orders:create"
```

### "Insufficient Permissions" Error

**Symptom:**
```json
{
  "error": {
    "code": "INSUFFICIENT_PERMISSIONS",
    "message": "You don't have permission to perform this action",
    "required_permission": "orders:delete"
  }
}
```

**Solution:**
```bash
# Check user permissions
SELECT permissions FROM users WHERE id = 'user-id';

# Check API key permissions
SELECT permissions FROM api_keys WHERE key = 'api-key';

# Grant permissions (requires admin)
# Update user role or API key permissions
```

---

## API Request Issues

### 404 Not Found

**Symptom:**
```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "Resource not found"
  }
}
```

**Common causes:**
1. **Wrong URL**: Check for typos in endpoint path
   - ✅ `/api/v1/orders`
   - ❌ `/api/v1/order` (missing 's')
   - ❌ `/orders` (missing `/api/v1`)

2. **Resource doesn't exist**: Verify ID is correct
   ```bash
   # List all orders to get valid ID
   curl http://localhost:8080/api/v1/orders
   ```

3. **Wrong HTTP method**:
   - ❌ `GET /api/v1/orders` (to create order)
   - ✅ `POST /api/v1/orders`

### 400 Bad Request - Validation Error

**Symptom:**
```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid request data",
    "details": {
      "email": ["Email is required"],
      "password": ["Password must be at least 8 characters"]
    }
  }
}
```

**Solution:**
1. Check required fields are present
2. Verify data types (string vs number)
3. Check format (email, UUID, date)
4. Review constraints (min/max length, range)

**Example fix:**
```bash
# ❌ Bad
curl -X POST /api/v1/auth/register -d '{
  "email": "not-an-email",
  "password": "123"
}'

# ✅ Good
curl -X POST /api/v1/auth/register -d '{
  "email": "user@example.com",
  "password": "SecurePass123!"
}'
```

### 409 Conflict - Duplicate Resource

**Symptom:**
```json
{
  "error": {
    "code": "DUPLICATE_ENTRY",
    "message": "A resource with this identifier already exists",
    "details": {
      "field": "email",
      "value": "user@example.com"
    }
  }
}
```

**Common causes:**
1. **Duplicate email**: User already registered
2. **Duplicate SKU**: Product SKU already exists
3. **Duplicate order number**: Order number collision
4. **Idempotency key conflict**: Using same idempotency key

**Solution:**
```bash
# Check for existing resource
curl http://localhost:8080/api/v1/customers?email=user@example.com

# Update existing instead of creating new
curl -X PUT http://localhost:8080/api/v1/customers/{id} -d '{...}'
```

### 422 Unprocessable Entity - Business Logic Error

**Symptom:**
```json
{
  "error": {
    "code": "INSUFFICIENT_INVENTORY",
    "message": "Not enough inventory available",
    "details": {
      "product_id": "...",
      "requested": 10,
      "available": 5
    }
  }
}
```

**Common causes:**
- Insufficient inventory
- Order can't be canceled (already shipped)
- Return window expired
- Payment already processed

**Solution:**
Address the business constraint:
```bash
# Check inventory before ordering
curl http://localhost:8080/api/v1/inventory?product_id=...

# Reduce quantity or wait for restock
```

### 429 Too Many Requests - Rate Limited

**Symptom:**
```json
{
  "error": {
    "code": "RATE_LIMIT_EXCEEDED",
    "message": "Rate limit exceeded"
  }
}
```

**Response headers:**
```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 0
X-RateLimit-Reset: 1699564800
Retry-After: 60
```

**Solution:**
```javascript
// Implement retry with exponential backoff
async function requestWithRetry(url, options, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fetch(url, options);
    } catch (error) {
      if (error.response?.status === 429) {
        const retryAfter = error.response.headers['retry-after'] || Math.pow(2, i);
        await sleep(retryAfter * 1000);
        continue;
      }
      throw error;
    }
  }
}
```

### 500 Internal Server Error

**Symptom:**
```json
{
  "error": {
    "code": "INTERNAL_ERROR",
    "message": "An internal error occurred",
    "request_id": "req-abc123"
  }
}
```

**Debug steps:**
1. **Check server logs** for the request ID:
   ```bash
   grep "req-abc123" logs/stateset.log
   ```

2. **Common causes:**
   - Database connection lost
   - Out of memory
   - Panic in code
   - External service timeout

3. **Report the issue** with:
   - Request ID
   - Timestamp
   - Request payload (sanitized)
   - Server logs

### Request Timeout

**Symptom:**
```
Error: Request timeout after 30000ms
```

**Solution:**
```bash
# Increase client timeout
curl --max-time 60 http://localhost:8080/api/v1/...

# Or in JavaScript
const response = await axios.get(url, {
  timeout: 60000 // 60 seconds
});
```

**Server-side optimization:**
- Add database indexes
- Optimize slow queries
- Use pagination for large results
- Cache frequently accessed data

### CORS Error (Browser)

**Symptom:**
```
Access to fetch at 'http://localhost:8080/api/v1/orders' from origin
'http://localhost:3000' has been blocked by CORS policy
```

**Solution:**
Configure CORS in `config/default.toml`:
```toml
[server]
cors_origins = ["http://localhost:3000", "https://yourdomain.com"]
cors_methods = ["GET", "POST", "PUT", "DELETE", "PATCH"]
cors_headers = ["Authorization", "Content-Type", "X-API-Key"]
```

**Or for development (allow all):**
```toml
[server]
cors_origins = ["*"]
```

---

## Performance Issues

### Slow Response Times

**Symptom:**
API requests taking >1000ms

**Diagnostics:**
```bash
# Check metrics
curl http://localhost:8080/metrics | grep http_request_duration

# Enable debug logging
export RUST_LOG=debug
cargo run
```

**Common causes & solutions:**

1. **Missing database indexes**
   ```sql
   -- Check slow queries
   SELECT * FROM pg_stat_statements ORDER BY total_time DESC LIMIT 10;

   -- Add indexes
   CREATE INDEX idx_orders_customer_id ON orders(customer_id);
   CREATE INDEX idx_orders_status ON orders(status);
   CREATE INDEX idx_orders_created_at ON orders(created_at);
   ```

2. **N+1 query problem**
   ```rust
   // ❌ Bad: N+1 queries
   for order in orders {
       let customer = get_customer(order.customer_id); // N queries
   }

   // ✅ Good: Eager loading
   let orders_with_customers = get_orders_with_customers(); // 1 query
   ```

3. **No pagination**
   ```bash
   # ❌ Bad: Loads all orders
   curl http://localhost:8080/api/v1/orders

   # ✅ Good: Paginated
   curl http://localhost:8080/api/v1/orders?page=1&limit=20
   ```

4. **Not using Redis cache**
   ```bash
   # Enable Redis
   export APP__REDIS_URL="redis://localhost:6379"
   ```

### High Memory Usage

**Symptom:**
Server using >1GB RAM

**Solutions:**
```toml
# config/default.toml
[database]
max_connections = 20  # Reduce pool size

[cache]
max_size_mb = 128  # Limit cache size
```

**Check for memory leaks:**
```bash
# Use valgrind
valgrind --leak-check=full ./target/debug/stateset-api

# Monitor memory
watch -n 1 'ps aux | grep stateset-api'
```

### Database Connection Pool Exhaustion

**Symptom:**
```
Error: Timeout acquiring connection from pool
```

**Solutions:**
1. **Increase pool size** (temporary):
   ```toml
   [database]
   max_connections = 50
   ```

2. **Find connection leaks**:
   ```sql
   -- PostgreSQL: Check active connections
   SELECT count(*), state FROM pg_stat_activity
   WHERE datname = 'stateset'
   GROUP BY state;
   ```

3. **Reduce query time**:
   - Add indexes
   - Optimize slow queries
   - Use connection pooling

---

## Integration Issues

### Webhook Not Received

**Symptom:**
Webhooks not arriving at your endpoint

**Checklist:**
1. [ ] Endpoint is publicly accessible (not localhost)
2. [ ] HTTPS enabled (HTTP may be blocked)
3. [ ] Firewall allows incoming requests
4. [ ] Webhook URL configured correctly
5. [ ] Endpoint returns 200 OK quickly (<10s)

**Test webhook locally:**
```bash
# Use ngrok to expose localhost
ngrok http 3000

# Use the ngrok URL as webhook endpoint
# https://abc123.ngrok.io/webhooks/stateset
```

**Debug webhook delivery:**
```bash
# Check webhook logs
curl http://localhost:8080/api/v1/admin/webhooks/logs

# Retry failed webhooks
curl -X POST http://localhost:8080/api/v1/admin/webhooks/{id}/retry
```

### Webhook Signature Verification Fails

**Symptom:**
```
Error: Invalid webhook signature
```

**Solution:**
```javascript
const crypto = require('crypto');

function verifyWebhookSignature(payload, signature, secret) {
  // Use raw body (before parsing JSON)
  const computedSignature = crypto
    .createHmac('sha256', secret)
    .update(payload) // Raw string, not parsed object
    .digest('hex');

  return crypto.timingSafeEqual(
    Buffer.from(signature),
    Buffer.from(computedSignature)
  );
}

// ❌ Wrong: Using parsed JSON
app.use(express.json());
app.post('/webhook', (req, res) => {
  verify(JSON.stringify(req.body), sig, secret); // Won't match
});

// ✅ Correct: Using raw body
app.use('/webhook', express.raw({ type: 'application/json' }));
app.post('/webhook', (req, res) => {
  verify(req.body.toString(), sig, secret); // Matches
});
```

### Idempotency Not Working

**Symptom:**
Duplicate requests creating multiple resources

**Solution:**
Ensure you're sending `Idempotency-Key` header:
```bash
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Idempotency-Key: unique-key-123" \
  -H "Content-Type: application/json" \
  -d '{...}'
```

**Key requirements:**
- Must be unique per operation
- Same key + same request = cached response
- Different keys = new operations
- Keys expire after 10 minutes

### OAuth Integration Issues

**Symptom:**
Third-party OAuth not working

**Common issues:**
1. **Redirect URI mismatch**: Must match exactly
2. **Invalid client ID/secret**: Check credentials
3. **Wrong scope**: Request required scopes
4. **Token exchange fails**: Check token endpoint

**Debug:**
```bash
# Check OAuth configuration
curl http://localhost:8080/api/v1/.well-known/oauth-authorization-server
```

---

## Deployment Issues

### Container Fails to Start

**Symptom:**
```
Error: Address already in use
```

**Solution:**
```yaml
# docker-compose.yml
services:
  api:
    ports:
      - "8080:8080"  # Change host port if needed
    environment:
      - APP__PORT=8080
      - APP__HOST=0.0.0.0  # Important for Docker
```

### Database Migrations Not Running in Docker

**Symptom:**
```
Error: relation "orders" does not exist
```

**Solution:**
```dockerfile
# Dockerfile - Run migrations before starting
CMD ["sh", "-c", "cargo run --bin migration && cargo run"]

# Or use separate init container
```

```yaml
# docker-compose.yml
services:
  migration:
    image: stateset-api
    command: cargo run --bin migration
    depends_on:
      - db

  api:
    image: stateset-api
    depends_on:
      migration:
        condition: service_completed_successfully
```

### Environment Variables Not Loading

**Symptom:**
Config not applying in Docker

**Solution:**
```yaml
# docker-compose.yml
services:
  api:
    environment:
      # Use APP__ prefix for config override
      - APP__DATABASE_URL=postgres://user:pass@db:5432/stateset
      - APP__REDIS_URL=redis://redis:6379
      - APP__PORT=8080
      - APP__HOST=0.0.0.0
      - RUST_LOG=info
```

**Debug:**
```bash
# Check environment variables in container
docker exec stateset-api env | grep APP__
```

### Health Check Failing in Kubernetes

**Symptom:**
```
Liveness probe failed: Get http://...:8080/health: connection refused
```

**Solution:**
```yaml
# k8s-deployment.yaml
apiVersion: v1
kind: Pod
spec:
  containers:
  - name: stateset-api
    livenessProbe:
      httpGet:
        path: /health
        port: 8080
        scheme: HTTP
      initialDelaySeconds: 30  # Give time to start
      periodSeconds: 10
      timeoutSeconds: 5
      failureThreshold: 3
    readinessProbe:
      httpGet:
        path: /health/readiness
        port: 8080
      initialDelaySeconds: 10
      periodSeconds: 5
```

### SSL/TLS Certificate Issues

**Symptom:**
```
Error: SSL certificate problem: unable to get local issuer certificate
```

**Solution:**
```bash
# Add trusted CA certificates
# Debian/Ubuntu
sudo apt-get install ca-certificates
sudo update-ca-certificates

# Alpine (Docker)
RUN apk add --no-cache ca-certificates
```

**For self-signed certs (dev only):**
```bash
# ⚠️ Not recommended for production
export APP__TLS_VERIFY=false
```

---

## Error Code Reference

### Authentication Errors

| Code | Status | Meaning | Solution |
|------|--------|---------|----------|
| `INVALID_CREDENTIALS` | 401 | Wrong email/password | Check credentials |
| `TOKEN_EXPIRED` | 401 | Access token expired | Use refresh token |
| `INVALID_TOKEN` | 401 | Malformed/invalid token | Get new token |
| `INSUFFICIENT_PERMISSIONS` | 403 | Missing required permission | Request access or use different user |
| `INVALID_API_KEY` | 401 | API key invalid/revoked | Create new API key |

### Resource Errors

| Code | Status | Meaning | Solution |
|------|--------|---------|----------|
| `NOT_FOUND` | 404 | Resource doesn't exist | Check ID is correct |
| `DUPLICATE_ENTRY` | 409 | Resource already exists | Use different identifier or update existing |
| `VALIDATION_ERROR` | 400 | Invalid request data | Fix validation errors |
| `RESOURCE_LOCKED` | 423 | Resource is locked | Wait and retry |

### Business Logic Errors

| Code | Status | Meaning | Solution |
|------|--------|---------|----------|
| `INSUFFICIENT_INVENTORY` | 422 | Not enough stock | Reduce quantity or wait for restock |
| `INVALID_STATUS_TRANSITION` | 422 | Can't change to that status | Check valid transitions |
| `ORDER_ALREADY_FULFILLED` | 422 | Order already processed | Can't modify fulfilled orders |
| `RETURN_WINDOW_EXPIRED` | 422 | Past return deadline | Contact support |
| `PAYMENT_FAILED` | 402 | Payment processing failed | Try different payment method |

### System Errors

| Code | Status | Meaning | Solution |
|------|--------|---------|----------|
| `INTERNAL_ERROR` | 500 | Server error | Check logs, report with request ID |
| `SERVICE_UNAVAILABLE` | 503 | Service temporarily down | Retry with exponential backoff |
| `RATE_LIMIT_EXCEEDED` | 429 | Too many requests | Wait and retry after rate limit reset |
| `TIMEOUT` | 504 | Request took too long | Optimize query or increase timeout |

---

## Getting More Help

### Collect Diagnostic Information

When reporting issues, include:

```bash
# 1. Version info
curl http://localhost:8080/health/version

# 2. Error details
{
  "request_id": "req-abc123",
  "timestamp": "2025-11-05T10:00:00Z",
  "error_code": "...",
  "error_message": "..."
}

# 3. Server logs (sanitized)
tail -n 100 logs/stateset.log | grep "req-abc123"

# 4. Configuration (remove secrets)
cat config/default.toml

# 5. Environment
rust --version
cargo --version
uname -a
```

### Support Channels

1. **Documentation**: [DOCUMENTATION_INDEX.md](./DOCUMENTATION_INDEX.md)
2. **Search Issues**: [GitHub Issues](https://github.com/stateset/stateset-api/issues)
3. **Community**: [GitHub Discussions](https://github.com/stateset/stateset-api/discussions)
4. **Email**: support@stateset.io

### Useful Debug Commands

```bash
# Enable verbose logging
export RUST_LOG=debug,sqlx=debug
cargo run

# Check database connectivity
psql $APP__DATABASE_URL -c "SELECT 1"

# Test Redis connectivity
redis-cli -u $APP__REDIS_URL ping

# Verify API is responding
curl -v http://localhost:8080/health

# Check port availability
netstat -an | grep 8080

# Monitor real-time logs
tail -f logs/stateset.log | jq

# Test webhook endpoint
curl -X POST https://your-webhook-url/test \
  -H "Content-Type: application/json" \
  -d '{"test": true}'
```

---

**Still stuck?** Open an issue with diagnostic information and we'll help!

[← Back to Documentation Index](./DOCUMENTATION_INDEX.md)
