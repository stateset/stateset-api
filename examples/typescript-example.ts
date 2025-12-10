/**
 * StateSet API - TypeScript/Node.js Client Example
 *
 * A comprehensive TypeScript client for the StateSet API demonstrating:
 * - Authentication (JWT & API Keys)
 * - Order management
 * - Inventory operations
 * - Shopping cart & checkout
 * - Returns & shipments
 * - Customer management
 * - Analytics
 *
 * Installation:
 * npm install axios uuid @types/node @types/uuid
 *
 * Usage:
 * ts-node typescript-example.ts
 */

import axios, { AxiosInstance, AxiosError } from 'axios';
import { v4 as uuidv4 } from 'uuid';

// ============================================================================
// Type Definitions
// ============================================================================

interface AuthResponse {
  access_token: string;
  refresh_token: string;
  token_type: string;
  expires_in: number;
}

interface Order {
  id: string;
  customer_id: string;
  status: string;
  total_amount: number;
  currency: string;
  items: OrderItem[];
  created_at: string;
  updated_at: string;
}

interface OrderItem {
  product_id: string;
  sku: string;
  quantity: number;
  unit_price: number;
  name: string;
}

interface InventoryItem {
  id: string;
  sku: string;
  location_id: string;
  quantity_on_hand: number;
  quantity_allocated: number;
  quantity_available: number;
}

interface Cart {
  id: string;
  customer_id?: string;
  session_id?: string;
  items: CartItem[];
  subtotal: number;
  tax: number;
  shipping: number;
  total: number;
}

interface CartItem {
  product_id: string;
  sku: string;
  quantity: number;
  price: number;
  name: string;
}

interface Customer {
  id: string;
  email: string;
  first_name: string;
  last_name: string;
  phone?: string;
  created_at: string;
}

interface Shipment {
  id: string;
  order_id: string;
  carrier: string;
  tracking_number?: string;
  status: string;
  shipped_at?: string;
  delivered_at?: string;
}

interface Return {
  id: string;
  order_id: string;
  status: string;
  items: ReturnItem[];
  customer_notes?: string;
}

interface ReturnItem {
  order_item_id: string;
  quantity: number;
  reason: string;
  description?: string;
}

interface PaginatedResponse<T> {
  data: T[];
  pagination: {
    page: number;
    per_page: number;
    total: number;
    total_pages: number;
  };
}

interface ApiError {
  error: {
    code: string;
    message: string;
    status: number;
    details?: Record<string, any>;
  };
}

// ============================================================================
// StateSet API Client
// ============================================================================

class StateSetClient {
  private client: AxiosInstance;
  private accessToken: string | null = null;
  private refreshToken: string | null = null;

  constructor(baseURL: string = 'http://localhost:8080/api/v1') {
    this.client = axios.create({
      baseURL,
      headers: {
        'Content-Type': 'application/json',
      },
      timeout: 30000,
    });

    // Add request interceptor for authentication
    this.client.interceptors.request.use(
      (config) => {
        if (this.accessToken) {
          config.headers.Authorization = `Bearer ${this.accessToken}`;
        }
        return config;
      },
      (error) => Promise.reject(error)
    );

    // Add response interceptor for error handling
    this.client.interceptors.response.use(
      (response) => response,
      async (error: AxiosError<ApiError>) => {
        if (error.response?.status === 401 && this.refreshToken) {
          // Try to refresh the token
          try {
            await this.refreshAccessToken();
            // Retry the original request
            if (error.config) {
              return this.client.request(error.config);
            }
          } catch (refreshError) {
            // Refresh failed, clear tokens
            this.accessToken = null;
            this.refreshToken = null;
          }
        }
        return Promise.reject(error);
      }
    );
  }

  // ==========================================================================
  // Authentication
  // ==========================================================================

  async login(email: string, password: string): Promise<AuthResponse> {
    const response = await this.client.post<AuthResponse>('/auth/login', {
      email,
      password,
    });

    this.accessToken = response.data.access_token;
    this.refreshToken = response.data.refresh_token;

    return response.data;
  }

