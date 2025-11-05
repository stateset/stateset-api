# StateSet API - Use Cases & Implementation Guides

## Table of Contents

- [E-Commerce Store](#e-commerce-store)
- [Omnichannel Retail](#omnichannel-retail)
- [Manufacturing & Production](#manufacturing--production)
- [Subscription Box Service](#subscription-box-service)
- [B2B Wholesale](#b2b-wholesale)
- [Marketplace Platform](#marketplace-platform)
- [AI-Powered Shopping](#ai-powered-shopping)
- [Crypto Commerce](#crypto-commerce)

---

## E-Commerce Store

### Scenario
A direct-to-consumer brand selling products online with standard e-commerce operations.

### Implementation Flow

#### 1. Product Catalog Setup

```bash
# Create products
curl -X POST http://localhost:8080/api/v1/products \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Organic Cotton T-Shirt",
    "sku": "SHIRT-ORGANIC-001",
    "description": "Comfortable organic cotton t-shirt",
    "price": 29.99,
    "currency": "USD",
    "category": "Apparel",
    "inventory_quantity": 100,
    "images": ["https://cdn.example.com/shirt-001.jpg"],
    "attributes": {
      "material": "100% Organic Cotton",
      "care": "Machine wash cold"
    }
  }'

# Create variants (sizes, colors)
curl -X POST http://localhost:8080/api/v1/products/$PRODUCT_ID/variants \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "sku": "SHIRT-ORGANIC-001-BLK-M",
    "name": "Organic Cotton T-Shirt - Black - Medium",
    "price": 29.99,
    "options": {
      "color": "Black",
      "size": "Medium"
    },
    "inventory_quantity": 25
  }'
```

#### 2. Customer Registration & Shopping

```javascript
// Customer registration
const customer = await axios.post('/api/v1/customers', {
  email: 'customer@example.com',
  password: 'SecurePass123!',
  first_name: 'Jane',
  last_name: 'Smith'
});

// Create shopping cart
const cart = await axios.post('/api/v1/carts', {
  customer_id: customer.data.data.id
});

// Add items to cart
await axios.post(`/api/v1/carts/${cart.data.data.id}/items`, {
  product_id: 'product-uuid',
  variant_id: 'variant-uuid',
  quantity: 2
});

// Update quantity
await axios.put(`/api/v1/carts/${cart.id}/items/${item.id}`, {
  quantity: 3
});
```

#### 3. Checkout Process

```javascript
// Start checkout
const checkout = await axios.post('/api/v1/checkout', {
  cart_id: cart.id
});

// Set shipping address
await axios.put(`/api/v1/checkout/${checkout.id}/shipping-address`, {
  first_name: 'Jane',
  last_name: 'Smith',
  street: '123 Main St',
  city: 'San Francisco',
  state: 'CA',
  postal_code: '94105',
  country: 'US',
  phone: '+1-555-0123'
});

// Get shipping options
const shippingOptions = await axios.get(
  `/api/v1/checkout/${checkout.id}/shipping-methods`
);

// Select shipping method
await axios.put(`/api/v1/checkout/${checkout.id}/shipping-method`, {
  method: 'standard',
  carrier: 'UPS',
  cost: 5.99
});

// Complete checkout with payment
const order = await axios.post(`/api/v1/checkout/${checkout.id}/complete`, {
  payment_method: 'credit_card',
  payment_details: {
    token: 'stripe-token-abc123' // From Stripe.js
  }
}, {
  headers: {
    'Idempotency-Key': crypto.randomUUID()
  }
});

console.log(`Order created: ${order.data.data.order_number}`);
```

#### 4. Order Fulfillment

```javascript
// Warehouse receives order notification
// Reserve inventory
await axios.post(`/api/v1/inventory/${inventoryId}/reserve`, {
  quantity: 2,
  order_id: order.id,
  notes: 'Reserved for order ' + order.order_number
});

// Create pick list (internal process)
// Pick and pack items
// Update order status
await axios.put(`/api/v1/orders/${order.id}/status`, {
  status: 'processing',
  notes: 'Items picked and packed'
});

// Create shipment
const shipment = await axios.post('/api/v1/shipments', {
  order_id: order.id,
  carrier: 'UPS',
  service_level: 'ground',
  items: order.items.map(item => ({
    order_item_id: item.id,
    quantity: item.quantity
  }))
});

// Mark as shipped with tracking
await axios.post(`/api/v1/shipments/${shipment.id}/ship`, {
  tracking_number: '1Z999AA10123456784',
  shipped_at: new Date().toISOString()
});

// Order status automatically updated to 'shipped'
// Customer receives email notification with tracking
```

#### 5. Returns Processing

```javascript
// Customer initiates return
const returnRequest = await axios.post('/api/v1/returns', {
  order_id: order.id,
  items: [{
    order_item_id: 'item-uuid',
    quantity: 1,
    reason: 'wrong_size',
    description: 'Ordered medium, need large'
  }],
  customer_notes: 'Size was too small'
});

// Customer receives RMA number
console.log(`RMA Number: ${returnRequest.data.data.rma_number}`);

// Staff reviews and approves
await axios.post(`/api/v1/returns/${returnRequest.id}/approve`, {
  refund_amount: 29.99,
  notes: 'Approved for full refund'
});

// Customer ships item back
// Warehouse receives and inspects
// Restock inventory
await axios.post(`/api/v1/returns/${returnRequest.id}/restock`, {
  location_id: 'warehouse-uuid',
  condition: 'good'
});

// Process refund
await axios.post('/api/v1/payments/refund', {
  payment_id: 'payment-uuid',
  amount: 29.99,
  reason: 'Customer return'
});
```

### Key Metrics to Track

```javascript
// Dashboard overview
const metrics = await axios.get('/api/v1/analytics/dashboard');

console.log('Today\'s Performance:');
console.log(`Orders: ${metrics.data.orders_today}`);
console.log(`Revenue: $${metrics.data.revenue_today}`);
console.log(`Average Order Value: $${metrics.data.average_order_value}`);
console.log(`Pending Shipments: ${metrics.data.pending_shipments}`);
console.log(`Low Stock Items: ${metrics.data.low_stock_items}`);
```

---

## Omnichannel Retail

### Scenario
A retailer with physical stores and online presence, requiring unified inventory and order management.

### Implementation Strategy

#### 1. Multi-Location Inventory Setup

```javascript
// Set up warehouse
const warehouse = await axios.post('/api/v1/facilities', {
  name: 'Main Warehouse',
  type: 'warehouse',
  address: {
    street: '1000 Industrial Blvd',
    city: 'Oakland',
    state: 'CA',
    postal_code: '94621',
    country: 'US'
  }
});

// Set up retail stores
const store1 = await axios.post('/api/v1/facilities', {
  name: 'San Francisco Store',
  type: 'retail',
  address: {
    street: '100 Market St',
    city: 'San Francisco',
    state: 'CA',
    postal_code: '94105',
    country: 'US'
  }
});

// Distribute inventory across locations
await axios.post('/api/v1/inventory', {
  product_id: 'product-uuid',
  location_id: warehouse.id,
  quantity_on_hand: 500,
  reorder_point: 100
});

await axios.post('/api/v1/inventory', {
  product_id: 'product-uuid',
  location_id: store1.id,
  quantity_on_hand: 50,
  reorder_point: 10
});
```

#### 2. Buy Online, Pick Up In Store (BOPIS)

```javascript
// Customer places order online, selects store pickup
const order = await axios.post('/api/v1/orders', {
  customer_id: 'customer-uuid',
  fulfillment_method: 'store_pickup',
  pickup_location_id: store1.id,
  items: [{
    product_id: 'product-uuid',
    quantity: 1,
    unit_price: 49.99
  }]
});

// Reserve inventory at selected store
await axios.post(`/api/v1/inventory/${storeInventoryId}/reserve`, {
  quantity: 1,
  order_id: order.id,
  location_id: store1.id
});

// Store staff prepares order
await axios.put(`/api/v1/orders/${order.id}/status`, {
  status: 'ready_for_pickup',
  notes: 'Order ready at customer service desk'
});

// Customer notified via SMS/email
// Customer picks up, staff confirms
await axios.post(`/api/v1/orders/${order.id}/pickup-complete`, {
  picked_up_by: 'Jane Smith',
  picked_up_at: new Date().toISOString(),
  staff_id: 'staff-uuid'
});
```

#### 3. Inventory Transfer Between Locations

```javascript
// Store running low, transfer from warehouse
const transfer = await axios.post('/api/v1/inventory/transfer', {
  product_id: 'product-uuid',
  from_location_id: warehouse.id,
  to_location_id: store1.id,
  quantity: 25,
  reason: 'Store replenishment',
  expected_date: '2025-11-10'
});

// Track transfer status
const status = await axios.get(`/api/v1/inventory/transfers/${transfer.id}`);
```

#### 4. Unified Customer View

```javascript
// Get customer's complete history across all channels
const customer = await axios.get(`/api/v1/customers/${customerId}`);
const orders = await axios.get(`/api/v1/customers/${customerId}/orders`, {
  params: {
    include_store_purchases: true,
    include_online_purchases: true
  }
});

console.log('Customer Lifetime Value:', customer.data.lifetime_value);
console.log('Total Orders:', orders.data.total);
console.log('Online Orders:', orders.data.items.filter(o => o.channel === 'online').length);
console.log('In-Store Orders:', orders.data.items.filter(o => o.channel === 'store').length);
```

---

## Manufacturing & Production

### Scenario
A manufacturer producing custom products with bill of materials (BOM) tracking.

### Implementation Flow

#### 1. Create Bill of Materials

```javascript
// Define product BOM
const bom = await axios.post('/api/v1/manufacturing/boms', {
  product_id: 'finished-product-uuid',
  name: 'Custom Widget - Standard',
  version: '1.0',
  components: [
    {
      part_id: 'component-a-uuid',
      quantity: 2,
      unit: 'pieces'
    },
    {
      part_id: 'component-b-uuid',
      quantity: 1,
      unit: 'pieces'
    },
    {
      part_id: 'raw-material-c-uuid',
      quantity: 0.5,
      unit: 'kg'
    }
  ],
  labor_hours: 2.5,
  overhead_cost: 15.00
});
```

#### 2. Create Work Order

```javascript
// Manufacturing order received
const workOrder = await axios.post('/api/v1/work-orders', {
  bom_id: bom.id,
  quantity: 100,
  due_date: '2025-12-01',
  priority: 'high',
  notes: 'Rush order for key customer'
});

// Assign to production line
await axios.post(`/api/v1/work-orders/${workOrder.id}/assign`, {
  assigned_to: 'production-line-1',
  start_date: '2025-11-06'
});

// Reserve raw materials
for (const component of bom.components) {
  await axios.post(`/api/v1/inventory/${component.inventory_id}/reserve`, {
    quantity: component.quantity * workOrder.quantity,
    work_order_id: workOrder.id
  });
}
```

#### 3. Track Production

```javascript
// Update work order progress
await axios.put(`/api/v1/work-orders/${workOrder.id}`, {
  quantity_completed: 50,
  status: 'in_progress',
  notes: 'First batch of 50 completed'
});

// Record quality check
await axios.post(`/api/v1/work-orders/${workOrder.id}/quality-check`, {
  passed: 48,
  failed: 2,
  failure_reasons: ['Minor defect in component assembly'],
  inspector_id: 'inspector-uuid'
});

// Complete work order
await axios.post(`/api/v1/work-orders/${workOrder.id}/complete`, {
  quantity_completed: 98,
  quantity_rejected: 2,
  completed_at: new Date().toISOString(),
  notes: '98 units passed final inspection'
});

// Add to finished goods inventory
await axios.post('/api/v1/inventory/adjust', {
  product_id: 'finished-product-uuid',
  location_id: 'warehouse-uuid',
  adjustment: 98,
  reason: 'Production completion',
  work_order_id: workOrder.id
});
```

---

## Subscription Box Service

### Scenario
A monthly subscription box service requiring recurring orders and inventory planning.

### Implementation Strategy

#### 1. Create Subscription Plans

```javascript
// Define subscription tier
const subscriptionPlan = await axios.post('/api/v1/subscriptions/plans', {
  name: 'Premium Box',
  description: 'Monthly curated selection of premium products',
  price: 49.99,
  billing_cycle: 'monthly',
  items_per_box: 5
});
```

#### 2. Customer Subscription Management

```javascript
// Customer subscribes
const subscription = await axios.post('/api/v1/subscriptions', {
  customer_id: 'customer-uuid',
  plan_id: subscriptionPlan.id,
  shipping_address: {...},
  payment_method: 'card-token',
  start_date: '2025-11-01'
});

// Pause subscription
await axios.post(`/api/v1/subscriptions/${subscription.id}/pause`, {
  pause_until: '2025-12-01',
  reason: 'Customer traveling'
});

// Resume subscription
await axios.post(`/api/v1/subscriptions/${subscription.id}/resume`);

// Cancel subscription
await axios.post(`/api/v1/subscriptions/${subscription.id}/cancel`, {
  reason: 'Customer request',
  process_remaining_boxes: false
});
```

#### 3. Automated Monthly Fulfillment

```javascript
// Run monthly (via scheduled job)
async function processMonthlySubscriptions() {
  // Get active subscriptions due for renewal
  const dueSubscriptions = await axios.get('/api/v1/subscriptions', {
    params: {
      status: 'active',
      next_billing_date: new Date().toISOString().split('T')[0]
    }
  });

  for (const subscription of dueSubscriptions.data.items) {
    try {
      // Charge customer
      const payment = await axios.post('/api/v1/payments', {
        customer_id: subscription.customer_id,
        amount: subscription.plan.price,
        payment_method_id: subscription.payment_method_id,
        description: `Monthly box - ${new Date().toISOString().substring(0, 7)}`
      }, {
        headers: {
          'Idempotency-Key': `sub-${subscription.id}-${Date.now()}`
        }
      });

      // Curate box contents (your business logic)
      const boxItems = await curateBoxItems(subscription);

      // Create order
      const order = await axios.post('/api/v1/orders', {
        customer_id: subscription.customer_id,
        subscription_id: subscription.id,
        items: boxItems,
        shipping_address: subscription.shipping_address,
        total_amount: subscription.plan.price,
        notes: 'Subscription box - November 2025'
      });

      // Reserve inventory
      for (const item of boxItems) {
        await axios.post(`/api/v1/inventory/${item.inventory_id}/reserve`, {
          quantity: item.quantity,
          order_id: order.id
        });
      }

      // Queue for fulfillment
      await axios.post('/api/v1/fulfillment/queue', {
        order_id: order.id,
        priority: 'normal'
      });

    } catch (error) {
      // Handle payment failure
      await axios.post(`/api/v1/subscriptions/${subscription.id}/payment-failed`, {
        error: error.message,
        retry_date: calculateRetryDate()
      });
    }
  }
}
```

---

## B2B Wholesale

### Scenario
B2B platform with bulk ordering, custom pricing, and purchase orders.

### Implementation

#### 1. Business Customer Setup

```javascript
// Create business account with custom pricing
const businessCustomer = await axios.post('/api/v1/customers', {
  type: 'business',
  company_name: 'Acme Retail Inc.',
  tax_id: '12-3456789',
  email: 'orders@acmeretail.com',
  credit_limit: 50000.00,
  payment_terms: 'net_30',
  custom_pricing_tier: 'volume_discount_15'
});

// Add multiple shipping addresses
await axios.post(`/api/v1/customers/${businessCustomer.id}/addresses`, {
  name: 'Warehouse',
  street: '500 Distribution Way',
  city: 'Los Angeles',
  state: 'CA',
  postal_code: '90001',
  country: 'US',
  is_default: true
});

await axios.post(`/api/v1/customers/${businessCustomer.id}/addresses`, {
  name: 'Store #1',
  street: '100 Retail Plaza',
  city: 'San Diego',
  state: 'CA',
  postal_code: '92101',
  country: 'US'
});
```

#### 2. Bulk Order Processing

```javascript
// Customer places bulk order
const purchaseOrder = await axios.post('/api/v1/purchase-orders', {
  customer_id: businessCustomer.id,
  po_number: 'PO-2025-1234', // Customer's PO number
  items: [
    {
      product_id: 'product-a-uuid',
      quantity: 500,
      unit_price: 19.99, // Wholesale price
      discount_percent: 15
    },
    {
      product_id: 'product-b-uuid',
      quantity: 1000,
      unit_price: 9.99,
      discount_percent: 15
    }
  ],
  shipping_address_id: 'warehouse-address-uuid',
  requested_delivery_date: '2025-12-01',
  payment_terms: 'net_30',
  notes: 'Partial shipments acceptable'
});

// Approve purchase order
await axios.post(`/api/v1/purchase-orders/${purchaseOrder.id}/approve`, {
  approved_by: 'sales-manager-uuid',
  credit_check_passed: true
});

// Convert to sales order
const order = await axios.post(`/api/v1/purchase-orders/${purchaseOrder.id}/convert`, {
  split_shipments: true, // Allow partial fulfillment
  priority: 'high'
});
```

#### 3. Invoice Generation

```javascript
// Generate invoice on shipment
const invoice = await axios.post('/api/v1/invoices', {
  order_id: order.id,
  customer_id: businessCustomer.id,
  due_date: calculateDueDate('net_30'),
  line_items: order.items.map(item => ({
    description: item.name,
    quantity: item.quantity,
    unit_price: item.unit_price,
    discount: item.discount_amount,
    total: item.total_price
  })),
  subtotal: order.subtotal,
  tax: order.tax_amount,
  total: order.total_amount,
  terms: 'Net 30 days'
});

// Send invoice
await axios.post(`/api/v1/invoices/${invoice.id}/send`, {
  email: businessCustomer.email,
  cc: businessCustomer.accounts_payable_email
});
```

---

## AI-Powered Shopping

### Scenario
Enable shopping through ChatGPT using the Agentic Commerce Protocol.

### Setup Guide

#### 1. Configure Agentic Server

```bash
# Start agentic server
cd agentic_server
cargo run

# Server runs on port 8080 by default
# Endpoints:
# - POST /checkout_sessions
# - GET /checkout_sessions/{id}
# - POST /checkout_sessions/{id}/complete
# - POST /agentic_commerce/delegate_payment
```

#### 2. Customer Shopping Flow

**Customer**: "I want to buy running shoes"

**ChatGPT calls**:
```http
POST /checkout_sessions
Content-Type: application/json

{
  "merchant_session_id": "session-uuid-from-chatgpt",
  "line_items": [
    {
      "product_id": "running-shoes-uuid",
      "title": "Nike Air Zoom Pegasus",
      "quantity": 1,
      "price": {
        "amount": 12000,
        "currency": "USD"
      }
    }
  ]
}
```

**Customer**: "Ship to 123 Main St, San Francisco, CA 94105"

**ChatGPT calls**:
```http
POST /checkout_sessions/{session_id}
Content-Type: application/json

{
  "customer": {
    "billing_address": {
      "address_line_1": "123 Main St",
      "locality": "San Francisco",
      "region": "CA",
      "postal_code": "94105",
      "country": "US"
    },
    "shipping_address": {
      "address_line_1": "123 Main St",
      "locality": "San Francisco",
      "region": "CA",
      "postal_code": "94105",
      "country": "US"
    }
  }
}
```

**Response includes**:
```json
{
  "status": "not_ready_for_payment",
  "fulfillment": {
    "choices": [
      {
        "id": "standard",
        "title": "Standard Shipping (5-7 days)",
        "price": { "amount": 599, "currency": "USD" }
      },
      {
        "id": "express",
        "title": "Express Shipping (2-3 days)",
        "price": { "amount": 1299, "currency": "USD" }
      }
    ]
  },
  "totals": {
    "subtotal": { "amount": 12000, "currency": "USD" },
    "tax": { "amount": 1080, "currency": "USD" },
    "shipping": { "amount": 0, "currency": "USD" },
    "grand_total": { "amount": 13080, "currency": "USD" }
  }
}
```

**Customer**: "Use standard shipping"

**ChatGPT calls**:
```http
POST /checkout_sessions/{session_id}
Content-Type: application/json

{
  "fulfillment": {
    "selected_choice_id": "standard"
  }
}
```

**Customer authorizes payment**

**ChatGPT calls**:
```http
POST /checkout_sessions/{session_id}/complete
Content-Type: application/json

{
  "payment": {
    "type": "card_vault_token",
    "vault_token": "tok_visa_4242"
  }
}
```

**Order created! Customer receives confirmation in chat.**

---

## Crypto Commerce

### Scenario
Accept cryptocurrency payments using StablePay integration.

### Implementation

#### 1. Configure StablePay

```javascript
// Initialize StablePay service
const stablePay = {
  api_key: process.env.STABLEPAY_API_KEY,
  webhook_secret: process.env.STABLEPAY_WEBHOOK_SECRET,
  accepted_currencies: ['USDC', 'USDT']
};
```

#### 2. Create Crypto Payment

```javascript
// Customer selects crypto payment at checkout
const cryptoPayment = await axios.post('/api/v1/payments/crypto', {
  order_id: order.id,
  amount: order.total_amount,
  currency: 'USD',
  crypto_currency: 'USDC', // or 'USDT'
  customer_wallet_address: '0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb'
});

// Response includes payment address and amount
console.log('Payment Details:');
console.log(`Send ${cryptoPayment.data.crypto_amount} ${cryptoPayment.data.crypto_currency}`);
console.log(`To address: ${cryptoPayment.data.payment_address}`);
console.log(`Network: ${cryptoPayment.data.network}`);
console.log(`Expires at: ${cryptoPayment.data.expires_at}`);
```

#### 3. Handle Payment Webhooks

```javascript
// Webhook endpoint to receive payment confirmations
app.post('/webhooks/stablepay', async (req, res) => {
  // Verify webhook signature
  const signature = req.headers['stablepay-signature'];
  const isValid = verifyStablePaySignature(req.body, signature);

  if (!isValid) {
    return res.status(401).json({ error: 'Invalid signature' });
  }

  const event = req.body;

  switch (event.type) {
    case 'payment.pending':
      // Transaction detected on blockchain
      await axios.put(`/api/v1/payments/${event.payment_id}/status`, {
        status: 'pending',
        transaction_hash: event.transaction_hash,
        confirmations: event.confirmations
      });
      break;

    case 'payment.confirmed':
      // Payment confirmed with sufficient confirmations
      await axios.put(`/api/v1/payments/${event.payment_id}/status`, {
        status: 'completed',
        transaction_hash: event.transaction_hash,
        confirmations: event.confirmations,
        completed_at: new Date().toISOString()
      });

      // Update order status
      await axios.put(`/api/v1/orders/${event.order_id}/status`, {
        status: 'processing',
        notes: 'Crypto payment confirmed'
      });

      // Send confirmation email
      await sendOrderConfirmation(event.order_id);
      break;

    case 'payment.failed':
      // Payment failed or expired
      await axios.put(`/api/v1/payments/${event.payment_id}/status`, {
        status: 'failed',
        error: event.error_message
      });
      break;
  }

  res.json({ received: true });
});
```

#### 4. Crypto Refunds

```javascript
// Process refund in crypto
const refund = await axios.post('/api/v1/payments/crypto/refund', {
  payment_id: 'payment-uuid',
  refund_amount: 99.99,
  refund_currency: 'USDC',
  destination_wallet: '0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb',
  reason: 'Customer return'
});

console.log(`Refund transaction: ${refund.data.transaction_hash}`);
console.log(`Explorer: https://etherscan.io/tx/${refund.data.transaction_hash}`);
```

---

## Best Practices Summary

### 1. Idempotency
Always use idempotency keys for write operations to prevent duplicate charges or orders.

### 2. Error Handling
Implement comprehensive error handling with retry logic and exponential backoff.

### 3. Webhooks
Set up webhooks for asynchronous events (payment confirmations, shipment updates, etc.).

### 4. Inventory Management
Reserve inventory immediately when orders are created to prevent overselling.

### 5. Security
- Never log sensitive data (payment details, passwords)
- Verify webhook signatures
- Use HTTPS for all API calls
- Rotate API keys regularly

### 6. Performance
- Use pagination for large datasets
- Cache frequently accessed data
- Batch operations when possible
- Monitor rate limits

### 7. Testing
- Test error scenarios (payment failures, out of stock, etc.)
- Use test mode for payment processors
- Validate all user inputs
- Test webhook handlers thoroughly

---

For more examples and implementation details, see:
- `/examples` directory
- API documentation at `/swagger-ui`
- CLI tool: `stateset-cli --help`
