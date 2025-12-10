/**
 * StateSet API - React/Next.js Integration Example
 *
 * This example demonstrates how to integrate the StateSet API into a React/Next.js application:
 * - React Context for state management
 * - Custom hooks for API operations
 * - TypeScript types
 * - Error handling
 * - Loading states
 * - Authentication flow
 *
 * Installation:
 * npm install axios swr react react-dom next
 * npm install --save-dev @types/react @types/node
 *
 * Usage:
 * 1. Copy this file to your Next.js project
 * 2. Update the API_BASE_URL
 * 3. Import and use the hooks in your components
 */

import React, { createContext, useContext, useState, useEffect, ReactNode } from 'react';
import axios, { AxiosInstance, AxiosError } from 'axios';
import useSWR, { mutate } from 'swr';

// ============================================================================
// Configuration
// ============================================================================

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || 'http://localhost:8080/api/v1';

// ============================================================================
// Types
// ============================================================================

interface AuthTokens {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

interface User {
  id: string;
  email: string;
  first_name: string;
  last_name: string;
}

interface Order {
  id: string;
  customer_id: string;
  status: string;
  total_amount: number;
  currency: string;
  items: OrderItem[];
  created_at: string;
}

interface OrderItem {
  product_id: string;
  sku: string;
  quantity: number;
  unit_price: number;
  name: string;
}

interface Cart {
  id: string;
  customer_id?: string;
  items: CartItem[];
  subtotal: number;
  tax: number;
  shipping: number;
  total: number;
}

interface CartItem {
  id: string;
  product_id: string;
  sku: string;
  quantity: number;
  price: number;
  name: string;
}

interface Product {
  id: string;
  name: string;
  sku: string;
  price: number;
  description?: string;
  image_url?: string;
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
// API Client
// ============================================================================

class StateSetAPIClient {
  private client: AxiosInstance;
  private accessToken: string | null = null;

  constructor() {
    this.client = axios.create({
      baseURL: API_BASE_URL,
      headers: {
        'Content-Type': 'application/json',
      },
      timeout: 30000,
    });

    // Request interceptor
    this.client.interceptors.request.use((config) => {
      if (this.accessToken) {
        config.headers.Authorization = `Bearer ${this.accessToken}`;
      }
      return config;
    });

    // Response interceptor
    this.client.interceptors.response.use(
      (response) => response,
      (error: AxiosError<ApiError>) => {
        if (error.response?.data?.error) {
          throw new Error(error.response.data.error.message);
        }
        throw error;
      }
    );
  }

  setAccessToken(token: string) {
    this.accessToken = token;
  }

  async login(email: string, password: string): Promise<AuthTokens> {
    const response = await this.client.post<AuthTokens>('/auth/login', {
      email,
      password,
    });
    this.setAccessToken(response.data.access_token);
    return response.data;
  }

  async register(email: string, password: string, firstName: string, lastName: string): Promise<AuthTokens> {
    const response = await this.client.post<AuthTokens>('/auth/register', {
      email,
      password,
      first_name: firstName,
      last_name: lastName,
    });
    this.setAccessToken(response.data.access_token);
    return response.data;
  }

  async getOrders(params?: { page?: number; limit?: number; status?: string }) {
    const response = await this.client.get<{ data: Order[] }>('/orders', { params });
    return response.data;
  }

  async getOrder(orderId: string) {
    const response = await this.client.get<Order>(`/orders/${orderId}`);
    return response.data;
  }

  async createOrder(data: {
    customer_id: string;
    items: OrderItem[];
  }) {
    const response = await this.client.post<Order>('/orders', data);
    return response.data;
  }

  async getCart(cartId: string) {
    const response = await this.client.get<Cart>(`/carts/${cartId}`);
    return response.data;
  }

  async createCart(customerId?: string) {
    const response = await this.client.post<Cart>('/carts', {
      customer_id: customerId,
      session_id: Math.random().toString(36).substring(7),
    });
    return response.data;
  }

