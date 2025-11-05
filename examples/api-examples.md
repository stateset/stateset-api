# StateSet API Examples

This guide provides practical examples for using the StateSet API across different use cases and programming languages.

## Table of Contents

- [Authentication](#authentication)
- [Orders Management](#orders-management)
- [Inventory Management](#inventory-management)
- [Returns Processing](#returns-processing)
- [Shipments](#shipments)
- [Payments](#payments)
- [E-Commerce Operations](#e-commerce-operations)
- [Analytics](#analytics)

## Base URL

```
http://localhost:8080/api/v1
```

For production, replace with your production domain.

---

## Authentication

### Register a New User

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "SecurePassword123!",
    "name": "John Doe"
  }'
```

**Response:**
```json
{
  "success": true,
  "data": {
    "access_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...",
    "refresh_token": "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9...",
    "token_type": "Bearer",
    "expires_in": 3600
  }
}
```

### Login

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "user@example.com",
    "password": "SecurePassword123!"
  }'
```

**JavaScript (Node.js):**
```javascript
const axios = require('axios');

async function login(email, password) {
  try {
    const response = await axios.post('http://localhost:8080/api/v1/auth/login', {
      email,
      password
    });

    const { access_token, refresh_token } = response.data.data;
    // Store tokens securely
    return { access_token, refresh_token };
  } catch (error) {
    console.error('Login failed:', error.response.data);
  }
}

// Usage
login('user@example.com', 'SecurePassword123!');
```

**Python:**
```python
import requests

def login(email, password):
    response = requests.post(
        'http://localhost:8080/api/v1/auth/login',
        json={'email': email, 'password': password}
    )

    if response.status_code == 200:
        data = response.json()['data']
        return data['access_token'], data['refresh_token']
    else:
        raise Exception(f"Login failed: {response.text}")

# Usage
access_token, refresh_token = login('user@example.com', 'SecurePassword123!')
```

### Create API Key (for service-to-service auth)

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/auth/api-keys \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Production Service Key",
    "expires_at": "2026-12-31T23:59:59Z",
    "permissions": ["orders:read", "orders:create", "inventory:read"]
  }'
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "key": "sk_live_a1b2c3d4e5f6g7h8i9j0",
    "name": "Production Service Key",
    "created_at": "2025-11-04T10:00:00Z"
  }
}
```

---

## Orders Management

### Create an Order

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "550e8400-e29b-41d4-a716-446655440001",
    "status": "pending",
    "total_amount": 199.98,
    "currency": "USD",
    "items": [
      {
        "product_id": "550e8400-e29b-41d4-a716-446655440002",
        "sku": "WIDGET-001",
        "quantity": 2,
        "unit_price": 99.99,
        "name": "Premium Widget"
      }
    ],
    "shipping_address": {
      "street": "123 Main St",
      "city": "San Francisco",
      "state": "CA",
      "postal_code": "94105",
      "country": "US"
    },
    "billing_address": {
      "street": "123 Main St",
      "city": "San Francisco",
      "state": "CA",
      "postal_code": "94105",
      "country": "US"
    }
  }'
```

**JavaScript:**
```javascript
async function createOrder(accessToken, orderData) {
  const response = await axios.post(
    'http://localhost:8080/api/v1/orders',
    {
      customer_id: orderData.customerId,
      status: 'pending',
      total_amount: orderData.totalAmount,
      currency: 'USD',
      items: orderData.items,
      shipping_address: orderData.shippingAddress,
      billing_address: orderData.billingAddress
    },
    {
      headers: {
        'Authorization': `Bearer ${accessToken}`,
        'Content-Type': 'application/json'
      }
    }
  );

  return response.data.data;
}

// Usage
const order = await createOrder(accessToken, {
  customerId: '550e8400-e29b-41d4-a716-446655440001',
  totalAmount: 199.98,
  items: [{
    product_id: '550e8400-e29b-41d4-a716-446655440002',
    sku: 'WIDGET-001',
    quantity: 2,
    unit_price: 99.99,
    name: 'Premium Widget'
  }],
  shippingAddress: {
    street: '123 Main St',
    city: 'San Francisco',
    state: 'CA',
    postal_code: '94105',
    country: 'US'
  },
  billingAddress: {
    street: '123 Main St',
    city: 'San Francisco',
    state: 'CA',
    postal_code: '94105',
    country: 'US'
  }
});

console.log('Order created:', order.id);
```

**Python:**
```python
def create_order(access_token, order_data):
    headers = {
        'Authorization': f'Bearer {access_token}',
        'Content-Type': 'application/json'
    }

    response = requests.post(
        'http://localhost:8080/api/v1/orders',
        json=order_data,
        headers=headers
    )

    return response.json()['data']

# Usage
order = create_order(access_token, {
    'customer_id': '550e8400-e29b-41d4-a716-446655440001',
    'status': 'pending',
    'total_amount': 199.98,
    'currency': 'USD',
    'items': [{
        'product_id': '550e8400-e29b-41d4-a716-446655440002',
        'sku': 'WIDGET-001',
        'quantity': 2,
        'unit_price': 99.99,
        'name': 'Premium Widget'
    }],
    'shipping_address': {
        'street': '123 Main St',
        'city': 'San Francisco',
        'state': 'CA',
        'postal_code': '94105',
        'country': 'US'
    }
})

print(f"Order created: {order['id']}")
```

### List Orders (with Pagination and Filters)

**cURL:**
```bash
# List all orders
curl -X GET "http://localhost:8080/api/v1/orders?page=1&limit=20" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"

# Filter by status
curl -X GET "http://localhost:8080/api/v1/orders?status=pending&page=1&limit=20" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"

# Filter by customer and date range
curl -X GET "http://localhost:8080/api/v1/orders?customer_id=550e8400-e29b-41d4-a716-446655440001&start_date=2025-01-01&end_date=2025-12-31" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

**JavaScript:**
```javascript
async function listOrders(accessToken, filters = {}) {
  const params = new URLSearchParams({
    page: filters.page || 1,
    limit: filters.limit || 20,
    ...(filters.status && { status: filters.status }),
    ...(filters.customer_id && { customer_id: filters.customer_id }),
    ...(filters.start_date && { start_date: filters.start_date }),
    ...(filters.end_date && { end_date: filters.end_date })
  });

  const response = await axios.get(
    `http://localhost:8080/api/v1/orders?${params}`,
    {
      headers: { 'Authorization': `Bearer ${accessToken}` }
    }
  );

  return response.data.data;
}

// Usage
const orders = await listOrders(accessToken, {
  status: 'pending',
  page: 1,
  limit: 20
});

console.log(`Found ${orders.total} orders`);
orders.items.forEach(order => {
  console.log(`Order ${order.order_number}: ${order.status}`);
});
```

### Get Order by ID

**cURL:**
```bash
curl -X GET http://localhost:8080/api/v1/orders/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

### Update Order Status

**cURL:**
```bash
curl -X PUT http://localhost:8080/api/v1/orders/550e8400-e29b-41d4-a716-446655440000/status \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "status": "processing",
    "notes": "Payment confirmed, preparing for shipment"
  }'
```

### Cancel an Order

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/orders/550e8400-e29b-41d4-a716-446655440000/cancel \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "reason": "Customer requested cancellation",
    "refund": true
  }'
```

---

## Inventory Management

### List Inventory

**cURL:**
```bash
# List all inventory
curl -X GET "http://localhost:8080/api/v1/inventory?page=1&limit=20" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"

# Filter by location
curl -X GET "http://localhost:8080/api/v1/inventory?location_id=550e8400-e29b-41d4-a716-446655440003" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"

# Get low stock items
curl -X GET "http://localhost:8080/api/v1/inventory/low-stock" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

**JavaScript:**
```javascript
async function getInventory(accessToken, filters = {}) {
  const params = new URLSearchParams({
    page: filters.page || 1,
    limit: filters.limit || 20,
    ...(filters.product_id && { product_id: filters.product_id }),
    ...(filters.location_id && { location_id: filters.location_id }),
    ...(filters.low_stock && { low_stock: filters.low_stock })
  });

  const response = await axios.get(
    `http://localhost:8080/api/v1/inventory?${params}`,
    {
      headers: { 'Authorization': `Bearer ${accessToken}` }
    }
  );

  return response.data.data;
}

// Get low stock items
const lowStock = await getInventory(accessToken, { low_stock: true });
lowStock.items.forEach(item => {
  console.log(`${item.sku}: ${item.quantity_available} units (reorder at ${item.reorder_point})`);
});
```

### Reserve Inventory

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/inventory/550e8400-e29b-41d4-a716-446655440000/reserve \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "quantity": 5,
    "order_id": "550e8400-e29b-41d4-a716-446655440001",
    "notes": "Reserved for order ORD-12345"
  }'
```

**JavaScript:**
```javascript
async function reserveInventory(accessToken, inventoryId, quantity, orderId) {
  const response = await axios.post(
    `http://localhost:8080/api/v1/inventory/${inventoryId}/reserve`,
    { quantity, order_id: orderId },
    {
      headers: {
        'Authorization': `Bearer ${accessToken}`,
        'Content-Type': 'application/json'
      }
    }
  );

  return response.data.data;
}