  async register(email: string, password: string, firstName: string, lastName: string): Promise<AuthResponse> {
    const response = await this.client.post<AuthResponse>('/auth/register', {
      email,
      password,
      first_name: firstName,
      last_name: lastName,
    });

    this.accessToken = response.data.access_token;
    this.refreshToken = response.data.refresh_token;

    return response.data;
  }

  async refreshAccessToken(): Promise<void> {
    if (!this.refreshToken) {
      throw new Error('No refresh token available');
    }

    const response = await this.client.post<AuthResponse>('/auth/refresh', {
      refresh_token: this.refreshToken,
    });

    this.accessToken = response.data.access_token;
  }

  async logout(): Promise<void> {
    await this.client.post('/auth/logout');
    this.accessToken = null;
    this.refreshToken = null;
  }

  async createApiKey(name: string, permissions: string[]): Promise<{ key: string; id: string }> {
    const response = await this.client.post('/auth/api-keys', {
      name,
      permissions,
      expires_at: new Date(Date.now() + 365 * 24 * 60 * 60 * 1000).toISOString(),
    });
    return response.data;
  }

  setApiKey(apiKey: string): void {
    this.client.defaults.headers['X-API-Key'] = apiKey;
  }

  // ==========================================================================
  // Orders
  // ==========================================================================

  async createOrder(data: {
    customer_id: string;
    items: OrderItem[];
    shipping_address?: any;
    billing_address?: any;
  }): Promise<Order> {
    const response = await this.client.post<Order>('/orders', data);
    return response.data;
  }

  async listOrders(params?: {
    page?: number;
    limit?: number;
    status?: string;
    customer_id?: string;
  }): Promise<PaginatedResponse<Order>> {
    const response = await this.client.get<PaginatedResponse<Order>>('/orders', { params });
    return response.data;
  }

  async getOrder(orderId: string): Promise<Order> {
    const response = await this.client.get<Order>(`/orders/${orderId}`);
    return response.data;
  }

  async updateOrderStatus(orderId: string, status: string, notes?: string): Promise<Order> {
    const response = await this.client.put<Order>(`/orders/${orderId}/status`, {
      status,
      notes,
    });
    return response.data;
  }

  async cancelOrder(orderId: string, reason?: string): Promise<Order> {
    const response = await this.client.post<Order>(`/orders/${orderId}/cancel`, {
      reason,
    });
    return response.data;
  }

  async refundOrder(orderId: string, amount: number, reason: string): Promise<any> {
    const response = await this.client.post(`/orders/${orderId}/refund`, {
      amount,
      reason,
      idempotency_key: uuidv4(),
    });
    return response.data;
  }

  // ==========================================================================
  // Inventory
  // ==========================================================================

  async listInventory(params?: {
    page?: number;
    limit?: number;
    location_id?: string;
  }): Promise<PaginatedResponse<InventoryItem>> {
    const response = await this.client.get<PaginatedResponse<InventoryItem>>('/inventory', { params });
    return response.data;
  }

  async getInventoryItem(id: string): Promise<InventoryItem> {
    const response = await this.client.get<InventoryItem>(`/inventory/${id}`);
    return response.data;
  }

  async getLowStockItems(): Promise<InventoryItem[]> {
    const response = await this.client.get<{ data: InventoryItem[] }>('/inventory/low-stock');
    return response.data.data;
  }

  async reserveInventory(id: string, quantity: number, orderId: string): Promise<any> {
    const response = await this.client.post(`/inventory/${id}/reserve`, {
      quantity,
      order_id: orderId,
      expires_at: new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString(), // 24 hours
    });
    return response.data;
  }

  async releaseInventory(reservationId: string): Promise<any> {
    const response = await this.client.post(`/inventory/reservations/${reservationId}/cancel`);
    return response.data;
  }

  async adjustInventory(id: string, quantity: number, reason: string): Promise<InventoryItem> {
    const response = await this.client.post<InventoryItem>(`/inventory/${id}/adjust`, {
      quantity,
      reason,
    });
    return response.data;
  }

