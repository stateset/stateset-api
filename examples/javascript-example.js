/**
 * StateSet API - JavaScript/Node.js Example
 *
 * This example demonstrates common workflows using the StateSet API:
 * - Authentication
 * - Creating orders
 * - Managing inventory
 * - Processing returns
 * - Tracking shipments
 *
 * Install dependencies: npm install axios
 */

const axios = require('axios');

// Configuration
const BASE_URL = 'http://localhost:8080/api/v1';
const config = {
  email: 'admin@stateset.com',
  password: 'your-password'
};

// Create an API client
class StateSetClient {
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
    this.accessToken = null;
    this.refreshToken = null;
  }

  // Helper method for authenticated requests
  async request(method, endpoint, data = null, headers = {}) {
    try {
      const config = {
        method,
        url: `${this.baseUrl}${endpoint}`,
        headers: {
          'Content-Type': 'application/json',
          ...headers
        }
      };

      if (this.accessToken) {
        config.headers['Authorization'] = `Bearer ${this.accessToken}`;
      }

      if (data) {
        config.data = data;
      }

      const response = await axios(config);
      return response.data;
    } catch (error) {
      if (error.response) {
        console.error('API Error:', error.response.data);
        throw new Error(error.response.data.message || 'API request failed');
      }
      throw error;
    }
  }

  // Authentication
  async login(email, password) {
    const response = await this.request('POST', '/auth/login', {
      email,
      password
    });

    this.accessToken = response.data.access_token;
    this.refreshToken = response.data.refresh_token;

    console.log('✓ Logged in successfully');
    return response.data;
  }

  async createApiKey(name, permissions) {
    const response = await this.request('POST', '/auth/api-keys', {
      name,
      permissions,
      expires_at: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString()
    });

    console.log(`✓ API Key created: ${response.data.key}`);
    return response.data;
  }

  // Orders
  async createOrder(orderData) {
    const response = await this.request('POST', '/orders', orderData);
    console.log(`✓ Order created: ${response.data.order_number || response.data.id}`);
    return response.data;
  }

  async listOrders(filters = {}) {
    const params = new URLSearchParams({
      page: filters.page || 1,
      limit: filters.limit || 20,
      ...(filters.status && { status: filters.status }),
      ...(filters.customer_id && { customer_id: filters.customer_id })
    });

    const response = await this.request('GET', `/orders?${params}`);
    console.log(`✓ Found ${response.data.total || response.data.length} orders`);
    return response.data;
  }

  async getOrder(orderId) {
    const response = await this.request('GET', `/orders/${orderId}`);
    return response.data;
  }

  async updateOrderStatus(orderId, status, notes = '') {
    const response = await this.request('PUT', `/orders/${orderId}/status`, {
      status,
      notes
    });
    console.log(`✓ Order status updated to: ${status}`);
    return response.data;
  }

  async cancelOrder(orderId, reason) {
    const response = await this.request('POST', `/orders/${orderId}/cancel`, {
      reason,
      refund: true
    });
    console.log('✓ Order cancelled');
    return response.data;
  }

  // Inventory
  async listInventory(filters = {}) {
    const params = new URLSearchParams({
      page: filters.page || 1,
      limit: filters.limit || 20,
      ...(filters.product_id && { product_id: filters.product_id }),
      ...(filters.location_id && { location_id: filters.location_id })
    });

    const response = await this.request('GET', `/inventory?${params}`);
    console.log(`✓ Found ${response.data.total || response.data.length} inventory items`);
    return response.data;
  }

  async getLowStock() {
    const response = await this.request('GET', '/inventory/low-stock');
    console.log(`✓ Found ${response.data.length} low stock items`);
    return response.data;
  }

  async reserveInventory(inventoryId, quantity, orderId) {
    const response = await this.request('POST', `/inventory/${inventoryId}/reserve`, {
      quantity,
      order_id: orderId
    });
    console.log(`✓ Reserved ${quantity} units`);
    return response.data;
  }

  async releaseInventory(inventoryId, quantity, reason) {
    const response = await this.request('POST', `/inventory/${inventoryId}/release`, {
      quantity,
      reason
    });
    console.log(`✓ Released ${quantity} units`);
    return response.data;
  }

  // Returns
  async createReturn(returnData) {
    const response = await this.request('POST', '/returns', returnData);
    console.log(`✓ Return created: ${response.data.rma_number || response.data.id}`);
    return response.data;
  }

  async approveReturn(returnId, refundAmount, notes = '') {
    const response = await this.request('POST', `/returns/${returnId}/approve`, {
      refund_amount: refundAmount,
      notes
    });
    console.log('✓ Return approved');
    return response.data;
  }

  async restockReturn(returnId, locationId) {
    const response = await this.request('POST', `/returns/${returnId}/restock`, {
      location_id: locationId,
      condition: 'good'
    });
    console.log('✓ Items restocked');
    return response.data;
  }

  // Shipments
  async createShipment(shipmentData) {
    const response = await this.request('POST', '/shipments', shipmentData);
    console.log(`✓ Shipment created: ${response.data.id}`);
    return response.data;
  }

  async markShipped(shipmentId, trackingNumber) {
    const response = await this.request('POST', `/shipments/${shipmentId}/ship`, {
      tracking_number: trackingNumber,
      shipped_at: new Date().toISOString()
    });
    console.log(`✓ Marked as shipped with tracking: ${trackingNumber}`);
    return response.data;
  }

  async trackShipment(trackingNumber) {
    const response = await this.request('GET', `/shipments/track/${trackingNumber}`);
    console.log(`✓ Shipment status: ${response.data.status}`);
    return response.data;
  }

  // Payments
  async processPayment(paymentData) {
    const idempotencyKey = this.generateUUID();
    const response = await this.request('POST', '/payments', paymentData, {
      'Idempotency-Key': idempotencyKey
    });
    console.log(`✓ Payment processed: ${response.data.transaction_id}`);
    return response.data;
  }

  async refundPayment(paymentId, amount, reason) {
    const response = await this.request('POST', '/payments/refund', {
      payment_id: paymentId,
      amount,
      reason
    });
    console.log(`✓ Refund processed: $${amount}`);
    return response.data;
  }

  // Analytics
  async getDashboard() {
    const response = await this.request('GET', '/analytics/dashboard');
    console.log('✓ Dashboard metrics retrieved');
    return response.data;
  }

  async getSalesTrends(startDate, endDate, interval = 'month') {
    const params = new URLSearchParams({
      start_date: startDate,
      end_date: endDate,
      interval
    });

    const response = await this.request('GET', `/analytics/sales/trends?${params}`);
    console.log('✓ Sales trends retrieved');
    return response.data;
  }

  // Utilities
  generateUUID() {
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, function(c) {
      const r = Math.random() * 16 | 0;
      const v = c === 'x' ? r : (r & 0x3 | 0x8);
      return v.toString(16);
    });
  }
}

