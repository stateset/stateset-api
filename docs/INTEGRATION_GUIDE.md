# StateSet API - Integration Guide

## Table of Contents

- [Getting Started](#getting-started)
- [Authentication Strategies](#authentication-strategies)
- [Webhook Integration](#webhook-integration)
- [Error Handling Patterns](#error-handling-patterns)
- [Rate Limiting & Throttling](#rate-limiting--throttling)
- [Idempotency Implementation](#idempotency-implementation)
- [Event-Driven Integration](#event-driven-integration)
- [Third-Party Platform Integrations](#third-party-platform-integrations)
- [Testing Your Integration](#testing-your-integration)
- [Production Checklist](#production-checklist)

---

## Getting Started

### Quick Start Integration

```javascript
// Install dependencies
npm install axios dotenv

// Create .env file
// STATESET_API_BASE_URL=http://localhost:8080/api/v1
// STATESET_API_KEY=your-api-key
// STATESET_WEBHOOK_SECRET=your-webhook-secret

// Basic client setup
const axios = require('axios');
require('dotenv').config();

class StateSetClient {
  constructor() {
    this.client = axios.create({
      baseURL: process.env.STATESET_API_BASE_URL,
      headers: {
        'X-API-Key': process.env.STATESET_API_KEY,
        'Content-Type': 'application/json'
      },
      timeout: 30000
    });

    // Add request interceptor for logging
    this.client.interceptors.request.use(
      config => {
        console.log(`${config.method.toUpperCase()} ${config.url}`);
        return config;
      }
    );

    // Add response interceptor for error handling
    this.client.interceptors.response.use(
      response => response,
      error => this.handleError(error)
    );
  }

  handleError(error) {
    if (error.response) {
      console.error('API Error:', {
        status: error.response.status,
        code: error.response.data.error?.code,
        message: error.response.data.error?.message,
        requestId: error.response.headers['x-request-id']
      });
    }
    throw error;
  }

  // Order operations
  async createOrder(orderData) {
    const response = await this.client.post('/orders', orderData, {
      headers: {
        'Idempotency-Key': this.generateIdempotencyKey()
      }
    });
    return response.data.data;
  }

  async getOrder(orderId) {
    const response = await this.client.get(`/orders/${orderId}`);
    return response.data.data;
  }

  async listOrders(filters = {}) {
    const response = await this.client.get('/orders', { params: filters });
    return response.data.data;
  }

  // Inventory operations
  async getInventory(productId, locationId) {
    const response = await this.client.get('/inventory', {
      params: { product_id: productId, location_id: locationId }
    });
    return response.data.data;
  }

  async reserveInventory(inventoryId, quantity, orderId) {
    const response = await this.client.post(
      `/inventory/${inventoryId}/reserve`,
      { quantity, order_id: orderId }
    );
    return response.data.data;
  }

  // Utility
  generateIdempotencyKey() {
    return `${Date.now()}-${Math.random().toString(36).substring(7)}`;
  }
}

// Usage
const client = new StateSetClient();

async function main() {
  try {
    const order = await client.createOrder({
      customer_id: 'customer-uuid',
      items: [{
        product_id: 'product-uuid',
        quantity: 2,
        unit_price: 29.99
      }],
      total_amount: 59.98
    });

    console.log('Order created:', order.order_number);
  } catch (error) {
    console.error('Failed to create order');
  }
}

main();
```

---

## Authentication Strategies

### 1. JWT Token Authentication (User Sessions)

**Best for**: Web applications, mobile apps with user login

```javascript
class StateSetAuthClient {
  constructor(baseURL) {
    this.baseURL = baseURL;
    this.accessToken = null;
    this.refreshToken = null;
    this.tokenExpiry = null;
  }

  async login(email, password) {
    const response = await axios.post(`${this.baseURL}/auth/login`, {
      email,
      password
    });

    this.accessToken = response.data.data.access_token;
    this.refreshToken = response.data.data.refresh_token;
    this.tokenExpiry = Date.now() + (response.data.data.expires_in * 1000);

    // Store in secure storage
    await this.storeTokens();

    return response.data.data;
  }

  async refreshAccessToken() {
    if (!this.refreshToken) {
      throw new Error('No refresh token available');
    }

    const response = await axios.post(`${this.baseURL}/auth/refresh`, {
      refresh_token: this.refreshToken
    });

    this.accessToken = response.data.data.access_token;
    this.tokenExpiry = Date.now() + (response.data.data.expires_in * 1000);

    await this.storeTokens();

    return response.data.data;
  }

  async getAccessToken() {
    // Check if token is about to expire (within 5 minutes)
    if (this.tokenExpiry && Date.now() > (this.tokenExpiry - 300000)) {
      await this.refreshAccessToken();
    }

    return this.accessToken;
  }

  async makeAuthenticatedRequest(method, url, data = null) {
    const token = await this.getAccessToken();

    const config = {
      method,
      url: `${this.baseURL}${url}`,
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json'
      }
    };

    if (data) {
      config.data = data;
    }

    try {
      const response = await axios(config);
      return response.data;
    } catch (error) {
      if (error.response?.status === 401) {
        // Token invalid, try refreshing
        await this.refreshAccessToken();

        // Retry request with new token
        config.headers.Authorization = `Bearer ${this.accessToken}`;
        const retryResponse = await axios(config);
        return retryResponse.data;
      }
      throw error;
    }
  }

  async storeTokens() {
    // Implement secure storage (keychain, secure enclave, etc.)
    // For server-side, use encrypted storage
    // For client-side, use httpOnly cookies or secure storage APIs
  }

  async logout() {
    await axios.post(`${this.baseURL}/auth/logout`, null, {
      headers: { 'Authorization': `Bearer ${this.accessToken}` }
    });

    this.accessToken = null;
    this.refreshToken = null;
    this.tokenExpiry = null;
  }
}
```

### 2. API Key Authentication (Service-to-Service)

**Best for**: Backend services, scheduled jobs, webhooks

```javascript
class StateSetAPIKeyClient {
  constructor(apiKey, baseURL) {
    this.client = axios.create({
      baseURL: baseURL || process.env.STATESET_API_BASE_URL,
      headers: {
        'X-API-Key': apiKey,
        'Content-Type': 'application/json'
      }
    });
  }

  // API key management
  async createAPIKey(name, permissions, expiresAt) {
    // Use user JWT to create API key
    const response = await axios.post(
      `${this.baseURL}/auth/api-keys`,
      {
        name,
        permissions,
        expires_at: expiresAt
      },
      {
        headers: {
          'Authorization': `Bearer ${userJWT}`,
          'Content-Type': 'application/json'
        }
      }
    );

    // IMPORTANT: Save the key immediately, it won't be shown again
    const apiKey = response.data.data.key;
    console.log('API Key (save this!):', apiKey);

    return response.data.data;
  }

  async listAPIKeys() {
    const response = await this.client.get('/auth/api-keys');
    return response.data.data;
  }

  async revokeAPIKey(keyId) {
    const response = await this.client.delete(`/auth/api-keys/${keyId}`);
    return response.data;
  }
}

// Best practice: Use different API keys for different environments
const productionClient = new StateSetAPIKeyClient(
  process.env.STATESET_PROD_API_KEY,
  'https://api.stateset.com/api/v1'
);

const stagingClient = new StateSetAPIKeyClient(
  process.env.STATESET_STAGING_API_KEY,
  'https://staging-api.stateset.com/api/v1'
);
```

---

## Webhook Integration

### Setting Up Webhooks

```javascript
const express = require('express');
const crypto = require('crypto');

const app = express();

// Use raw body for signature verification
app.use('/webhooks', express.raw({ type: 'application/json' }));

// Verify webhook signature
function verifyWebhookSignature(payload, signature, secret) {
  const computedSignature = crypto
    .createHmac('sha256', secret)
    .update(payload)
    .digest('hex');

  return crypto.timingSafeEqual(
    Buffer.from(signature),
    Buffer.from(computedSignature)
  );
}

// Webhook handler
app.post('/webhooks/stateset', async (req, res) => {
  const signature = req.headers['x-stateset-signature'];
  const payload = req.body;

  // Verify signature
  if (!verifyWebhookSignature(payload, signature, process.env.WEBHOOK_SECRET)) {
    console.error('Invalid webhook signature');
    return res.status(401).json({ error: 'Invalid signature' });
  }

  // Parse event
  const event = JSON.parse(payload.toString());

  console.log('Webhook received:', event.type);

  try {
    // Handle different event types
    switch (event.type) {
      case 'order.created':
        await handleOrderCreated(event.data);
        break;

      case 'order.status_changed':
        await handleOrderStatusChanged(event.data);
        break;

      case 'shipment.shipped':
        await handleShipmentShipped(event.data);
        break;

      case 'shipment.delivered':
        await handleShipmentDelivered(event.data);
        break;

      case 'payment.processed':
        await handlePaymentProcessed(event.data);
        break;

      case 'payment.failed':
        await handlePaymentFailed(event.data);
        break;

      case 'inventory.low_stock':
        await handleLowStock(event.data);
        break;

      case 'return.created':
        await handleReturnCreated(event.data);
        break;

      case 'return.approved':
        await handleReturnApproved(event.data);
        break;

      default:
        console.log('Unhandled event type:', event.type);
    }

    // Always respond with 200 to acknowledge receipt
    res.json({ received: true });
  } catch (error) {
    console.error('Error processing webhook:', error);
    // Still return 200 to prevent retries for application errors
    res.json({ received: true, error: error.message });
  }
});

// Event handlers
async function handleOrderCreated(order) {
  console.log('New order:', order.order_number);

  // Send confirmation email
  await sendEmail(order.customer_email, 'Order Confirmation', {
    orderNumber: order.order_number,
    total: order.total_amount
  });

  // Update internal systems
  await updateCRM('order_created', order);

  // Trigger fulfillment workflow
  await triggerFulfillment(order.id);
}

async function handleOrderStatusChanged(data) {
  const { order_id, old_status, new_status } = data;

  console.log(`Order ${order_id}: ${old_status} -> ${new_status}`);

  if (new_status === 'shipped') {
    // Send shipment notification
    const order = await client.getOrder(order_id);
    await sendEmail(order.customer_email, 'Order Shipped', {
      orderNumber: order.order_number,
      trackingNumber: order.tracking_number
    });
  }
}

async function handleShipmentShipped(shipment) {
  console.log('Shipment shipped:', shipment.tracking_number);

  // Update tracking in customer portal
  await updateCustomerPortal(shipment.order_id, {
    status: 'in_transit',
    trackingNumber: shipment.tracking_number,
    carrier: shipment.carrier
  });

  // Send SMS notification
  await sendSMS(shipment.phone_number,
    `Your order has shipped! Track: ${shipment.tracking_url}`
  );
}

async function handleLowStock(inventory) {
  console.log('Low stock alert:', inventory.sku);

  // Check reorder point
  if (inventory.quantity_available <= inventory.reorder_point) {
    // Auto-create purchase order with supplier
    await createPurchaseOrder({
      product_id: inventory.product_id,
      quantity: inventory.safety_stock * 2,
      supplier_id: inventory.preferred_supplier_id
    });

    // Notify purchasing team
    await sendSlackAlert('purchasing',
      `Low stock: ${inventory.sku} - PO created automatically`
    );
  }
}

app.listen(3000, () => {
  console.log('Webhook server running on port 3000');
});
```

### Webhook Retry Logic

```javascript
// Implement webhook retry with exponential backoff
class WebhookSender {
  constructor() {
    this.maxRetries = 5;
    this.baseDelay = 1000; // 1 second
  }

  async sendWebhook(url, payload, secret, attempt = 1) {
    const signature = crypto
      .createHmac('sha256', secret)
      .update(JSON.stringify(payload))
      .digest('hex');

    try {
      const response = await axios.post(url, payload, {
        headers: {
          'Content-Type': 'application/json',
          'X-StateSet-Signature': signature,
          'X-StateSet-Event': payload.type,
          'X-StateSet-Delivery': crypto.randomUUID()
        },
        timeout: 10000
      });

      if (response.status === 200) {
        console.log('Webhook delivered successfully');
        return true;
      }
    } catch (error) {
      console.error(`Webhook delivery failed (attempt ${attempt}):`, error.message);

      if (attempt < this.maxRetries) {
        const delay = this.baseDelay * Math.pow(2, attempt - 1);
        console.log(`Retrying in ${delay}ms...`);

        await new Promise(resolve => setTimeout(resolve, delay));
        return this.sendWebhook(url, payload, secret, attempt + 1);
      }

      console.error('Max retries reached, webhook failed');
      // Store failed webhook for manual review
      await storeFailed Webhook(url, payload, error);
      return false;
    }
  }
}
```

---

## Error Handling Patterns

### Comprehensive Error Handler

```javascript
class APIError extends Error {
  constructor(response) {
    super(response.data.error?.message || 'API Error');
    this.name = 'APIError';
    this.status = response.status;
    this.code = response.data.error?.code;
    this.details = response.data.error?.details;
    this.requestId = response.headers['x-request-id'];
  }
}

class StateSetClientWithErrorHandling {
  async request(method, url, data = null) {
    try {
      const response = await this.client.request({ method, url, data });
      return response.data.data;
    } catch (error) {
      if (error.response) {
        throw new APIError(error.response);
      } else if (error.request) {
        throw new Error('No response from API - network error');
      } else {
        throw error;
      }
    }
  }

  async createOrderWithRetry(orderData, maxRetries = 3) {
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
      try {
        return await this.request('POST', '/orders', orderData);
      } catch (error) {
        if (error instanceof APIError) {
          // Don't retry client errors (4xx)
          if (error.status >= 400 && error.status < 500) {
            console.error('Client error:', error.code, error.message);
            throw error;
          }

          // Retry server errors (5xx)
          if (error.status >= 500 && attempt < maxRetries) {
            const delay = Math.pow(2, attempt) * 1000;
            console.log(`Server error, retrying in ${delay}ms... (attempt ${attempt})`);
            await new Promise(resolve => setTimeout(resolve, delay));
            continue;
          }
        }

        throw error;
      }
    }
  }
}

// Usage with error handling
async function processOrder(orderData) {
  try {
    const order = await client.createOrderWithRetry(orderData);
    console.log('Order created:', order.order_number);
    return order;
  } catch (error) {
    if (error instanceof APIError) {
      switch (error.code) {
        case 'INSUFFICIENT_INVENTORY':
          console.error('Out of stock:', error.details.product_id);
          // Handle inventory issue
          await notifyCustomer('Out of stock');
          break;

        case 'PAYMENT_FAILED':
          console.error('Payment declined');
          // Handle payment failure
          await requestAlternativePayment();
          break;

        case 'INVALID_ADDRESS':
          console.error('Invalid shipping address');
          // Request address correction
          await requestAddressVerification();
          break;

        default:
          console.error('Unknown error:', error.code);
          await logError(error);
      }
    } else {
      console.error('Unexpected error:', error);
      await logError(error);
    }
  }
}
```

---

## Rate Limiting & Throttling

### Client-Side Rate Limiting

```javascript
class RateLimitedClient {
  constructor(client, requestsPerSecond = 10) {
    this.client = client;
    this.interval = 1000 / requestsPerSecond;
    this.lastRequestTime = 0;
    this.queue = [];
    this.processing = false;
  }

  async request(method, url, data = null) {
    return new Promise((resolve, reject) => {
      this.queue.push({ method, url, data, resolve, reject });
      this.processQueue();
    });
  }

  async processQueue() {
    if (this.processing || this.queue.length === 0) {
      return;
    }

    this.processing = true;

    while (this.queue.length > 0) {
      const now = Date.now();
      const timeSinceLastRequest = now - this.lastRequestTime;

      if (timeSinceLastRequest < this.interval) {
        await new Promise(resolve =>
          setTimeout(resolve, this.interval - timeSinceLastRequest)
        );
      }

      const { method, url, data, resolve, reject } = this.queue.shift();

      try {
        this.lastRequestTime = Date.now();
        const response = await this.client.request({ method, url, data });

        // Check rate limit headers
        const remaining = response.headers['x-ratelimit-remaining'];
        const reset = response.headers['x-ratelimit-reset'];

        if (remaining && parseInt(remaining) < 5) {
          console.warn(`Rate limit low: ${remaining} requests remaining`);
          console.warn(`Resets at: ${new Date(parseInt(reset) * 1000)}`);
        }

        resolve(response.data.data);
      } catch (error) {
        if (error.response?.status === 429) {
          // Rate limit exceeded, wait and retry
          const retryAfter = error.response.headers['retry-after'] || 60;
          console.log(`Rate limited, waiting ${retryAfter}s...`);
          await new Promise(resolve => setTimeout(resolve, retryAfter * 1000));

          // Re-queue the request
          this.queue.unshift({ method, url, data, resolve, reject });
        } else {
          reject(error);
        }
      }
    }

    this.processing = false;
  }
}

// Usage
const rateLimitedClient = new RateLimitedClient(
  axios.create({ baseURL: 'http://localhost:8080/api/v1' }),
  10 // 10 requests per second
);

// These will be automatically queued and throttled
for (let i = 0; i < 100; i++) {
  rateLimitedClient.request('GET', '/orders', { page: i });
}
```

---

## Idempotency Implementation

### Idempotency Key Generation

```javascript
class IdempotencyManager {
  constructor() {
    this.keys = new Map(); // Store keys with TTL
  }

  // Generate idempotency key based on operation and data
  generateKey(operation, data) {
    const hash = crypto
      .createHash('sha256')
      .update(JSON.stringify({ operation, data }))
      .digest('hex');

    return `idem-${operation}-${hash.substring(0, 16)}`;
  }

  // Store key to prevent duplicate requests
  storeKey(key, response, ttl = 600000) { // 10 minutes
    this.keys.set(key, {
      response,
      expiresAt: Date.now() + ttl
    });

    setTimeout(() => this.keys.delete(key), ttl);
  }

  // Check if operation already completed
  getCachedResponse(key) {
    const cached = this.keys.get(key);

    if (cached && cached.expiresAt > Date.now()) {
      return cached.response;
    }

    return null;
  }
}

// Usage
const idempotency = new IdempotencyManager();

async function createOrderIdempotent(orderData) {
  // Generate idempotency key based on order data
  const idempotencyKey = idempotency.generateKey('create_order', {
    customer_id: orderData.customer_id,
    items: orderData.items,
    timestamp: Math.floor(Date.now() / 60000) // Round to minute
  });

  // Check if we already made this request
  const cached = idempotency.getCachedResponse(idempotencyKey);
  if (cached) {
    console.log('Returning cached response');
    return cached;
  }

  // Make API request with idempotency key
  try {
    const response = await axios.post('/orders', orderData, {
      headers: {
        'Idempotency-Key': idempotencyKey,
        'X-API-Key': process.env.API_KEY
      }
    });

    // Cache the response
    idempotency.storeKey(idempotencyKey, response.data.data);

    return response.data.data;
  } catch (error) {
    if (error.response?.status === 409) {
      // Conflict - request already processed
      console.log('Request already processed');
      return error.response.data.original_response;
    }
    throw error;
  }
}
```

---

## Event-Driven Integration

### Subscribe to Events via Polling

```javascript
class EventPoller {
  constructor(client, pollInterval = 5000) {
    this.client = client;
    this.pollInterval = pollInterval;
    this.lastEventId = null;
    this.handlers = new Map();
    this.running = false;
  }

  // Register event handler
  on(eventType, handler) {
    if (!this.handlers.has(eventType)) {
      this.handlers.set(eventType, []);
    }
    this.handlers.get(eventType).push(handler);
  }

  // Start polling for events
  start() {
    if (this.running) return;

    this.running = true;
    this.poll();
  }

  // Stop polling
  stop() {
    this.running = false;
  }

  async poll() {
    while (this.running) {
      try {
        // Fetch new events
        const events = await this.client.get('/admin/outbox', {
          params: {
            after: this.lastEventId,
            limit: 100
          }
        });

        // Process each event
        for (const event of events.data.data) {
          await this.processEvent(event);
          this.lastEventId = event.id;
        }
      } catch (error) {
        console.error('Error polling events:', error);
      }

      // Wait before next poll
      await new Promise(resolve => setTimeout(resolve, this.pollInterval));
    }
  }

  async processEvent(event) {
    const handlers = this.handlers.get(event.type) || [];

    for (const handler of handlers) {
      try {
        await handler(event.data);
      } catch (error) {
        console.error(`Error handling event ${event.type}:`, error);
      }
    }
  }
}

// Usage
const poller = new EventPoller(client, 5000);

poller.on('order.created', async (order) => {
  console.log('New order:', order.order_number);
  await processNewOrder(order);
});

poller.on('inventory.low_stock', async (inventory) => {
  console.log('Low stock:', inventory.sku);
  await sendLowStockAlert(inventory);
});

poller.on('shipment.delivered', async (shipment) => {
  console.log('Delivered:', shipment.tracking_number);
  await sendDeliveryConfirmation(shipment);
});

poller.start();
```

---

## Third-Party Platform Integrations

### Shopify Integration

```javascript
class ShopifyStateSetSync {
  constructor(shopifyClient, stateSetClient) {
    this.shopify = shopifyClient;
    this.stateSet = stateSetClient;
  }

  // Sync products from StateSet to Shopify
  async syncProducts() {
    const products = await this.stateSet.get('/products');

    for (const product of products.data.items) {
      await this.shopify.product.create({
        title: product.name,
        variants: [{
          sku: product.sku,
          price: product.price,
          inventory_quantity: product.inventory_quantity
        }],
        metafields: [{
          namespace: 'stateset',
          key: 'product_id',
          value: product.id,
          type: 'single_line_text_field'
        }]
      });
    }
  }

  // Sync inventory levels
  async syncInventory() {
    const inventory = await this.stateSet.get('/inventory');

    for (const item of inventory.data.items) {
      // Find Shopify variant by SKU
      const shopifyProduct = await this.findShopifyProductBySKU(item.sku);

      if (shopifyProduct) {
        await this.shopify.inventoryLevel.set({
          inventory_item_id: shopifyProduct.inventory_item_id,
          location_id: this.getShopifyLocationId(item.location_id),
          available: item.quantity_available
        });
      }
    }
  }

  // Handle Shopify order webhook
  async handleShopifyOrder(shopifyOrder) {
    // Create order in StateSet
    const order = await this.stateSet.post('/orders', {
      external_id: shopifyOrder.id.toString(),
      external_platform: 'shopify',
      customer_id: await this.getOrCreateCustomer(shopifyOrder.customer),
      items: shopifyOrder.line_items.map(item => ({
        external_id: item.id.toString(),
        product_id: await this.getProductByShopifyId(item.product_id),
        sku: item.sku,
        quantity: item.quantity,
        unit_price: item.price,
        name: item.name
      })),
      total_amount: shopifyOrder.total_price,
      shipping_address: this.convertAddress(shopifyOrder.shipping_address),
      billing_address: this.convertAddress(shopifyOrder.billing_address)
    });

    return order;
  }

  // Update Shopify order with fulfillment info
  async updateShopifyFulfillment(stateSetShipment) {
    const order = await this.stateSet.get(`/orders/${stateSetShipment.order_id}`);

    await this.shopify.fulfillment.create(order.external_id, {
      tracking_number: stateSetShipment.tracking_number,
      tracking_company: stateSetShipment.carrier,
      tracking_url: stateSetShipment.tracking_url,
      line_items: stateSetShipment.items.map(item => ({
        id: item.external_id
      }))
    });
  }
}
```

---

## Testing Your Integration

### Unit Tests

```javascript
const assert = require('assert');
const nock = require('nock');

describe('StateSet Client', () => {
  let client;

  beforeEach(() => {
    client = new StateSetClient();
  });

  afterEach(() => {
    nock.cleanAll();
  });

  it('should create an order', async () => {
    const orderData = {
      customer_id: 'test-customer-uuid',
      items: [{
        product_id: 'test-product-uuid',
        quantity: 1,
        unit_price: 29.99
      }],
      total_amount: 29.99
    };

    nock('http://localhost:8080')
      .post('/api/v1/orders', orderData)
      .reply(200, {
        success: true,
        data: {
          id: 'test-order-uuid',
          order_number: 'ORD-12345',
          ...orderData
        }
      });

    const order = await client.createOrder(orderData);

    assert.strictEqual(order.order_number, 'ORD-12345');
    assert.strictEqual(order.total_amount, 29.99);
  });

  it('should handle API errors', async () => {
    nock('http://localhost:8080')
      .get('/api/v1/orders/invalid-uuid')
      .reply(404, {
        error: {
          code: 'ORDER_NOT_FOUND',
          message: 'Order not found',
          status: 404
        }
      });

    try {
      await client.getOrder('invalid-uuid');
      assert.fail('Should have thrown an error');
    } catch (error) {
      assert.strictEqual(error.status, 404);
      assert.strictEqual(error.code, 'ORDER_NOT_FOUND');
    }
  });

  it('should retry on server errors', async () => {
    let attempts = 0;

    nock('http://localhost:8080')
      .post('/api/v1/orders')
      .times(2)
      .reply(500, { error: 'Internal Server Error' });

    nock('http://localhost:8080')
      .post('/api/v1/orders')
      .reply(200, {
        success: true,
        data: { id: 'test-order-uuid' }
      });

    const order = await client.createOrderWithRetry({
      customer_id: 'test-customer-uuid',
      total_amount: 29.99
    });

    assert.ok(order.id);
  });
});
```

---

## Production Checklist

### Pre-Launch

- [ ] Use production API keys (not test keys)
- [ ] Implement proper error handling and retries
- [ ] Set up webhook endpoints with signature verification
- [ ] Configure idempotency for all write operations
- [ ] Implement rate limiting on client side
- [ ] Set up monitoring and alerting
- [ ] Test failure scenarios (payment failures, inventory issues)
- [ ] Configure timeouts appropriately
- [ ] Implement proper logging (but don't log sensitive data)
- [ ] Set up backup/recovery procedures
- [ ] Document your integration
- [ ] Load test your integration
- [ ] Set up staging environment matching production

### Security

- [ ] Store API keys in secure environment variables
- [ ] Use HTTPS for all API calls
- [ ] Validate all webhook signatures
- [ ] Never log sensitive data (tokens, payment details)
- [ ] Implement proper authentication flows
- [ ] Rotate API keys regularly
- [ ] Use least-privilege API key permissions
- [ ] Sanitize all user inputs
- [ ] Implement CORS properly
- [ ] Keep dependencies up to date

### Monitoring

- [ ] Track API response times
- [ ] Monitor error rates
- [ ] Set up alerts for critical failures
- [ ] Log request IDs for debugging
- [ ] Track webhook delivery success
- [ ] Monitor rate limit usage
- [ ] Set up uptime monitoring
- [ ] Create dashboards for key metrics

### Support & Documentation

- [ ] Document your integration architecture
- [ ] Create runbooks for common issues
- [ ] Set up error reporting/tracking (Sentry, etc.)
- [ ] Document API credentials management
- [ ] Create testing procedures
- [ ] Document webhook endpoint URLs
- [ ] Keep integration documentation up to date

---

For more information, see:
- [API Overview](./API_OVERVIEW.md)
- [Use Cases](./USE_CASES.md)
- [Examples](../examples/)
- Interactive API docs at `/swagger-ui`
