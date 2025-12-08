import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const orderCreationTrend = new Trend('order_creation_duration');
const inventoryCheckTrend = new Trend('inventory_check_duration');

// Test configuration
export const options = {
  stages: [
    { duration: '30s', target: 20 },  // Ramp up to 20 users
    { duration: '1m', target: 50 },   // Stay at 50 users
    { duration: '30s', target: 100 }, // Spike to 100 users
    { duration: '1m', target: 50 },   // Back to 50 users
    { duration: '30s', target: 0 },   // Ramp down
  ],
  thresholds: {
    http_req_duration: ['p(95)<500', 'p(99)<1000'], // 95% under 500ms, 99% under 1s
    errors: ['rate<0.01'],                          // Error rate under 1%
    'order_creation_duration': ['p(95)<1000'],
    'inventory_check_duration': ['p(95)<200'],
  },
};

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const API_KEY = __ENV.API_KEY || 'test-api-key';

// Helper function for authenticated requests
function authHeaders() {
  return {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${API_KEY}`,
  };
}

export default function () {
  // Health check
  let healthRes = http.get(`${BASE_URL}/api/v1/health`);
  check(healthRes, {
    'health check status is 200': (r) => r.status === 200,
  });

  // List orders
  let ordersRes = http.get(`${BASE_URL}/api/v1/orders?page=1&limit=10`, {
    headers: authHeaders(),
  });
  check(ordersRes, {
    'list orders status is 200': (r) => r.status === 200,
    'list orders returns array': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.success === true;
      } catch {
        return false;
      }
    },
  });
  errorRate.add(ordersRes.status !== 200);

  // Check inventory
  let inventoryStart = Date.now();
  let inventoryRes = http.get(`${BASE_URL}/api/v1/inventory?page=1&limit=20`, {
    headers: authHeaders(),
  });
  inventoryCheckTrend.add(Date.now() - inventoryStart);
  check(inventoryRes, {
    'inventory check status is 200': (r) => r.status === 200,
  });
  errorRate.add(inventoryRes.status !== 200);

  // Create order (write operation)
  let orderStart = Date.now();
  let createOrderRes = http.post(
    `${BASE_URL}/api/v1/orders`,
    JSON.stringify({
      customer_id: `customer-${__VU}-${__ITER}`,
      order_number: `ORD-${Date.now()}-${__VU}`,
      status: 'pending',
      items: [
        {
          product_id: 'prod-001',
          quantity: 1,
          unit_price: '19.99',
        },
      ],
    }),
    { headers: authHeaders() }
  );
  orderCreationTrend.add(Date.now() - orderStart);
  check(createOrderRes, {
    'create order status is 200 or 201': (r) => r.status === 200 || r.status === 201,
  });
  errorRate.add(createOrderRes.status !== 200 && createOrderRes.status !== 201);

  // Status endpoint
  let statusRes = http.get(`${BASE_URL}/api/v1/status`);
  check(statusRes, {
    'status endpoint returns 200': (r) => r.status === 200,
  });

  sleep(1);
}

// Lifecycle hooks for setup/teardown
export function setup() {
  console.log('Starting load test against:', BASE_URL);

  // Verify API is available
  let res = http.get(`${BASE_URL}/api/v1/health`);
  if (res.status !== 200) {
    throw new Error(`API health check failed: ${res.status}`);
  }

  return { startTime: Date.now() };
}

export function teardown(data) {
  console.log(`Load test completed. Duration: ${(Date.now() - data.startTime) / 1000}s`);
}