// Example usage and workflows
async function main() {
  const client = new StateSetClient(BASE_URL);

  try {
    console.log('\n=== StateSet API Example ===\n');

    // 1. Login
    console.log('1. Authentication');
    await client.login(config.email, config.password);

    // 2. List orders
    console.log('\n2. List Orders');
    const orders = await client.listOrders({ status: 'pending', limit: 5 });

    // 3. Check low stock items
    console.log('\n3. Check Low Stock');
    const lowStock = await client.getLowStock();

    // 4. Create a new order
    console.log('\n4. Create Order');
    const customerId = '550e8400-e29b-41d4-a716-446655440001'; // Replace with actual customer ID
    const productId = '550e8400-e29b-41d4-a716-446655440002'; // Replace with actual product ID

    const newOrder = await client.createOrder({
      customer_id: customerId,
      status: 'pending',
      total_amount: 199.98,
      currency: 'USD',
      items: [{
        product_id: productId,
        sku: 'WIDGET-001',
        quantity: 2,
        unit_price: 99.99,
        name: 'Premium Widget'
      }],
      shipping_address: {
        street: '123 Main St',
        city: 'San Francisco',
        state: 'CA',
        postal_code: '94105',
        country: 'US'
      }
    });

    const orderId = newOrder.id;

    // 5. Update order status
    console.log('\n5. Update Order Status');
    await client.updateOrderStatus(orderId, 'processing', 'Payment confirmed');

    // 6. Create shipment
    console.log('\n6. Create Shipment');
    const shipment = await client.createShipment({
      order_id: orderId,
      carrier: 'UPS',
      service_level: 'ground',
      items: [{
        order_item_id: newOrder.items[0].id,
        quantity: 2
      }]
    });

    // 7. Mark as shipped
    console.log('\n7. Mark as Shipped');
    const trackingNumber = '1Z999AA10123456784';
    await client.markShipped(shipment.id, trackingNumber);

    // 8. Track shipment
    console.log('\n8. Track Shipment');
    await client.trackShipment(trackingNumber);

    // 9. Get analytics
    console.log('\n9. Dashboard Metrics');
    const dashboard = await client.getDashboard();
    console.log('Dashboard:', JSON.stringify(dashboard, null, 2));

    console.log('\n=== Example completed successfully! ===\n');

  } catch (error) {
    console.error('\n❌ Error:', error.message);
    process.exit(1);
  }
}

// Run if executed directly
if (require.main === module) {
  main();
}

// Export for use as a module
module.exports = StateSetClient;