  async addItemToCart(cartId: string, item: Omit<CartItem, 'id'>) {
    const response = await this.client.post<Cart>(`/carts/${cartId}/items`, item);
    return response.data;
  }

  async updateCartItem(cartId: string, itemId: string, quantity: number) {
    const response = await this.client.put<Cart>(`/carts/${cartId}/items/${itemId}`, {
      quantity,
    });
    return response.data;
  }

  async removeCartItem(cartId: string, itemId: string) {
    const response = await this.client.delete<Cart>(`/carts/${cartId}/items/${itemId}`);
    return response.data;
  }

  async getProducts(params?: { page?: number; limit?: number }) {
    const response = await this.client.get<{ data: Product[] }>('/products', { params });
    return response.data;
  }
}

// ============================================================================
// Auth Context
// ============================================================================

interface AuthContextType {
  user: User | null;
  accessToken: string | null;
  isLoading: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (email: string, password: string, firstName: string, lastName: string) => Promise<void>;
  logout: () => void;
}

const AuthContext = createContext<AuthContextType | undefined>(undefined);

export const AuthProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  const [user, setUser] = useState<User | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [client] = useState(() => new StateSetAPIClient());

  useEffect(() => {
    // Load auth state from localStorage
    const storedToken = localStorage.getItem('access_token');
    const storedUser = localStorage.getItem('user');

    if (storedToken && storedUser) {
      setAccessToken(storedToken);
      setUser(JSON.parse(storedUser));
      client.setAccessToken(storedToken);
    }

    setIsLoading(false);
  }, [client]);

  const login = async (email: string, password: string) => {
    const tokens = await client.login(email, password);

    // Store tokens
    localStorage.setItem('access_token', tokens.access_token);
    localStorage.setItem('refresh_token', tokens.refresh_token);

    setAccessToken(tokens.access_token);

    // TODO: Fetch user profile
    const mockUser: User = {
      id: '1',
      email,
      first_name: 'User',
      last_name: 'Name',
    };
    setUser(mockUser);
    localStorage.setItem('user', JSON.stringify(mockUser));
  };

  const register = async (email: string, password: string, firstName: string, lastName: string) => {
    const tokens = await client.register(email, password, firstName, lastName);

    localStorage.setItem('access_token', tokens.access_token);
    localStorage.setItem('refresh_token', tokens.refresh_token);

    setAccessToken(tokens.access_token);

    const newUser: User = {
      id: '1',
      email,
      first_name: firstName,
      last_name: lastName,
    };
    setUser(newUser);
    localStorage.setItem('user', JSON.stringify(newUser));
  };

  const logout = () => {
    localStorage.removeItem('access_token');
    localStorage.removeItem('refresh_token');
    localStorage.removeItem('user');
    setAccessToken(null);
    setUser(null);
  };

  return (
    <AuthContext.Provider value={{ user, accessToken, isLoading, login, register, logout }}>
      {children}
    </AuthContext.Provider>
  );
};

export const useAuth = () => {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error('useAuth must be used within AuthProvider');
  }
  return context;
};

// ============================================================================
// Custom Hooks
// ============================================================================

/**
 * Hook for fetching orders
 */
export function useOrders(params?: { page?: number; limit?: number; status?: string }) {
  const { accessToken } = useAuth();
  const client = new StateSetAPIClient();

  if (accessToken) {
    client.setAccessToken(accessToken);
  }

  const { data, error, isLoading } = useSWR(
    accessToken ? ['/orders', params] : null,
    () => client.getOrders(params),
    {
      revalidateOnFocus: false,
      dedupingInterval: 60000, // 1 minute
    }
  );

  return {
    orders: data?.data || [],
    isLoading,
    error,
  };
}

/**
 * Hook for fetching a single order
 */
export function useOrder(orderId: string | null) {
  const { accessToken } = useAuth();
  const client = new StateSetAPIClient();

  if (accessToken) {
    client.setAccessToken(accessToken);
  }

  const { data, error, isLoading } = useSWR(
    accessToken && orderId ? `/orders/${orderId}` : null,
    () => client.getOrder(orderId!),
    {
      revalidateOnFocus: false,
    }
  );

  return {
    order: data || null,
    isLoading,
    error,
  };
}