  // ==========================================================================
  // Shopping Cart
  // ==========================================================================

  async createCart(customerId?: string): Promise<Cart> {
    const response = await this.client.post<Cart>('/carts', {
      customer_id: customerId,
      session_id: uuidv4(),
    });
    return response.data;
  }

  async getCart(cartId: string): Promise<Cart> {
    const response = await this.client.get<Cart>(`/carts/${cartId}`);
    return response.data;
  }

  async addItemToCart(cartId: string, item: {
    product_id: string;
    sku: string;
    quantity: number;
    price: number;
    name: string;
  }): Promise<Cart> {
    const response = await this.client.post<Cart>(`/carts/${cartId}/items`, item);
    return response.data;
  }

  async updateCartItem(cartId: string, itemId: string, quantity: number): Promise<Cart> {
    const response = await this.client.put<Cart>(`/carts/${cartId}/items/${itemId}`, {
      quantity,
    });
    return response.data;
  }

  async removeCartItem(cartId: string, itemId: string): Promise<Cart> {
    const response = await this.client.delete<Cart>(`/carts/${cartId}/items/${itemId}`);
    return response.data;
  }

  async checkout(cartId: string, data: {
    customer_id: string;
    shipping_address: any;
    billing_address: any;
    payment_method: any;
  }): Promise<Order> {
    const response = await this.client.post<Order>('/checkout', {
      cart_id: cartId,
      ...data,
    });
    return response.data;
  }

  // ==========================================================================
  // Customers
  // ==========================================================================

  async createCustomer(data: {
    email: string;
    first_name: string;
    last_name: string;
    phone?: string;
  }): Promise<Customer> {
    const response = await this.client.post<Customer>('/customers', data);
    return response.data;
  }

  async listCustomers(params?: {
    page?: number;
    limit?: number;
    search?: string;
  }): Promise<PaginatedResponse<Customer>> {
    const response = await this.client.get<PaginatedResponse<Customer>>('/customers', { params });
    return response.data;
  }

  async getCustomer(id: string): Promise<Customer> {
    const response = await this.client.get<Customer>(`/customers/${id}`);
    return response.data;
  }

  async updateCustomer(id: string, data: Partial<Customer>): Promise<Customer> {
    const response = await this.client.put<Customer>(`/customers/${id}`, data);
    return response.data;
  }

  // ==========================================================================
  // Returns
  // ==========================================================================

  async createReturn(data: {
    order_id: string;
    items: ReturnItem[];
    customer_notes?: string;
  }): Promise<Return> {
    const response = await this.client.post<Return>('/returns', data);
    return response.data;
  }

  async listReturns(params?: {
    page?: number;
    limit?: number;
    status?: string;
  }): Promise<PaginatedResponse<Return>> {
    const response = await this.client.get<PaginatedResponse<Return>>('/returns', { params });
    return response.data;
  }

  async getReturn(id: string): Promise<Return> {
    const response = await this.client.get<Return>(`/returns/${id}`);
    return response.data;
  }

  async approveReturn(id: string): Promise<Return> {
    const response = await this.client.post<Return>(`/returns/${id}/approve`);
    return response.data;
  }

  async restockReturn(id: string): Promise<Return> {
    const response = await this.client.post<Return>(`/returns/${id}/restock`);
    return response.data;
  }

  // ==========================================================================
  // Shipments
  // ==========================================================================

  async createShipment(data: {
    order_id: string;
    carrier: string;
    service_level: string;
  }): Promise<Shipment> {
    const response = await this.client.post<Shipment>('/shipments', data);
    return response.data;
  }

  async markAsShipped(shipmentId: string, trackingNumber: string): Promise<Shipment> {
    const response = await this.client.post<Shipment>(`/shipments/${shipmentId}/ship`, {
      tracking_number: trackingNumber,
      shipped_at: new Date().toISOString(),
    });
    return response.data;
  }

  async trackShipment(trackingNumber: string): Promise<any> {
    const response = await this.client.get(`/shipments/track/${trackingNumber}`);
    return response.data;
  }