// Reserve 5 units
await reserveInventory(
  accessToken,
  '550e8400-e29b-41d4-a716-446655440000',
  5,
  '550e8400-e29b-41d4-a716-446655440001'
);
```

### Release Reserved Inventory

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/inventory/550e8400-e29b-41d4-a716-446655440000/release \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "quantity": 5,
    "reason": "Order cancelled"
  }'
```

---

## Returns Processing

### Create a Return Request

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/returns \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": "550e8400-e29b-41d4-a716-446655440001",
    "items": [
      {
        "order_item_id": "550e8400-e29b-41d4-a716-446655440002",
        "quantity": 1,
        "reason": "defective",
        "description": "Product arrived damaged"
      }
    ],
    "customer_notes": "Package was damaged during shipping"
  }'
```

**JavaScript:**
```javascript
async function createReturn(accessToken, returnData) {
  const response = await axios.post(
    'http://localhost:8080/api/v1/returns',
    returnData,
    {
      headers: {
        'Authorization': `Bearer ${accessToken}`,
        'Content-Type': 'application/json'
      }
    }
  );

  return response.data.data;
}

// Usage
const returnRequest = await createReturn(accessToken, {
  order_id: '550e8400-e29b-41d4-a716-446655440001',
  items: [{
    order_item_id: '550e8400-e29b-41d4-a716-446655440002',
    quantity: 1,
    reason: 'defective',
    description: 'Product arrived damaged'
  }],
  customer_notes: 'Package was damaged during shipping'
});

