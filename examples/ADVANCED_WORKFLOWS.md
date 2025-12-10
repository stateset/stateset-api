# StateSet API - Advanced Workflows

This document demonstrates advanced usage patterns and complete workflows for the StateSet API. These examples show how to combine multiple API endpoints to implement real-world business processes.

## Table of Contents

- [Complete E-Commerce Checkout Flow](#complete-e-commerce-checkout-flow)
- [Order Fulfillment Workflow](#order-fulfillment-workflow)
- [Returns Processing Workflow](#returns-processing-workflow)
- [Inventory Management with Reservations](#inventory-management-with-reservations)
- [Subscription Order Management](#subscription-order-management)
- [Multi-Location Inventory Transfer](#multi-location-inventory-transfer)
- [Partial Order Fulfillment](#partial-order-fulfillment)
- [Pre-Order Management](#pre-order-management)
- [Drop-Shipping Workflow](#drop-shipping-workflow)
- [Error Handling and Retry Patterns](#error-handling-and-retry-patterns)

---

## Complete E-Commerce Checkout Flow

This workflow demonstrates a complete checkout process from browsing products to order completion.

### Step 1: Create Customer (or Login)

```bash
# Register a new customer
curl -X POST http://localhost:8080/api/v1/customers \
  -H "Content-Type: application/json" \
  -d '{
    "email": "customer@example.com",
    "first_name": "Jane",
    "last_name": "Smith",
    "phone": "+1-555-0100"
  }'

# Response: { "id": "customer-uuid", ... }
```

### Step 2: Create Shopping Cart

```bash
CUSTOMER_ID="customer-uuid"

curl -X POST http://localhost:8080/api/v1/carts \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"customer_id\": \"$CUSTOMER_ID\",
    \"session_id\": \"session-$(uuidgen)\"
  }"

# Response: { "id": "cart-uuid", ... }
```

### Step 3: Add Products to Cart

```bash
CART_ID="cart-uuid"

# Add first product
curl -X POST http://localhost:8080/api/v1/carts/$CART_ID/items \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "product_id": "product-uuid-1",
    "sku": "WIDGET-001",
    "quantity": 2,
    "price": 99.99,
    "name": "Premium Widget"
  }'

# Add second product
curl -X POST http://localhost:8080/api/v1/carts/$CART_ID/items \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "product_id": "product-uuid-2",
    "sku": "GADGET-002",
    "quantity": 1,
    "price": 149.99,
    "name": "Smart Gadget"
  }'
```

### Step 4: Get Cart with Calculated Totals

```bash
curl -X GET http://localhost:8080/api/v1/carts/$CART_ID \
  -H "Authorization: Bearer $ACCESS_TOKEN"

# Response includes: subtotal, tax, shipping, total
```

### Step 5: Initiate Checkout

```bash
curl -X POST http://localhost:8080/api/v1/checkout \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"cart_id\": \"$CART_ID\",
    \"customer_id\": \"$CUSTOMER_ID\",
    \"shipping_address\": {
      \"street\": \"123 Main St\",
      \"city\": \"San Francisco\",
      \"state\": \"CA\",
      \"postal_code\": \"94105\",
      \"country\": \"US\"
    },
    \"billing_address\": {
      \"street\": \"123 Main St\",
      \"city\": \"San Francisco\",
      \"state\": \"CA\",
      \"postal_code\": \"94105\",
      \"country\": \"US\"
    },
    \"payment_method\": {
      \"type\": \"card\",
      \"token\": \"tok_visa_4242\"
    }
  }"

# Response: Order created with status "pending"
```

### TypeScript Implementation

```typescript
import StateSetClient from './typescript-example';

async function completeCheckoutFlow() {
  const client = new StateSetClient('http://localhost:8080/api/v1');

  // 1. Authenticate
  await client.login('customer@example.com', 'password');

  // 2. Create customer if needed
  const customer = await client.createCustomer({
    email: 'customer@example.com',
    first_name: 'Jane',
    last_name: 'Smith',
    phone: '+1-555-0100'
  });

  // 3. Create cart
  const cart = await client.createCart(customer.id);

  // 4. Add items
  await client.addItemToCart(cart.id, {
    product_id: 'product-uuid-1',
    sku: 'WIDGET-001',
    quantity: 2,
    price: 99.99,
    name: 'Premium Widget'
  });

  await client.addItemToCart(cart.id, {
    product_id: 'product-uuid-2',
    sku: 'GADGET-002',
    quantity: 1,
    price: 149.99,
    name: 'Smart Gadget'
  });

  // 5. Get cart totals
  const updatedCart = await client.getCart(cart.id);
  console.log(`Cart total: $${updatedCart.total}`);

  // 6. Checkout
  const order = await client.checkout(cart.id, {
    customer_id: customer.id,
    shipping_address: {
      street: '123 Main St',
      city: 'San Francisco',
      state: 'CA',
      postal_code: '94105',
      country: 'US'
    },
    billing_address: {
      street: '123 Main St',
      city: 'San Francisco',
      state: 'CA',
      postal_code: '94105',
      country: 'US'
    },
    payment_method: {
      type: 'card',
      token: 'tok_visa_4242'
    }
  });

  console.log(`Order created: ${order.id}`);
  return order;
}
```

---

## Order Fulfillment Workflow

Complete workflow from order placement to delivery.

### Step 1: Create Order

```bash
ORDER_RESPONSE=$(curl -X POST http://localhost:8080/api/v1/orders \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "customer-uuid",
    "status": "pending",
    "total_amount": 299.97,
    "currency": "USD",
    "items": [
      {
        "product_id": "product-uuid-1",
        "sku": "WIDGET-001",
        "quantity": 3,
        "unit_price": 99.99,
        "name": "Premium Widget"
      }
    ]
  }')

ORDER_ID=$(echo $ORDER_RESPONSE | jq -r '.id')
```

### Step 2: Reserve Inventory

```bash
# Get inventory item for the SKU
INVENTORY_RESPONSE=$(curl -X GET "http://localhost:8080/api/v1/inventory?sku=WIDGET-001" \
  -H "Authorization: Bearer $ACCESS_TOKEN")

INVENTORY_ID=$(echo $INVENTORY_RESPONSE | jq -r '.data[0].id')

# Reserve inventory
curl -X POST http://localhost:8080/api/v1/inventory/$INVENTORY_ID/reserve \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"quantity\": 3,
    \"order_id\": \"$ORDER_ID\",
    \"expires_at\": \"$(date -u -d '+24 hours' '+%Y-%m-%dT%H:%M:%SZ')\"
  }"
```

### Step 3: Update Order Status to Processing

```bash
curl -X PUT http://localhost:8080/api/v1/orders/$ORDER_ID/status \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "status": "processing",
    "notes": "Inventory reserved, ready for fulfillment"
  }'
```

### Step 4: Create Shipment

```bash
SHIPMENT_RESPONSE=$(curl -X POST http://localhost:8080/api/v1/shipments \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"order_id\": \"$ORDER_ID\",
    \"carrier\": \"UPS\",
    \"service_level\": \"ground\"
  }")

SHIPMENT_ID=$(echo $SHIPMENT_RESPONSE | jq -r '.id')
```

### Step 5: Mark as Shipped

```bash
TRACKING_NUMBER="1Z999AA10123456784"

curl -X POST http://localhost:8080/api/v1/shipments/$SHIPMENT_ID/ship \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"tracking_number\": \"$TRACKING_NUMBER\",
    \"shipped_at\": \"$(date -u '+%Y-%m-%dT%H:%M:%SZ')\"
  }"
```

### Step 6: Update Order Status to Shipped

```bash
curl -X PUT http://localhost:8080/api/v1/orders/$ORDER_ID/status \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"status\": \"shipped\",
    \"notes\": \"Shipped via UPS, tracking: $TRACKING_NUMBER\"
  }"
```

### Step 7: Mark as Delivered (When Delivered)

```bash
curl -X POST http://localhost:8080/api/v1/shipments/$SHIPMENT_ID/deliver \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"delivered_at\": \"$(date -u '+%Y-%m-%dT%H:%M:%SZ')\"
  }"

curl -X PUT http://localhost:8080/api/v1/orders/$ORDER_ID/status \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "status": "delivered",
    "notes": "Package delivered successfully"
  }'
```

---

## Returns Processing Workflow

Complete returns workflow with inspection and restocking.

### Step 1: Customer Creates Return Request

```bash
RETURN_RESPONSE=$(curl -X POST http://localhost:8080/api/v1/returns \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{
    \"order_id\": \"$ORDER_ID\",
    \"items\": [
      {
        \"order_item_id\": \"item-uuid\",
        \"quantity\": 1,
        \"reason\": \"defective\",
        \"description\": \"Product stopped working after 2 days\"
      }
    ],
    \"customer_notes\": \"Requesting replacement or refund\"
  }")

RETURN_ID=$(echo $RETURN_RESPONSE | jq -r '.id')
```

### Step 2: Review and Approve Return

```bash
# Approve the return
curl -X POST http://localhost:8080/api/v1/returns/$RETURN_ID/approve \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "approved_by": "manager-uuid",
    "notes": "Valid return, defective product confirmed"
  }'
```

### Step 3: Customer Ships Product Back

```bash
# Update return with tracking info
curl -X PUT http://localhost:8080/api/v1/returns/$RETURN_ID \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "return_tracking_number": "1Z999AA19876543210",
    "status": "in_transit"
  }'
```

### Step 4: Receive and Inspect

```bash
# Mark as received
curl -X PUT http://localhost:8080/api/v1/returns/$RETURN_ID \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "status": "received",
    "inspection_notes": "Product confirmed defective, eligible for refund"
  }'
```

### Step 5: Restock (If Applicable) or Dispose

```bash
# If product can be restocked
curl -X POST http://localhost:8080/api/v1/returns/$RETURN_ID/restock \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "location_id": "warehouse-uuid",
    "notes": "Restocked as B-grade item"
  }'

# Or mark as disposed
curl -X PUT http://localhost:8080/api/v1/returns/$RETURN_ID \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "status": "disposed",
    "disposition": "defective",
    "notes": "Product beyond repair, disposed per policy"
  }'
```

### Step 6: Process Refund

```bash
curl -X POST http://localhost:8080/api/v1/payments/refund \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Idempotency-Key: $(uuidgen)" \
  -H "Content-Type: application/json" \
  -d "{
    \"order_id\": \"$ORDER_ID\",
    \"amount\": 99.99,
    \"reason\": \"Return approved - defective product\",
    \"return_id\": \"$RETURN_ID\"
  }"
```

---

## Inventory Management with Reservations

Implement inventory reservations with automatic expiration handling.

### Reserve Inventory During Checkout

```typescript
async function reserveInventoryForOrder(
  client: StateSetClient,
  orderId: string,
  items: Array<{ sku: string; quantity: number }>
): Promise<void> {
  const reservations: string[] = [];

  try {
    // Reserve inventory for each item
    for (const item of items) {
      // Find inventory item by SKU
      const inventory = await client.listInventory({ sku: item.sku });
      const inventoryItem = inventory.data[0];

      if (!inventoryItem) {
        throw new Error(`Inventory not found for SKU: ${item.sku}`);
      }

      // Check if enough inventory is available
      if (inventoryItem.quantity_available < item.quantity) {
        throw new Error(
          `Insufficient inventory for ${item.sku}. ` +
          `Requested: ${item.quantity}, Available: ${inventoryItem.quantity_available}`
        );
      }

      // Reserve the inventory (expires in 24 hours)
      const reservation = await client.reserveInventory(
        inventoryItem.id,
        item.quantity,
        orderId
      );

      reservations.push(reservation.id);
    }

    console.log(`Successfully reserved inventory for order ${orderId}`);

  } catch (error) {
    // If any reservation fails, release all successful reservations
    console.error('Inventory reservation failed, rolling back...', error);

    for (const reservationId of reservations) {
      try {
        await client.releaseInventory(reservationId);
      } catch (releaseError) {
        console.error(`Failed to release reservation ${reservationId}`, releaseError);
      }
    }

    throw error;
  }
}
```

### Automatic Reservation Cleanup

```bash
# Run this periodically (e.g., via cron) to cleanup expired reservations
curl -X POST http://localhost:8080/api/v1/inventory/reservations/cleanup \
  -H "Authorization: Bearer $ACCESS_TOKEN" \
  -H "Content-Type: application/json"
```

---

## Subscription Order Management

Implement recurring subscription orders.

### Create Initial Subscription Order

```typescript
interface Subscription {
  id: string;
  customer_id: string;
  items: OrderItem[];
  frequency: 'weekly' | 'monthly' | 'quarterly';
  next_order_date: string;
  status: 'active' | 'paused' | 'cancelled';
}

async function createSubscription(
  client: StateSetClient,
  customerId: string,
  items: OrderItem[],
  frequency: string
): Promise<Subscription> {
  // Create first order
  const order = await client.createOrder({
    customer_id: customerId,
    items: items,
  });

  // Store subscription metadata
  const subscription: Subscription = {
    id: uuidv4(),
    customer_id: customerId,
    items: items,
    frequency: frequency as any,
    next_order_date: calculateNextOrderDate(frequency),
    status: 'active',
  };

  console.log(`Subscription created: ${subscription.id}`);
  console.log(`First order: ${order.id}`);
  console.log(`Next order date: ${subscription.next_order_date}`);

  return subscription;
}

function calculateNextOrderDate(frequency: string): string {
  const now = new Date();

  switch (frequency) {
    case 'weekly':
      now.setDate(now.getDate() + 7);
      break;
    case 'monthly':
      now.setMonth(now.getMonth() + 1);
      break;
    case 'quarterly':
      now.setMonth(now.getMonth() + 3);
      break;
  }

  return now.toISOString();
}
```

### Process Subscription Renewal

```typescript
async function processSubscriptionRenewal(
  client: StateSetClient,
  subscription: Subscription
): Promise<void> {
  if (subscription.status !== 'active') {
    console.log(`Subscription ${subscription.id} is not active, skipping`);
    return;
  }

  try {
    // Create new order
    const order = await client.createOrder({
      customer_id: subscription.customer_id,
      items: subscription.items,
    });

    console.log(`Subscription renewal order created: ${order.id}`);

    // Update next order date
    subscription.next_order_date = calculateNextOrderDate(subscription.frequency);

    // Send notification to customer
    console.log(`Sending confirmation email to customer ${subscription.customer_id}`);

  } catch (error) {
    console.error(`Failed to process subscription renewal:`, error);

    // Implement retry logic or alert admins
    throw error;
  }
}
```

---

## Error Handling and Retry Patterns

Implement robust error handling with exponential backoff.

### TypeScript Retry Helper

```typescript
interface RetryOptions {
  maxRetries: number;
  baseDelay: number;
  maxDelay: number;
  retryableErrors?: string[];
}

async function withRetry<T>(
  fn: () => Promise<T>,
  options: RetryOptions = {
    maxRetries: 3,
    baseDelay: 1000,
    maxDelay: 10000,
    retryableErrors: ['NETWORK_ERROR', 'TIMEOUT', 'SERVICE_UNAVAILABLE'],
  }
): Promise<T> {
  let lastError: Error;

  for (let attempt = 0; attempt <= options.maxRetries; attempt++) {
    try {
      return await fn();
    } catch (error: any) {
      lastError = error;

      // Check if error is retryable
      const isRetryable = options.retryableErrors?.some(
        code => error.message?.includes(code)
      );

      if (!isRetryable || attempt === options.maxRetries) {
        throw error;
      }

      // Calculate delay with exponential backoff
      const delay = Math.min(
        options.baseDelay * Math.pow(2, attempt),
        options.maxDelay
      );

      console.log(
        `Attempt ${attempt + 1} failed, retrying in ${delay}ms...`,
        error.message
      );

      await new Promise(resolve => setTimeout(resolve, delay));
    }
  }

  throw lastError!;
}
```

### Usage Example

```typescript
async function createOrderWithRetry(
  client: StateSetClient,
  orderData: any
): Promise<Order> {
  return withRetry(
    () => client.createOrder(orderData),
    {
      maxRetries: 3,
      baseDelay: 1000,
      maxDelay: 10000,
      retryableErrors: ['NETWORK_ERROR', 'TIMEOUT'],
    }
  );
}

// Usage
try {
  const order = await createOrderWithRetry(client, {
    customer_id: 'customer-uuid',
    items: [/* items */],
  });
  console.log('Order created:', order.id);
} catch (error) {
  console.error('Failed to create order after retries:', error);
}
```

---

## Idempotency Best Practices

Use idempotency keys to prevent duplicate operations.

```typescript
import { v4 as uuidv4 } from 'uuid';

async function processPaymentIdempotent(
  client: StateSetClient,
  orderId: string,
  amount: number
): Promise<any> {
  // Generate idempotency key based on order ID
  const idempotencyKey = uuidv4();

  // Store the key for potential retries
  console.log(`Processing payment with idempotency key: ${idempotencyKey}`);

  // Make request with idempotency key
  const response = await client.client.post('/payments', {
    order_id: orderId,
    amount: amount,
  }, {
    headers: {
      'Idempotency-Key': idempotencyKey,
    },
  });

  return response.data;
}
```

---

## Webhooks Integration

Handle webhook events from the StateSet API.

### Express.js Webhook Handler

```typescript
import express from 'express';
import crypto from 'crypto';

const app = express();

// Verify webhook signature
function verifyWebhookSignature(
  payload: string,
  signature: string,
  secret: string
): boolean {
  const hmac = crypto.createHmac('sha256', secret);
  const digest = hmac.update(payload).digest('hex');
  return crypto.timingSafeEqual(
    Buffer.from(signature),
    Buffer.from(digest)
  );
}

app.post('/webhooks/stateset', express.raw({ type: 'application/json' }), (req, res) => {
  const signature = req.headers['x-stateset-signature'] as string;
  const webhookSecret = process.env.STATESET_WEBHOOK_SECRET!;

  // Verify signature
  if (!verifyWebhookSignature(req.body.toString(), signature, webhookSecret)) {
    return res.status(401).send('Invalid signature');
  }

  const event = JSON.parse(req.body.toString());

  console.log('Received webhook event:', event.type);

  // Handle different event types
  switch (event.type) {
    case 'order.created':
      handleOrderCreated(event.data);
      break;
    case 'order.updated':
      handleOrderUpdated(event.data);
      break;
    case 'payment.succeeded':
      handlePaymentSucceeded(event.data);
      break;
    case 'shipment.delivered':
      handleShipmentDelivered(event.data);
      break;
    default:
      console.log('Unhandled event type:', event.type);
  }

  res.status(200).send('OK');
});

async function handleOrderCreated(order: any) {
  console.log('Order created:', order.id);
  // Your business logic here
}

async function handleOrderUpdated(order: any) {
  console.log('Order updated:', order.id, 'Status:', order.status);
  // Your business logic here
}

async function handlePaymentSucceeded(payment: any) {
  console.log('Payment succeeded:', payment.id);
  // Your business logic here
}

async function handleShipmentDelivered(shipment: any) {
  console.log('Shipment delivered:', shipment.id);
  // Your business logic here
}
```

---

## Batch Operations

Process multiple operations efficiently.

```typescript
async function bulkUpdateInventory(
  client: StateSetClient,
  adjustments: Array<{ sku: string; quantity: number; reason: string }>
): Promise<void> {
  const results = await Promise.allSettled(
    adjustments.map(async ({ sku, quantity, reason }) => {
      // Find inventory item
      const inventory = await client.listInventory({ sku });
      const item = inventory.data[0];

      if (!item) {
        throw new Error(`Inventory not found for SKU: ${sku}`);
      }

      // Adjust inventory
      return client.adjustInventory(item.id, quantity, reason);
    })
  );

  // Log results
  const succeeded = results.filter(r => r.status === 'fulfilled').length;
  const failed = results.filter(r => r.status === 'rejected').length;

  console.log(`Bulk inventory update: ${succeeded} succeeded, ${failed} failed`);

  // Log failures
  results.forEach((result, index) => {
    if (result.status === 'rejected') {
      console.error(
        `Failed to update ${adjustments[index].sku}:`,
        result.reason
      );
    }
  });
}
```

---

## Additional Resources

- [Main README](../README.md) - Getting started guide
- [API Overview](../API_OVERVIEW.md) - Complete API reference
- [Integration Guide](../docs/INTEGRATION_GUIDE.md) - Production integration patterns
- [Best Practices](../docs/BEST_PRACTICES.md) - API usage best practices

For more examples and support, visit the [GitHub repository](https://github.com/stateset/stateset-api).