  // ==========================================================================
  // Analytics
  // ==========================================================================

  async getDashboardMetrics(): Promise<any> {
    const response = await this.client.get('/analytics/dashboard');
    return response.data;
  }

  async getSalesTrends(params?: {
    start_date?: string;
    end_date?: string;
    interval?: string;
  }): Promise<any> {
    const response = await this.client.get('/analytics/sales/trends', { params });
    return response.data;
  }

  async getInventoryAnalytics(): Promise<any> {
    const response = await this.client.get('/analytics/inventory');
    return response.data;
  }

  // ==========================================================================
  // Health & Status
  // ==========================================================================

  async getHealth(): Promise<any> {
    const response = await this.client.get('/health');
    return response.data;
  }

  async getStatus(): Promise<any> {
    const response = await this.client.get('/status');
    return response.data;
  }
}

// ============================================================================
// Example Usage
// ============================================================================

async function main() {
  const client = new StateSetClient('http://localhost:8080/api/v1');

  try {
    console.log('🚀 StateSet API TypeScript Example\n');

    // 1. Authentication
    console.log('1️⃣  Authenticating...');
    await client.login('admin@stateset.com', 'your-password');
    console.log('✅ Logged in successfully\n');

    // 2. Check API health
    console.log('2️⃣  Checking API health...');
    const health = await client.getHealth();
    console.log('✅ API Status:', health.status, '\n');

    // 3. Create a customer
    console.log('3️⃣  Creating customer...');
    const customer = await client.createCustomer({
      email: `test-${Date.now()}@example.com`,
      first_name: 'John',
      last_name: 'Doe',
      phone: '+1-555-0100',
    });
    console.log('✅ Customer created:', customer.id, '\n');

    // 4. Create a shopping cart
    console.log('4️⃣  Creating shopping cart...');
    const cart = await client.createCart(customer.id);
    console.log('✅ Cart created:', cart.id, '\n');

    // 5. Add items to cart
    console.log('5️⃣  Adding items to cart...');
    await client.addItemToCart(cart.id, {
      product_id: uuidv4(), // Replace with actual product ID
      sku: 'WIDGET-001',
      quantity: 2,
      price: 99.99,
      name: 'Premium Widget',
    });
    console.log('✅ Items added to cart\n');

    // 6. Get cart details
    const updatedCart = await client.getCart(cart.id);
    console.log('📦 Cart total:', `$${updatedCart.total}`, '\n');

    // 7. List orders
    console.log('7️⃣  Listing orders...');
    const orders = await client.listOrders({ page: 1, limit: 5 });
    console.log(`✅ Found ${orders.data.length} orders\n`);

    // 8. Check low stock items
    console.log('8️⃣  Checking low stock items...');
    const lowStock = await client.getLowStockItems();
    console.log(`📊 Low stock items: ${lowStock.length}\n`);

    // 9. Get dashboard metrics
    console.log('9️⃣  Fetching analytics...');
    const dashboard = await client.getDashboardMetrics();
    console.log('📈 Dashboard metrics retrieved\n');

    // 10. Create API key
    console.log('🔟 Creating API key...');
    const apiKey = await client.createApiKey('Test API Key', [
      'orders:read',
      'inventory:read',
    ]);
    console.log('✅ API Key created:', apiKey.key.substring(0, 20) + '...\n');

    console.log('✨ All examples completed successfully!');

  } catch (error) {
    if (axios.isAxiosError(error)) {
      const apiError = error.response?.data as ApiError;
      console.error('❌ API Error:', apiError?.error?.message || error.message);
      console.error('   Code:', apiError?.error?.code);
      console.error('   Status:', apiError?.error?.status);
      if (apiError?.error?.details) {
        console.error('   Details:', JSON.stringify(apiError.error.details, null, 2));
      }
    } else {
      console.error('❌ Unexpected error:', error);
    }
    process.exit(1);
  }
}

// Run the example if this file is executed directly
if (require.main === module) {
  main();
}

// Export the client for use in other modules
export default StateSetClient;