console.log(`Return RMA: ${returnRequest.rma_number}`);
```

### Approve a Return

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/returns/550e8400-e29b-41d4-a716-446655440000/approve \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "refund_amount": 99.99,
    "notes": "Return approved, full refund"
  }'
```

### Restock Returned Items

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/returns/550e8400-e29b-41d4-a716-446655440000/restock \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "location_id": "550e8400-e29b-41d4-a716-446655440003",
    "condition": "good"
  }'
```

---

## Shipments

### Create a Shipment

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/shipments \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": "550e8400-e29b-41d4-a716-446655440001",
    "carrier": "UPS",
    "service_level": "ground",
    "items": [
      {
        "order_item_id": "550e8400-e29b-41d4-a716-446655440002",
        "quantity": 2
      }
    ]
  }'
```

**JavaScript:**
```javascript
async function createShipment(accessToken, shipmentData) {
  const response = await axios.post(
    'http://localhost:8080/api/v1/shipments',
    shipmentData,
    {
      headers: {
        'Authorization': `Bearer ${accessToken}`,
        'Content-Type': 'application/json'
      }
    }
  );

  return response.data.data;
}

const shipment = await createShipment(accessToken, {
  order_id: '550e8400-e29b-41d4-a716-446655440001',
  carrier: 'UPS',
  service_level: 'ground',
  items: [{
    order_item_id: '550e8400-e29b-41d4-a716-446655440002',
    quantity: 2
  }]
});
```

### Mark Shipment as Shipped

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/shipments/550e8400-e29b-41d4-a716-446655440000/ship \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "tracking_number": "1Z999AA10123456784",
    "shipped_at": "2025-11-04T10:00:00Z"
  }'
```

### Track Shipment

**cURL:**
```bash
# Track by shipment ID
curl -X GET http://localhost:8080/api/v1/shipments/550e8400-e29b-41d4-a716-446655440000/track \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"

# Track by tracking number
curl -X GET http://localhost:8080/api/v1/shipments/track/1Z999AA10123456784 \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

**JavaScript:**
```javascript
async function trackShipment(accessToken, trackingNumber) {
  const response = await axios.get(
    `http://localhost:8080/api/v1/shipments/track/${trackingNumber}`,
    {
      headers: { 'Authorization': `Bearer ${accessToken}` }
    }
  );

  return response.data.data;
}

const tracking = await trackShipment(accessToken, '1Z999AA10123456784');
console.log(`Shipment status: ${tracking.status}`);
console.log(`Estimated delivery: ${tracking.estimated_delivery}`);
```

---

## Payments

### Process a Payment

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/payments \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "order_id": "550e8400-e29b-41d4-a716-446655440001",
    "amount": 199.98,
    "currency": "USD",
    "payment_method": "credit_card",
    "payment_details": {
      "card_number": "4111111111111111",
      "exp_month": "12",
      "exp_year": "2026",
      "cvv": "123",
      "cardholder_name": "John Doe"
    }
  }'
```