/**
 * Hook for cart operations
 */
export function useCart(cartId: string | null) {
  const { accessToken } = useAuth();
  const [client] = useState(() => new StateSetAPIClient());

  useEffect(() => {
    if (accessToken) {
      client.setAccessToken(accessToken);
    }
  }, [accessToken, client]);

  const { data: cart, error, isLoading, mutate: mutateCart } = useSWR(
    cartId ? `/carts/${cartId}` : null,
    () => client.getCart(cartId!),
    {
      revalidateOnFocus: false,
    }
  );

  const addItem = async (item: Omit<CartItem, 'id'>) => {
    if (!cartId) throw new Error('No cart ID');
    const updatedCart = await client.addItemToCart(cartId, item);
    mutateCart(updatedCart);
    return updatedCart;
  };

  const updateItem = async (itemId: string, quantity: number) => {
    if (!cartId) throw new Error('No cart ID');
    const updatedCart = await client.updateCartItem(cartId, itemId, quantity);
    mutateCart(updatedCart);
    return updatedCart;
  };

  const removeItem = async (itemId: string) => {
    if (!cartId) throw new Error('No cart ID');
    const updatedCart = await client.removeCartItem(cartId, itemId);
    mutateCart(updatedCart);
    return updatedCart;
  };

  return {
    cart: cart || null,
    isLoading,
    error,
    addItem,
    updateItem,
    removeItem,
  };
}

/**
 * Hook for product list
 */
export function useProducts(params?: { page?: number; limit?: number }) {
  const { accessToken } = useAuth();
  const client = new StateSetAPIClient();

  if (accessToken) {
    client.setAccessToken(accessToken);
  }

  const { data, error, isLoading } = useSWR(
    ['/products', params],
    () => client.getProducts(params),
    {
      revalidateOnFocus: false,
      dedupingInterval: 300000, // 5 minutes
    }
  );

  return {
    products: data?.data || [],
    isLoading,
    error,
  };
}

// ============================================================================
// Example Components
// ============================================================================

/**
 * Login Form Component
 */
export const LoginForm: React.FC = () => {
  const { login } = useAuth();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);

    try {
      await login(email, password);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed');
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form onSubmit={handleSubmit} className="space-y-4">
      <div>
        <label htmlFor="email" className="block text-sm font-medium">
          Email
        </label>
        <input
          id="email"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          required
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2"
        />
      </div>

      <div>
        <label htmlFor="password" className="block text-sm font-medium">
          Password
        </label>
        <input
          id="password"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          required
          className="mt-1 block w-full rounded-md border border-gray-300 px-3 py-2"
        />
      </div>

      {error && (
        <div className="text-red-600 text-sm">{error}</div>
      )}

      <button
        type="submit"
        disabled={isLoading}
        className="w-full bg-blue-600 text-white py-2 px-4 rounded-md hover:bg-blue-700 disabled:opacity-50"
      >
        {isLoading ? 'Logging in...' : 'Login'}
      </button>
    </form>
  );
};

/**
 * Order List Component
 */
