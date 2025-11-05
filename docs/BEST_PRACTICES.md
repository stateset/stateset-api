# StateSet API - Best Practices Guide

Production-ready patterns and anti-patterns for building with StateSet API.

## Table of Contents

- [API Design Patterns](#api-design-patterns)
- [Authentication & Security](#authentication--security)
- [Error Handling](#error-handling)
- [Performance Optimization](#performance-optimization)
- [Data Management](#data-management)
- [Integration Patterns](#integration-patterns)
- [Testing Strategies](#testing-strategies)
- [Monitoring & Observability](#monitoring--observability)
- [Common Anti-Patterns](#common-anti-patterns)

---

## API Design Patterns

### Use Idempotency Keys for All Mutations

**✅ DO:**
```javascript
async function createOrder(orderData) {
  const idempotencyKey = generateUUID();

  try {
    return await api.post('/orders', orderData, {
      headers: { 'Idempotency-Key': idempotencyKey }
    });
  } catch (error) {
    if (isNetworkError(error)) {
      // Safe to retry with same key
      return await api.post('/orders', orderData, {
        headers: { 'Idempotency-Key': idempotencyKey }
      });
    }
    throw error;
  }
}
```

**❌ DON'T:**
```javascript
// No idempotency key - might create duplicate orders
await api.post('/orders', orderData);
```

**Why:** Network issues can cause duplicate requests. Idempotency keys ensure requests are processed only once.

### Always Use Pagination

**✅ DO:**
```javascript
async function getAllOrders() {
  let allOrders = [];
  let page = 1;
  let hasMore = true;

  while (hasMore) {
    const response = await api.get('/orders', {
      params: { page, limit: 100 }
    });

    allOrders = allOrders.concat(response.data.items);
    hasMore = page < response.data.total_pages;
    page++;
  }

  return allOrders;
}
```

**❌ DON'T:**
```javascript
// Loading thousands of records - will timeout or OOM
const orders = await api.get('/orders');
```

**Why:** Unpaginated requests can timeout, exhaust memory, and impact performance.

### Filter on the Server, Not the Client

**✅ DO:**
```javascript
// Filter at API level
const pendingOrders = await api.get('/orders', {
  params: {
    status: 'pending',
    customer_id: customerId,
    start_date: '2025-01-01'
  }
});
```

**❌ DON'T:**
```javascript
// Fetch everything, filter client-side
const allOrders = await api.get('/orders');
const pendingOrders = allOrders.filter(o =>
  o.status === 'pending' && o.customer_id === customerId
);
```

**Why:** Server-side filtering reduces bandwidth, improves performance, and leverages database indexes.

### Use Specific Error Handling

**✅ DO:**
```javascript
try {
  const order = await createOrder(orderData);
} catch (error) {
  switch (error.code) {
    case 'INSUFFICIENT_INVENTORY':
      await notifyOutOfStock(error.details.product_id);
      break;
    case 'PAYMENT_FAILED':
      await requestAlternativePayment();
      break;
    case 'INVALID_ADDRESS':
      await requestAddressCorrection();
      break;
    default:
      await logUnexpectedError(error);
  }
}
```

**❌ DON'T:**
```javascript
try {
  const order = await createOrder(orderData);
} catch (error) {
  console.error('Order creation failed');
  // No specific handling - poor UX
}
```

**Why:** Specific error handling provides better user experience and enables appropriate recovery actions.

### Store Request IDs for Debugging

**✅ DO:**
```javascript
try {
  const response = await api.post('/orders', orderData);
  const requestId = response.headers['x-request-id'];

  await logSuccess({
    requestId,
    orderId: response.data.id,
    timestamp: new Date()
  });
} catch (error) {
  const requestId = error.response?.headers['x-request-id'];

  await logError({
    requestId,
    error: error.message,
    timestamp: new Date()
  });
}
```

**❌ DON'T:**
```javascript
// No request ID tracking
await api.post('/orders', orderData);
```

**Why:** Request IDs enable correlation between client and server logs for debugging.

---

## Authentication & Security

### Never Log Sensitive Data

**✅ DO:**
```javascript
logger.info('Processing payment', {
  orderId: order.id,
  amount: order.total,
  // Card number masked
  cardLast4: payment.card_number.slice(-4)
});
```

**❌ DON'T:**
```javascript
logger.info('Processing payment', {
  orderId: order.id,
  cardNumber: payment.card_number,  // ⚠️ PCI violation
  cvv: payment.cvv,                 // ⚠️ Never log
  password: user.password           // ⚠️ Never log
});
```

**Sensitive data to never log:**
- Passwords
- API keys
- Access tokens
- Full credit card numbers
- CVV codes
- Social Security Numbers
- Personal health information

### Store API Keys Securely

**✅ DO:**
```javascript
// Environment variables
const apiKey = process.env.STATESET_API_KEY;

// Or secret management service
const apiKey = await secretsManager.getSecret('stateset-api-key');
```

**❌ DON'T:**
```javascript
// Hardcoded in source code
const apiKey = 'sk_live_abc123...';  // ⚠️ Will leak in git

// Or in config files committed to git
// config.json:
{
  "apiKey": "sk_live_abc123..."  // ⚠️ Will leak
}
```

**Best practices:**
- Use environment variables
- Use secret management services (AWS Secrets Manager, etc.)
- Add `.env` to `.gitignore`
- Rotate keys regularly
- Use different keys per environment

### Implement Token Refresh Logic

**✅ DO:**
```javascript
class AuthClient {
  async getValidToken() {
    // Check if token expires soon (within 5 minutes)
    if (this.tokenExpiresAt < Date.now() + 300000) {
      await this.refreshToken();
    }
    return this.accessToken;
  }

  async refreshToken() {
    const response = await api.post('/auth/refresh', {
      refresh_token: this.refreshToken
    });

    this.accessToken = response.data.access_token;
    this.tokenExpiresAt = Date.now() + response.data.expires_in * 1000;
  }

  async request(method, url, data) {
    const token = await this.getValidToken();

    try {
      return await api.request({
        method,
        url,
        data,
        headers: { Authorization: `Bearer ${token}` }
      });
    } catch (error) {
      if (error.response?.status === 401) {
        // Token invalid, refresh and retry
        await this.refreshToken();
        return await this.request(method, url, data);
      }
      throw error;
    }
  }
}
```

**❌ DON'T:**
```javascript
// No automatic refresh - frequent 401 errors
const response = await api.get('/orders', {
  headers: { Authorization: `Bearer ${expiredToken}` }
});
```

### Use Scoped API Keys

**✅ DO:**
```javascript
// Create API key with specific permissions
const readOnlyKey = await createAPIKey({
  name: 'Analytics Service',
  permissions: [
    'orders:read',
    'inventory:read',
    'analytics:read'
  ]
});

const fulfillmentKey = await createAPIKey({
  name: 'Fulfillment Service',
  permissions: [
    'orders:read',
    'shipments:read',
    'shipments:create',
    'inventory:reserve'
  ]
});
```

**❌ DON'T:**
```javascript
// Full access for all services
const apiKey = await createAPIKey({
  name: 'All Services',
  permissions: ['*']  // ⚠️ Too permissive
});
```

**Why:** Principle of least privilege limits damage from compromised keys.

---

## Error Handling

### Implement Exponential Backoff for Retries

**✅ DO:**
```javascript
async function requestWithRetry(url, options, maxRetries = 3) {
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      return await fetch(url, options);
    } catch (error) {
      const isRetryable =
        error.response?.status >= 500 ||  // Server error
        error.code === 'ECONNRESET' ||    // Network error
        error.response?.status === 429;    // Rate limited

      if (!isRetryable || attempt === maxRetries) {
        throw error;
      }

      // Exponential backoff: 1s, 2s, 4s, 8s...
      const delay = Math.min(1000 * Math.pow(2, attempt - 1), 30000);

      // Add jitter to prevent thundering herd
      const jitter = Math.random() * 1000;

      await sleep(delay + jitter);
    }
  }
}
```

**❌ DON'T:**
```javascript
// Fixed delay retries
async function requestWithRetry(url, options, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await fetch(url, options);
    } catch (error) {
      await sleep(1000);  // ⚠️ Fixed delay causes thundering herd
    }
  }
}
```

### Don't Retry Client Errors (4xx)

**✅ DO:**
```javascript
async function request(url, options) {
  try {
    return await fetch(url, options);
  } catch (error) {
    const status = error.response?.status;

    // Retry server errors and network issues
    if (status >= 500 || !status) {
      return await retryRequest(url, options);
    }

    // Don't retry client errors
    if (status >= 400 && status < 500) {
      throw error;  // Fix the request instead
    }
  }
}
```

**❌ DON'T:**
```javascript
// Retrying 400 Bad Request - will always fail
for (let i = 0; i < 3; i++) {
  try {
    return await fetch(url, invalidPayload);
  } catch (error) {
    // ⚠️ Retrying won't fix invalid payload
    continue;
  }
}
```

### Implement Circuit Breaker Pattern

**✅ DO:**
```javascript
class CircuitBreaker {
  constructor(threshold = 5, timeout = 60000) {
    this.failureCount = 0;
    this.threshold = threshold;
    this.timeout = timeout;
    this.state = 'CLOSED';
    this.nextAttempt = Date.now();
  }

  async execute(fn) {
    if (this.state === 'OPEN') {
      if (Date.now() < this.nextAttempt) {
        throw new Error('Circuit breaker is OPEN');
      }
      this.state = 'HALF_OPEN';
    }

    try {
      const result = await fn();
      this.onSuccess();
      return result;
    } catch (error) {
      this.onFailure();
      throw error;
    }
  }

  onSuccess() {
    this.failureCount = 0;
    this.state = 'CLOSED';
  }

  onFailure() {
    this.failureCount++;
    if (this.failureCount >= this.threshold) {
      this.state = 'OPEN';
      this.nextAttempt = Date.now() + this.timeout;
    }
  }
}

// Usage
const breaker = new CircuitBreaker();
await breaker.execute(() => api.get('/orders'));
```

**Why:** Prevents cascading failures by stopping requests to failing services.

---

## Performance Optimization

### Cache Frequently Accessed Data

**✅ DO:**
```javascript
class CachedAPIClient {
  constructor() {
    this.cache = new Map();
    this.ttl = 60000; // 1 minute
  }

  async getProduct(id) {
    const cached = this.cache.get(`product:${id}`);

    if (cached && cached.expiresAt > Date.now()) {
      return cached.data;
    }

    const product = await api.get(`/products/${id}`);

    this.cache.set(`product:${id}`, {
      data: product,
      expiresAt: Date.now() + this.ttl
    });

    return product;
  }

  invalidate(id) {
    this.cache.delete(`product:${id}`);
  }
}
```

**❌ DON'T:**
```javascript
// Fetching product for every request
app.get('/cart', async (req, res) => {
  for (const item of cart.items) {
    // ⚠️ N queries - very slow
    const product = await api.get(`/products/${item.product_id}`);
  }
});
```

### Batch Related Operations

**✅ DO:**
```javascript
// Batch inventory reservations
async function reserveInventoryBatch(orderItems) {
  const reservations = orderItems.map(item => ({
    inventory_id: item.inventory_id,
    quantity: item.quantity,
    order_id: order.id
  }));

  return await api.post('/inventory/reserve-batch', {
    reservations
  });
}
```

**❌ DON'T:**
```javascript
// Individual requests - N round trips
for (const item of orderItems) {
  await api.post(`/inventory/${item.inventory_id}/reserve`, {
    quantity: item.quantity,
    order_id: order.id
  });
}
```

### Use Compression for Large Payloads

**✅ DO:**
```javascript
const client = axios.create({
  baseURL: 'http://localhost:8080/api/v1',
  headers: {
    'Accept-Encoding': 'gzip, deflate'
  },
  decompress: true
});
```

### Implement Request Deduplication

**✅ DO:**
```javascript
class RequestDeduplicator {
  constructor() {
    this.pending = new Map();
  }

  async request(key, fn) {
    // Return existing pending request
    if (this.pending.has(key)) {
      return await this.pending.get(key);
    }

    // Create new request
    const promise = fn().finally(() => {
      this.pending.delete(key);
    });

    this.pending.set(key, promise);
    return await promise;
  }
}

// Usage - multiple simultaneous requests for same product
const dedup = new RequestDeduplicator();

// All 3 calls will share the same HTTP request
const [p1, p2, p3] = await Promise.all([
  dedup.request('product-123', () => api.get('/products/123')),
  dedup.request('product-123', () => api.get('/products/123')),
  dedup.request('product-123', () => api.get('/products/123'))
]);
```

---

## Data Management

### Validate Data Before Sending

**✅ DO:**
```javascript
function validateOrder(order) {
  const errors = [];

  if (!order.customer_id || !isValidUUID(order.customer_id)) {
    errors.push('Invalid customer_id');
  }

  if (!order.items || order.items.length === 0) {
    errors.push('Order must have at least one item');
  }

  for (const item of order.items) {
    if (!item.product_id || !isValidUUID(item.product_id)) {
      errors.push(`Invalid product_id: ${item.product_id}`);
    }
    if (item.quantity <= 0) {
      errors.push('Quantity must be positive');
    }
  }

  if (errors.length > 0) {
    throw new ValidationError(errors);
  }

  return order;
}

// Usage
const validatedOrder = validateOrder(orderData);
await api.post('/orders', validatedOrder);
```

**❌ DON'T:**
```javascript
// No validation - API will reject and waste round trip
await api.post('/orders', {
  customer_id: 'not-a-uuid',  // ⚠️ Invalid
  items: []                    // ⚠️ Empty
});
```

### Handle Inventory Carefully

**✅ DO:**
```javascript
async function createOrderWithInventory(orderData) {
  // 1. Check inventory availability first
  const availability = await api.post('/inventory/check-availability', {
    items: orderData.items
  });

  if (!availability.all_available) {
    throw new InsufficientInventoryError(availability.unavailable_items);
  }

  // 2. Create order
  const order = await api.post('/orders', orderData);

  try {
    // 3. Reserve inventory
    await api.post('/inventory/reserve', {
      order_id: order.id,
      items: orderData.items
    });

    return order;
  } catch (error) {
    // 4. Cancel order if reservation fails
    await api.post(`/orders/${order.id}/cancel`, {
      reason: 'Inventory reservation failed'
    });
    throw error;
  }
}
```

**❌ DON'T:**
```javascript
// Create order without checking inventory
const order = await api.post('/orders', orderData);
// ⚠️ Might oversell - no inventory check
```

### Use Transactions for Related Operations

**✅ DO:**
```javascript
async function processRefund(returnId) {
  // Use idempotency key for the entire workflow
  const idempotencyKey = `refund-${returnId}-${Date.now()}`;

  try {
    // 1. Approve return
    await api.post(`/returns/${returnId}/approve`, null, {
      headers: { 'Idempotency-Key': `${idempotencyKey}-approve` }
    });

    // 2. Restock inventory
    await api.post(`/returns/${returnId}/restock`, null, {
      headers: { 'Idempotency-Key': `${idempotencyKey}-restock` }
    });

    // 3. Process refund
    await api.post('/payments/refund', {
      return_id: returnId
    }, {
      headers: { 'Idempotency-Key': `${idempotencyKey}-payment` }
    });
  } catch (error) {
    // Log error with idempotency key for investigation
    logger.error('Refund processing failed', {
      returnId,
      idempotencyKey,
      error: error.message
    });
    throw error;
  }
}
```

---

## Integration Patterns

### Use Webhooks Instead of Polling

**✅ DO:**
```javascript
// Register webhook endpoint
app.post('/webhooks/stateset', async (req, res) => {
  const event = req.body;

  switch (event.type) {
    case 'order.status_changed':
      await updateOrderStatus(event.data);
      break;
    case 'shipment.shipped':
      await notifyCustomer(event.data);
      break;
  }

  res.status(200).send('OK');
});
```

**❌ DON'T:**
```javascript
// Polling every 10 seconds - inefficient
setInterval(async () => {
  const orders = await api.get('/orders?status=shipped');
  for (const order of orders) {
    await processShippedOrder(order);
  }
}, 10000);
```

**Why:** Webhooks are real-time, efficient, and don't hit rate limits.

### Store External IDs for Sync

**✅ DO:**
```javascript
// Store both StateSet ID and your system's ID
const order = await api.post('/orders', {
  ...orderData,
  external_id: yourSystemOrderId,
  external_system: 'your-ecommerce-platform'
});

// Store mapping in your database
await db.orders.create({
  id: yourSystemOrderId,
  stateset_id: order.id,
  stateset_order_number: order.order_number
});
```

**Why:** Bidirectional mapping enables easy sync and troubleshooting.

### Implement Graceful Degradation

**✅ DO:**
```javascript
async function getProductWithFallback(productId) {
  try {
    // Try to get from primary API
    return await api.get(`/products/${productId}`);
  } catch (error) {
    // Fall back to cache
    const cached = await cache.get(`product:${productId}`);
    if (cached) {
      logger.warn('Using cached product data', { productId });
      return cached;
    }

    // Fall back to minimal data
    return {
      id: productId,
      name: 'Product Unavailable',
      available: false
    };
  }
}
```

---

## Testing Strategies

### Test Error Scenarios

**✅ DO:**
```javascript
describe('Order Creation', () => {
  it('should handle insufficient inventory', async () => {
    // Mock API to return inventory error
    mockAPI.post('/orders').replyOnce(422, {
      error: {
        code: 'INSUFFICIENT_INVENTORY',
        details: { product_id: '123', available: 5 }
      }
    });

    await expect(createOrder(orderData)).rejects.toThrow(
      InsufficientInventoryError
    );

    expect(notifyOutOfStock).toHaveBeenCalledWith('123');
  });

  it('should retry on network errors', async () => {
    mockAPI.post('/orders')
      .replyOnce(500) // First attempt fails
      .replyOnce(200, { id: 'order-123' }); // Retry succeeds

    const order = await createOrder(orderData);
    expect(order.id).toBe('order-123');
  });
});
```

### Use Test Fixtures

**✅ DO:**
```javascript
// test/fixtures/orders.js
export const validOrder = {
  customer_id: 'test-customer-uuid',
  items: [{
    product_id: 'test-product-uuid',
    sku: 'TEST-001',
    quantity: 2,
    unit_price: 29.99
  }],
  total_amount: 59.98
};

// test/orders.test.js
import { validOrder } from './fixtures/orders';

test('creates order', async () => {
  const order = await api.post('/orders', validOrder);
  expect(order.id).toBeDefined();
});
```

### Test Idempotency

**✅ DO:**
```javascript
test('idempotency prevents duplicate orders', async () => {
  const idempotencyKey = 'test-key-123';

  // First request
  const order1 = await api.post('/orders', orderData, {
    headers: { 'Idempotency-Key': idempotencyKey }
  });

  // Duplicate request with same key
  const order2 = await api.post('/orders', orderData, {
    headers: { 'Idempotency-Key': idempotencyKey }
  });

  // Should return same order
  expect(order1.id).toBe(order2.id);

  // Verify only one order was created
  const orders = await db.orders.count();
  expect(orders).toBe(1);
});
```

---

## Monitoring & Observability

### Log Structured Data

**✅ DO:**
```javascript
logger.info('Order created', {
  orderId: order.id,
  orderNumber: order.order_number,
  customerId: order.customer_id,
  totalAmount: order.total_amount,
  itemCount: order.items.length,
  requestId: requestId,
  timestamp: new Date().toISOString()
});
```

**❌ DON'T:**
```javascript
console.log(`Order created: ${order.id}`);
// ⚠️ Unstructured, hard to parse, missing context
```

### Track Key Metrics

**✅ DO:**
```javascript
const metrics = {
  orderCreated: new Counter('orders_created_total'),
  orderValue: new Histogram('order_value_dollars'),
  apiLatency: new Histogram('api_request_duration_ms'),
  apiErrors: new Counter('api_errors_total', ['code'])
};

async function createOrder(orderData) {
  const start = Date.now();

  try {
    const order = await api.post('/orders', orderData);

    metrics.orderCreated.inc();
    metrics.orderValue.observe(order.total_amount);
    metrics.apiLatency.observe(Date.now() - start);

    return order;
  } catch (error) {
    metrics.apiErrors.inc({ code: error.code });
    throw error;
  }
}
```

### Set Up Alerts

**✅ DO:**
```javascript
// Alert on high error rate
if (errorRate > 0.05) {  // 5% error rate
  alert('High API error rate', {
    errorRate,
    service: 'stateset-api',
    severity: 'critical'
  });
}

// Alert on slow responses
if (p95Latency > 1000) {  // 1 second
  alert('Slow API responses', {
    p95Latency,
    service: 'stateset-api',
    severity: 'warning'
  });
}
```

---

## Common Anti-Patterns

### ❌ Not Using Request IDs

**Problem:** Can't correlate client and server logs

**Solution:** Always log and store request IDs from headers

### ❌ Hardcoded URLs

**Problem:** Can't switch between environments

**Solution:** Use configuration/environment variables

### ❌ No Timeout Configuration

**Problem:** Requests hang indefinitely

**Solution:** Set appropriate timeouts (30-60 seconds)

### ❌ Ignoring Rate Limit Headers

**Problem:** Get rate limited unexpectedly

**Solution:** Monitor `X-RateLimit-Remaining` and slow down proactively

### ❌ No Retry Logic

**Problem:** Transient errors cause failures

**Solution:** Implement exponential backoff with retries

### ❌ Exposing Full Error Details to Users

**Problem:** Security information leakage

**Solution:** Log details, show user-friendly messages

### ❌ Not Validating Webhooks

**Problem:** Accept fake or malicious webhooks

**Solution:** Always verify webhook signatures

### ❌ Synchronous Order Processing

**Problem:** Slow checkout experience

**Solution:** Create order quickly, process async via webhooks

---

## Summary Checklist

**Authentication:**
- [ ] Use environment variables for API keys
- [ ] Implement automatic token refresh
- [ ] Use scoped API keys
- [ ] Never log sensitive data

**Error Handling:**
- [ ] Implement exponential backoff
- [ ] Don't retry 4xx errors
- [ ] Use circuit breaker pattern
- [ ] Handle specific error codes

**Performance:**
- [ ] Use pagination
- [ ] Cache frequently accessed data
- [ ] Batch related operations
- [ ] Filter on server side

**Integration:**
- [ ] Use webhooks instead of polling
- [ ] Verify webhook signatures
- [ ] Store external IDs for sync
- [ ] Implement graceful degradation

**Observability:**
- [ ] Log structured data
- [ ] Track request IDs
- [ ] Monitor key metrics
- [ ] Set up alerts

**Testing:**
- [ ] Test error scenarios
- [ ] Test retry logic
- [ ] Test idempotency
- [ ] Use test fixtures

---

**Want more details?**
- [Integration Guide](./INTEGRATION_GUIDE.md) - Complete integration patterns
- [Troubleshooting Guide](./TROUBLESHOOTING.md) - Common issues and solutions
- [API Overview](./API_OVERVIEW.md) - Complete API reference

[← Back to Documentation Index](./DOCUMENTATION_INDEX.md)