**JavaScript:**
```javascript
async function processPayment(accessToken, paymentData) {
  const response = await axios.post(
    'http://localhost:8080/api/v1/payments',
    paymentData,
    {
      headers: {
        'Authorization': `Bearer ${accessToken}`,
        'Content-Type': 'application/json',
        'Idempotency-Key': crypto.randomUUID() // Prevent duplicate charges
      }
    }
  );

  return response.data.data;
}

// Usage
const payment = await processPayment(accessToken, {
  order_id: '550e8400-e29b-41d4-a716-446655440001',
  amount: 199.98,
  currency: 'USD',
  payment_method: 'credit_card',
  payment_details: {
    card_number: '4111111111111111',
    exp_month: '12',
    exp_year: '2026',
    cvv: '123',
    cardholder_name: 'John Doe'
  }
});

console.log(`Payment status: ${payment.status}`);
console.log(`Transaction ID: ${payment.transaction_id}`);
```

### Refund a Payment

**cURL:**
```bash
curl -X POST http://localhost:8080/api/v1/payments/refund \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "payment_id": "550e8400-e29b-41d4-a716-446655440000",
    "amount": 199.98,
    "reason": "Customer requested refund"
  }'
```

### Get Payment Details

**cURL:**
```bash
curl -X GET http://localhost:8080/api/v1/payments/550e8400-e29b-41d4-a716-446655440000 \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

---

## E-Commerce Operations

### Product Management

**Create a Product:**
```bash
curl -X POST http://localhost:8080/api/v1/products \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Premium Widget",
    "sku": "WIDGET-001",
    "description": "High-quality widget for professional use",
    "price": 99.99,
    "currency": "USD",
    "category": "Widgets",
    "inventory_quantity": 100,
    "images": [
      "https://example.com/images/widget-001.jpg"
    ],
    "attributes": {
      "color": "Blue",
      "size": "Large",
      "weight": "2.5 lbs"
    }
  }'
```

**Search Products:**
```bash
curl -X GET "http://localhost:8080/api/v1/products/search?q=widget&category=Widgets&min_price=50&max_price=150" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

### Shopping Cart

**Create a Cart:**
```bash
curl -X POST http://localhost:8080/api/v1/carts \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "550e8400-e29b-41d4-a716-446655440001"
  }'
```

**Add Item to Cart:**
```bash
curl -X POST http://localhost:8080/api/v1/carts/550e8400-e29b-41d4-a716-446655440000/items \
  -H "Content-Type: application/json" \
  -d '{
    "product_id": "550e8400-e29b-41d4-a716-446655440002",
    "quantity": 2,
    "variant_id": null
  }'
```

**Update Cart Item:**
```bash
curl -X PUT http://localhost:8080/api/v1/carts/550e8400-e29b-41d4-a716-446655440000/items/550e8400-e29b-41d4-a716-446655440003 \
  -H "Content-Type: application/json" \
  -d '{
    "quantity": 3
  }'
```

### Checkout Flow

**JavaScript Complete Checkout Example:**
```javascript
async function completeCheckout(accessToken) {
  // 1. Start checkout
  const checkoutSession = await axios.post(
    'http://localhost:8080/api/v1/checkout',
    { cart_id: 'cart-uuid' },
    { headers: { 'Authorization': `Bearer ${accessToken}` }}
  );

  const sessionId = checkoutSession.data.data.id;

  // 2. Set customer info
  await axios.put(
    `http://localhost:8080/api/v1/checkout/${sessionId}/customer`,
    {
      email: 'customer@example.com',
      first_name: 'John',
      last_name: 'Doe'
    },
    { headers: { 'Authorization': `Bearer ${accessToken}` }}
  );

  // 3. Set shipping address
  await axios.put(
    `http://localhost:8080/api/v1/checkout/${sessionId}/shipping-address`,
    {
      street: '123 Main St',
      city: 'San Francisco',
      state: 'CA',
      postal_code: '94105',
      country: 'US'
    },
    { headers: { 'Authorization': `Bearer ${accessToken}` }}
  );

  // 4. Set shipping method
  await axios.put(
    `http://localhost:8080/api/v1/checkout/${sessionId}/shipping-method`,
    { method: 'standard', carrier: 'UPS' },
    { headers: { 'Authorization': `Bearer ${accessToken}` }}
  );

  // 5. Complete checkout
  const result = await axios.post(
    `http://localhost:8080/api/v1/checkout/${sessionId}/complete`,
    {
      payment_method: 'credit_card',
      payment_details: {
        card_number: '4111111111111111',
        exp_month: '12',
        exp_year: '2026',
        cvv: '123'
      }
    },
    { headers: { 'Authorization': `Bearer ${accessToken}` }}
  );

  return result.data.data; // Returns order details
}
```

---

## Analytics

### Get Dashboard Metrics

**cURL:**
```bash
curl -X GET http://localhost:8080/api/v1/analytics/dashboard \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