export const OrderList: React.FC = () => {
  const { orders, isLoading, error } = useOrders({ page: 1, limit: 10 });

  if (isLoading) return <div>Loading orders...</div>;
  if (error) return <div>Error loading orders: {error.message}</div>;

  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-bold">Your Orders</h2>

      {orders.length === 0 ? (
        <p>No orders found</p>
      ) : (
        <div className="space-y-2">
          {orders.map((order) => (
            <div key={order.id} className="border rounded-lg p-4">
              <div className="flex justify-between items-center">
                <div>
                  <p className="font-semibold">Order #{order.id.substring(0, 8)}</p>
                  <p className="text-sm text-gray-600">{order.status}</p>
                </div>
                <div className="text-right">
                  <p className="font-bold">${order.total_amount.toFixed(2)}</p>
                  <p className="text-sm text-gray-600">
                    {new Date(order.created_at).toLocaleDateString()}
                  </p>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

/**
 * Shopping Cart Component
 */
export const ShoppingCart: React.FC<{ cartId: string }> = ({ cartId }) => {
  const { cart, isLoading, error, updateItem, removeItem } = useCart(cartId);

  if (isLoading) return <div>Loading cart...</div>;
  if (error) return <div>Error loading cart: {error.message}</div>;
  if (!cart) return <div>Cart not found</div>;

  return (
    <div className="space-y-4">
      <h2 className="text-2xl font-bold">Shopping Cart</h2>

      {cart.items.length === 0 ? (
        <p>Your cart is empty</p>
      ) : (
        <>
          <div className="space-y-2">
            {cart.items.map((item) => (
              <div key={item.id} className="border rounded-lg p-4 flex justify-between items-center">
                <div>
                  <p className="font-semibold">{item.name}</p>
                  <p className="text-sm text-gray-600">SKU: {item.sku}</p>
                  <p className="text-sm">${item.price.toFixed(2)} each</p>
                </div>

                <div className="flex items-center space-x-2">
                  <button
                    onClick={() => updateItem(item.id, Math.max(1, item.quantity - 1))}
                    className="px-2 py-1 border rounded"
                  >
                    -
                  </button>
                  <span className="px-4">{item.quantity}</span>
                  <button
                    onClick={() => updateItem(item.id, item.quantity + 1)}
                    className="px-2 py-1 border rounded"
                  >
                    +
                  </button>
                  <button
                    onClick={() => removeItem(item.id)}
                    className="ml-4 text-red-600 hover:text-red-800"
                  >
                    Remove
                  </button>
                </div>
              </div>
            ))}
          </div>

          <div className="border-t pt-4">
            <div className="flex justify-between text-lg font-bold">
              <span>Total:</span>
              <span>${cart.total.toFixed(2)}</span>
            </div>
          </div>
        </>
      )}
    </div>
  );
};

/**
 * Product Grid Component
 */
export const ProductGrid: React.FC<{ onAddToCart: (product: Product) => void }> = ({ onAddToCart }) => {
  const { products, isLoading, error } = useProducts({ page: 1, limit: 12 });

  if (isLoading) return <div>Loading products...</div>;
  if (error) return <div>Error loading products: {error.message}</div>;

  return (
    <div className="grid grid-cols-1 md:grid-cols-3 lg:grid-cols-4 gap-6">
      {products.map((product) => (
        <div key={product.id} className="border rounded-lg p-4">
          {product.image_url && (
            <img src={product.image_url} alt={product.name} className="w-full h-48 object-cover mb-4" />
          )}
          <h3 className="font-semibold">{product.name}</h3>
          <p className="text-sm text-gray-600 mb-2">{product.description}</p>
          <div className="flex justify-between items-center">
            <span className="font-bold">${product.price.toFixed(2)}</span>
            <button
              onClick={() => onAddToCart(product)}
              className="bg-blue-600 text-white px-4 py-2 rounded hover:bg-blue-700"
            >
              Add to Cart
            </button>
          </div>
        </div>
      ))}
    </div>
  );
};

// ============================================================================
// Example Page Component
// ============================================================================

/**
 * Example Next.js Page
 */
export default function ExamplePage() {
  const { user, isLoading, logout } = useAuth();

  if (isLoading) {
    return <div>Loading...</div>;
  }

  if (!user) {
    return (
      <div className="max-w-md mx-auto mt-8">
        <h1 className="text-3xl font-bold mb-6">Login</h1>
        <LoginForm />
      </div>
    );
  }

  return (
    <div className="max-w-6xl mx-auto p-8">
      <div className="flex justify-between items-center mb-8">
        <h1 className="text-3xl font-bold">Welcome, {user.first_name}!</h1>
        <button
          onClick={logout}
          className="bg-gray-600 text-white px-4 py-2 rounded hover:bg-gray-700"
        >
          Logout
        </button>
      </div>

      <div className="space-y-8">
        <OrderList />
      </div>
    </div>
  );
}