**Response:**
```json
{
  "success": true,
  "data": {
    "total_orders": 1250,
    "total_revenue": 125000.00,
    "average_order_value": 100.00,
    "orders_today": 45,
    "revenue_today": 4500.00,
    "low_stock_items": 12,
    "pending_shipments": 23
  }
}
```

### Get Sales Trends

**cURL:**
```bash
curl -X GET "http://localhost:8080/api/v1/analytics/sales/trends?start_date=2025-01-01&end_date=2025-12-31&interval=month" \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN"
```

**JavaScript:**
```javascript
async function getSalesTrends(accessToken, startDate, endDate, interval = 'month') {
  const response = await axios.get(
    'http://localhost:8080/api/v1/analytics/sales/trends',
    {
      params: { start_date: startDate, end_date: endDate, interval },
      headers: { 'Authorization': `Bearer ${accessToken}` }
    }
  );

  return response.data.data;
}

// Usage
const trends = await getSalesTrends(accessToken, '2025-01-01', '2025-12-31', 'month');
trends.forEach(point => {
  console.log(`${point.period}: $${point.revenue} (${point.order_count} orders)`);
});
```

---

## Error Handling

All API responses follow a consistent format:

**Success Response:**
```json
{
  "success": true,
  "data": { /* response data */ },
  "meta": {
    "request_id": "req-123",
    "timestamp": "2025-11-04T10:00:00Z"
  }
}
```

**Error Response:**
```json
{
  "success": false,
  "message": "Validation failed",
  "errors": [
    "Field 'email' is required",
    "Field 'password' must be at least 8 characters"
  ],
  "meta": {
    "request_id": "req-123",
    "timestamp": "2025-11-04T10:00:00Z"
  }
}
```

**JavaScript Error Handling:**
```javascript
async function apiCall() {
  try {
    const response = await axios.get('http://localhost:8080/api/v1/orders');
    return response.data.data;
  } catch (error) {
    if (error.response) {
      // Server responded with error
      console.error('API Error:', error.response.data.message);
      console.error('Details:', error.response.data.errors);
      console.error('Request ID:', error.response.data.meta.request_id);
    } else if (error.request) {
      // No response received
      console.error('Network error - no response received');
    } else {
      // Request setup error
      console.error('Error:', error.message);
    }
    throw error;
  }
}
```

---

## Rate Limiting

The API implements rate limiting with the following headers:

```
X-RateLimit-Limit: 100
X-RateLimit-Remaining: 95
X-RateLimit-Reset: 1730707200
```

**JavaScript Rate Limit Handler:**
```javascript
function checkRateLimit(response) {
  const remaining = response.headers['x-ratelimit-remaining'];
  const reset = response.headers['x-ratelimit-reset'];

  if (remaining && parseInt(remaining) < 10) {
    const resetDate = new Date(parseInt(reset) * 1000);
    console.warn(`Rate limit low: ${remaining} requests remaining until ${resetDate}`);
  }
}
```

---

## Idempotency

For write operations (POST, PUT, DELETE), use the `Idempotency-Key` header to prevent duplicate requests:

```bash
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Authorization: Bearer YOUR_ACCESS_TOKEN" \
  -H "Idempotency-Key: 550e8400-e29b-41d4-a716-446655440000" \
  -H "Content-Type: application/json" \
  -d '{ ... }'
```

**JavaScript:**
```javascript
const { v4: uuidv4 } = require('uuid');

async function createOrderIdempotent(accessToken, orderData) {
  const idempotencyKey = uuidv4();

  const response = await axios.post(
    'http://localhost:8080/api/v1/orders',
    orderData,
    {
      headers: {
        'Authorization': `Bearer ${accessToken}`,
        'Idempotency-Key': idempotencyKey,
        'Content-Type': 'application/json'
      }
    }
  );

  return response.data.data;
}
```

---

## Need More Help?

- **API Documentation**: Visit `http://localhost:8080/swagger-ui` for interactive API docs
- **CLI Tool**: Use `stateset-cli` for quick testing and development
- **Support**: Check the main README.md and docs/ directory for detailed guides
