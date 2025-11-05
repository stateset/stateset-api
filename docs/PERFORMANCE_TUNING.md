# StateSet API - Performance Tuning Guide

Comprehensive guide to optimizing StateSet API for production workloads.

## Table of Contents

- [Performance Baselines](#performance-baselines)
- [Database Optimization](#database-optimization)
- [Caching Strategies](#caching-strategies)
- [Connection Pooling](#connection-pooling)
- [Query Optimization](#query-optimization)
- [API Client Optimization](#api-client-optimization)
- [Load Testing](#load-testing)
- [Monitoring Performance](#monitoring-performance)
- [Scaling Strategies](#scaling-strategies)
- [Common Bottlenecks](#common-bottlenecks)

---

## Performance Baselines

### Expected Performance (Default Configuration)

**Response Times (p50 / p95 / p99):**
- `GET /health`: 5ms / 10ms / 15ms
- `GET /orders`: 30ms / 80ms / 150ms
- `POST /orders`: 50ms / 120ms / 200ms
- `GET /products/search`: 40ms / 100ms / 180ms
- `POST /inventory/reserve`: 45ms / 110ms / 190ms

**Throughput:**
- Simple reads: 2000+ req/s
- Complex queries: 500-1000 req/s
- Writes with validation: 800-1200 req/s

**Resource Usage (Single Instance):**
- Memory: 200-400 MB baseline
- CPU: <10% idle, 40-60% under load
- Connections: 20 database, 10 Redis

### When to Optimize

Optimize when you observe:
- Response times >500ms (p95)
- Throughput <100 req/s
- CPU usage >80%
- Memory usage >1GB
- Database connections exhausted
- Cache hit rate <70%

---

## Database Optimization

### Add Indexes for Common Queries

**Identify slow queries:**
```sql
-- PostgreSQL: Find slow queries
SELECT
  query,
  mean_exec_time,
  calls,
  total_exec_time
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 20;
```

**Essential indexes:**
```sql
-- Orders table
CREATE INDEX idx_orders_customer_id ON orders(customer_id);
CREATE INDEX idx_orders_status ON orders(status);
CREATE INDEX idx_orders_created_at ON orders(created_at DESC);
CREATE INDEX idx_orders_order_number ON orders(order_number);
CREATE INDEX idx_orders_status_created ON orders(status, created_at DESC);

-- Inventory table
CREATE INDEX idx_inventory_product_id ON inventory(product_id);
CREATE INDEX idx_inventory_location_id ON inventory(location_id);
CREATE INDEX idx_inventory_sku ON inventory(sku);
CREATE INDEX idx_inventory_low_stock ON inventory(quantity_available)
  WHERE quantity_available <= reorder_point;

-- Products table
CREATE INDEX idx_products_sku ON products(sku);
CREATE INDEX idx_products_active ON products(is_active)
  WHERE is_active = true;
CREATE INDEX idx_products_name_search ON products
  USING gin(to_tsvector('english', name));

-- Order items table
CREATE INDEX idx_order_items_order_id ON order_items(order_id);
CREATE INDEX idx_order_items_product_id ON order_items(product_id);

-- Shipments table
CREATE INDEX idx_shipments_order_id ON shipments(order_id);
CREATE INDEX idx_shipments_tracking_number ON shipments(tracking_number);
CREATE INDEX idx_shipments_status ON shipments(status);

-- Returns table
CREATE INDEX idx_returns_order_id ON returns(order_id);
CREATE INDEX idx_returns_status ON returns(status);
CREATE INDEX idx_returns_rma_number ON returns(rma_number);

-- Customers table
CREATE INDEX idx_customers_email ON customers(email);
```

**Monitor index usage:**
```sql
-- Check unused indexes
SELECT
  schemaname,
  tablename,
  indexname,
  idx_scan
FROM pg_stat_user_indexes
WHERE idx_scan = 0
  AND indexrelname NOT LIKE 'pg_%'
ORDER BY tablename, indexname;

-- Drop unused indexes
DROP INDEX idx_unused_index;
```

### Optimize Table Statistics

```sql
-- Update statistics for better query planning
ANALYZE orders;
ANALYZE inventory;
ANALYZE products;

-- Schedule regular statistics updates
-- Add to cron: 0 2 * * * psql -c "ANALYZE"
```

### Partition Large Tables

**When to partition:**
- Tables >10 million rows
- Time-series data (orders, events, logs)
- Archival requirements

**Example: Partition orders by month:**
```sql
-- Create partitioned table
CREATE TABLE orders (
  id UUID NOT NULL,
  created_at TIMESTAMP NOT NULL,
  ...
) PARTITION BY RANGE (created_at);

-- Create partitions
CREATE TABLE orders_2025_01 PARTITION OF orders
  FOR VALUES FROM ('2025-01-01') TO ('2025-02-01');

CREATE TABLE orders_2025_02 PARTITION OF orders
  FOR VALUES FROM ('2025-02-01') TO ('2025-03-01');

-- Auto-create partitions with pg_partman extension
```

### Optimize PostgreSQL Configuration

**For 8GB RAM server:**
```ini
# /etc/postgresql/14/main/postgresql.conf

# Memory
shared_buffers = 2GB
effective_cache_size = 6GB
work_mem = 16MB
maintenance_work_mem = 512MB

# Connections
max_connections = 100

# WAL
wal_buffers = 16MB
checkpoint_completion_target = 0.9
checkpoint_timeout = 15min

# Query Planning
random_page_cost = 1.1  # For SSD
effective_io_concurrency = 200

# Parallel Queries
max_parallel_workers_per_gather = 4
max_parallel_workers = 8
```

**Apply changes:**
```bash
sudo systemctl restart postgresql
```

---

## Caching Strategies

### Enable Redis Caching

**Configuration:**
```toml
# config/default.toml
[cache]
enabled = true
redis_url = "redis://localhost:6379"
default_ttl = 3600  # 1 hour

[cache.entities]
products = 3600       # 1 hour
customers = 1800      # 30 minutes
inventory = 300       # 5 minutes
orders = 600          # 10 minutes
```

### Cache Hot Data

**Products (high read, low write):**
```rust
// Cache product data
pub async fn get_product(
    cache: &Cache,
    db: &DatabaseConnection,
    id: Uuid,
) -> Result<Product> {
    let cache_key = format!("product:{}", id);

    // Try cache first
    if let Some(cached) = cache.get(&cache_key).await? {
        return Ok(cached);
    }

    // Fetch from database
    let product = db.get_product(id).await?;

    // Store in cache
    cache.set(&cache_key, &product, 3600).await?;

    Ok(product)
}
```

**Inventory (medium read, high write):**
```rust
// Shorter TTL due to frequent updates
cache.set(&cache_key, &inventory, 300).await?;  // 5 minutes
```

**Session data (high read, high write):**
```rust
// Store in Redis with short TTL
cache.set(&session_key, &session, 900).await?;  // 15 minutes
```

### Cache Invalidation

**On update:**
```rust
pub async fn update_product(
    cache: &Cache,
    db: &DatabaseConnection,
    id: Uuid,
    update: ProductUpdate,
) -> Result<Product> {
    // Update database
    let product = db.update_product(id, update).await?;

    // Invalidate cache
    let cache_key = format!("product:{}", id);
    cache.delete(&cache_key).await?;

    Ok(product)
}
```

**Pattern-based invalidation:**
```rust
// Invalidate all product caches
cache.delete_pattern("product:*").await?;

// Invalidate customer's orders cache
cache.delete_pattern(&format!("orders:customer:{}:*", customer_id)).await?;
```

### Monitor Cache Performance

```bash
# Redis stats
redis-cli INFO stats | grep hits
# keyspace_hits:1000000
# keyspace_misses:100000
# Hit rate: 90.9%

# Monitor memory usage
redis-cli INFO memory | grep used_memory_human

# Monitor slow commands
redis-cli SLOWLOG GET 10
```

---

## Connection Pooling

### Optimize Database Pool

**Configuration:**
```toml
# config/default.toml
[database]
# Connection pool settings
max_connections = 50      # Max pool size
min_connections = 10      # Always keep 10 ready
connect_timeout = 30      # 30 seconds
idle_timeout = 600        # Close idle connections after 10 minutes
max_lifetime = 1800       # Recycle connections after 30 minutes

# Statement cache
statement_cache_size = 100
```

**Right-size your pool:**
```
Optimal connections = ((core_count * 2) + effective_spindle_count)

Example:
- 4 CPU cores
- 1 SSD (effective_spindle_count = 1)
Result: (4 * 2) + 1 = 9 connections

Add buffer: 10-15 connections minimum
For high concurrency: 20-50 connections
```

**Monitor pool exhaustion:**
```rust
// Check pool metrics
let pool_status = db.pool().status();
println!("Connections: {} / {}", pool_status.connections, pool_status.max_size);
println!("Idle: {}", pool_status.idle_connections);
```

### Redis Connection Pool

```toml
[redis]
url = "redis://localhost:6379"
pool_size = 20
timeout = 5000  # 5 seconds
```

---

## Query Optimization

### Avoid N+1 Queries

**❌ Bad: N+1 queries**
```rust
// Loads orders
let orders = Orders::find().all(db).await?;

// For each order, load customer (N queries)
for order in orders {
    let customer = Customers::find_by_id(order.customer_id)
        .one(db)
        .await?;
}
```

**✅ Good: Eager loading**
```rust
// Load orders with customers in one query
let orders = Orders::find()
    .find_also_related(Customers)
    .all(db)
    .await?;

// Or use JOIN
let orders = Orders::find()
    .inner_join(Customers)
    .all(db)
    .await?;
```

### Use Pagination

**Always paginate large result sets:**
```rust
let orders = Orders::find()
    .order_by_desc(Column::CreatedAt)
    .paginate(db, 20)  // 20 per page
    .fetch_page(page)
    .await?;
```

### Select Only Needed Columns

**❌ Bad: SELECT ***
```rust
// Loads all columns (including large JSON fields)
let orders = Orders::find().all(db).await?;
```

**✅ Good: SELECT specific columns**
```rust
let orders = Orders::find()
    .select_only()
    .column(orders::Column::Id)
    .column(orders::Column::OrderNumber)
    .column(orders::Column::TotalAmount)
    .column(orders::Column::Status)
    .into_model::<OrderSummary>()
    .all(db)
    .await?;
```

### Use Database Functions

**Calculate totals in database:**
```rust
// ❌ Bad: Load all items, sum in application
let items = OrderItems::find()
    .filter(order_items::Column::OrderId.eq(order_id))
    .all(db)
    .await?;
let total = items.iter().map(|i| i.total_price).sum();

// ✅ Good: Sum in database
let total: Option<f64> = OrderItems::find()
    .filter(order_items::Column::OrderId.eq(order_id))
    .select_only()
    .column_as(order_items::Column::TotalPrice.sum(), "total")
    .into_tuple()
    .one(db)
    .await?;
```

### Batch Inserts

**❌ Bad: Individual inserts**
```rust
for item in items {
    OrderItems::insert(item).exec(db).await?;
}
```

**✅ Good: Batch insert**
```rust
OrderItems::insert_many(items).exec(db).await?;
```

---

## API Client Optimization

### Use HTTP/2

```rust
let client = reqwest::Client::builder()
    .http2_prior_knowledge()  // Use HTTP/2
    .pool_max_idle_per_host(10)
    .timeout(Duration::from_secs(30))
    .build()?;
```

### Enable Compression

```rust
let client = reqwest::Client::builder()
    .gzip(true)  // Enable gzip compression
    .brotli(true) // Enable brotli compression
    .build()?;
```

### Reuse Connections

**✅ DO:**
```javascript
// Create client once, reuse for all requests
const client = axios.create({
  baseURL: 'http://localhost:8080/api/v1',
  timeout: 30000,
  maxRedirects: 5,
  httpAgent: new http.Agent({
    keepAlive: true,
    maxSockets: 50
  })
});

// Reuse client
await client.get('/orders');
await client.post('/orders', data);
```

**❌ DON'T:**
```javascript
// Creating new connection for each request
await axios.get('http://localhost:8080/api/v1/orders');
await axios.post('http://localhost:8080/api/v1/orders', data);
```

### Parallel Requests

**❌ Sequential:**
```javascript
const customer = await api.get(`/customers/${id}`);
const orders = await api.get(`/customers/${id}/orders`);
const addresses = await api.get(`/customers/${id}/addresses`);
// Total: ~150ms
```

**✅ Parallel:**
```javascript
const [customer, orders, addresses] = await Promise.all([
  api.get(`/customers/${id}`),
  api.get(`/customers/${id}/orders`),
  api.get(`/customers/${id}/addresses`)
]);
// Total: ~50ms (fastest of the three)
```

---

## Load Testing

### Use k6 for Load Testing

**Install k6:**
```bash
brew install k6  # macOS
# or
sudo apt-get install k6  # Linux
```

**Create test script (`load-test.js`):**
```javascript
import http from 'k6/http';
import { check, sleep } from 'k6';

export const options = {
  stages: [
    { duration: '2m', target: 100 },   // Ramp up to 100 users
    { duration: '5m', target: 100 },   // Stay at 100 users
    { duration: '2m', target: 200 },   // Ramp up to 200 users
    { duration: '5m', target: 200 },   // Stay at 200 users
    { duration: '2m', target: 0 },     // Ramp down to 0
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],  // 95% of requests < 500ms
    http_req_failed: ['rate<0.01'],    // Error rate < 1%
  },
};

const BASE_URL = 'http://localhost:8080/api/v1';
const API_KEY = 'your-test-api-key';

export default function () {
  // Test: List orders
  const listResponse = http.get(`${BASE_URL}/orders?page=1&limit=20`, {
    headers: { 'X-API-Key': API_KEY },
  });

  check(listResponse, {
    'list orders status is 200': (r) => r.status === 200,
    'list orders duration < 200ms': (r) => r.timings.duration < 200,
  });

  sleep(1);

  // Test: Create order
  const orderPayload = JSON.stringify({
    customer_id: '550e8400-e29b-41d4-a716-446655440001',
    items: [{
      product_id: '550e8400-e29b-41d4-a716-446655440002',
      sku: 'TEST-001',
      quantity: 2,
      unit_price: 29.99
    }],
    total_amount: 59.98
  });

  const createResponse = http.post(`${BASE_URL}/orders`, orderPayload, {
    headers: {
      'Content-Type': 'application/json',
      'X-API-Key': API_KEY,
      'Idempotency-Key': `test-${Date.now()}-${Math.random()}`
    },
  });

  check(createResponse, {
    'create order status is 200': (r) => r.status === 200,
    'create order duration < 500ms': (r) => r.timings.duration < 500,
  });

  sleep(1);
}
```

**Run load test:**
```bash
k6 run load-test.js

# Or with cloud integration
k6 cloud load-test.js
```

**Interpret results:**
```
checks.........................: 99.95% ✓ 15992  ✗ 8
data_received..................: 4.8 MB 16 kB/s
data_sent......................: 2.4 MB 8.1 kB/s
http_req_blocked...............: avg=1.2ms   min=1µs     med=4µs     max=123ms  p(90)=7µs     p(95)=10µs
http_req_connecting............: avg=600µs   min=0s      med=0s      max=85ms   p(90)=0s      p(95)=0s
http_req_duration..............: avg=45ms    min=12ms    med=38ms    max=890ms  p(90)=85ms    p(95)=125ms
http_req_receiving.............: avg=180µs   min=15µs    med=95µs    max=12ms   p(90)=350µs   p(95)=550µs
http_req_sending...............: avg=35µs    min=8µs     med=28µs    max=5ms    p(90)=55µs    p(95)=75µs
http_req_tls_handshaking.......: avg=0s      min=0s      med=0s      max=0s     p(90)=0s      p(95)=0s
http_req_waiting...............: avg=44.8ms  min=11.8ms  med=37.5ms  max=888ms  p(90)=84ms    p(95)=124ms
http_reqs......................: 8000   26.666667/s
iteration_duration.............: avg=2.09s   min=2.04s   med=2.08s   max=2.95s  p(90)=2.13s   p(95)=2.18s
iterations.....................: 4000   13.333333/s
vus............................: 200    min=0    max=200
vus_max........................: 200    min=200  max=200
```

---

## Monitoring Performance

### Enable Prometheus Metrics

**Access metrics:**
```bash
# Text format (for Prometheus scraping)
curl http://localhost:8080/metrics

# JSON format
curl http://localhost:8080/metrics/json
```

**Key metrics to monitor:**
```
# Request rate
rate(http_requests_total[5m])

# Error rate
rate(http_requests_total{status=~"5.."}[5m])

# Response time (p95)
histogram_quantile(0.95, http_request_duration_seconds)

# Database connection pool
database_connections{state="idle"}
database_connections{state="active"}

# Cache hit rate
cache_hits_total / (cache_hits_total + cache_misses_total)
```

### Grafana Dashboard

**Import dashboard:**
1. Go to Grafana → Create → Import
2. Upload JSON dashboard
3. Select Prometheus data source

**Key panels:**
- Request rate over time
- Error rate over time
- Response time (p50, p95, p99)
- Active database connections
- Cache hit rate
- CPU and memory usage

### Application Performance Monitoring (APM)

**OpenTelemetry integration:**
```toml
[telemetry]
enabled = true
endpoint = "http://localhost:4317"
service_name = "stateset-api"
```

**Available in:**
- Jaeger
- Zipkin
- Datadog APM
- New Relic
- Elastic APM

---

## Scaling Strategies

### Vertical Scaling (Scale Up)

**When to scale up:**
- CPU usage consistently >80%
- Memory pressure
- I/O bottlenecks
- Single instance performance inadequate

**Recommended sizes:**
```
Small:   2 vCPU,  4 GB RAM  → ~100 req/s
Medium:  4 vCPU,  8 GB RAM  → ~500 req/s
Large:   8 vCPU, 16 GB RAM  → ~1500 req/s
X-Large: 16 vCPU, 32 GB RAM → ~3000 req/s
```

### Horizontal Scaling (Scale Out)

**When to scale out:**
- Vertical scaling becomes expensive
- Need high availability
- Geographic distribution
- Traffic spikes

**Load balancer configuration:**
```nginx
upstream stateset_api {
    least_conn;  # Route to least busy server

    server api1:8080 max_fails=3 fail_timeout=30s;
    server api2:8080 max_fails=3 fail_timeout=30s;
    server api3:8080 max_fails=3 fail_timeout=30s;
    server api4:8080 max_fails=3 fail_timeout=30s;

    keepalive 32;
}

server {
    listen 80;

    location / {
        proxy_pass http://stateset_api;
        proxy_http_version 1.1;
        proxy_set_header Connection "";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

**Kubernetes HPA (Horizontal Pod Autoscaler):**
```yaml
apiVersion: autoscaling/v2
kind: HorizontalPodAutoscaler
metadata:
  name: stateset-api-hpa
spec:
  scaleTargetRef:
    apiVersion: apps/v1
    kind: Deployment
    name: stateset-api
  minReplicas: 3
  maxReplicas: 20
  metrics:
  - type: Resource
    resource:
      name: cpu
      target:
        type: Utilization
        averageUtilization: 70
  - type: Resource
    resource:
      name: memory
      target:
        type: Utilization
        averageUtilization: 80
```

### Database Read Replicas

**Setup read replicas:**
```toml
[database]
# Primary (writes)
primary_url = "postgres://user:pass@primary:5432/stateset"

# Replicas (reads)
replica_urls = [
  "postgres://user:pass@replica1:5432/stateset",
  "postgres://user:pass@replica2:5432/stateset"
]

# Routing
read_preference = "nearest"  # or "random", "round_robin"
```

**Query routing:**
```rust
// Write to primary
db.primary().insert_order(order).await?;

// Read from replica
let orders = db.replica().find_orders(filters).await?;
```

---

## Common Bottlenecks

### Symptom: Slow List Endpoints

**Diagnosis:**
```sql
-- Check if query uses index
EXPLAIN ANALYZE
SELECT * FROM orders
WHERE status = 'pending'
ORDER BY created_at DESC
LIMIT 20;
```

**Solutions:**
1. Add composite index: `CREATE INDEX ON orders(status, created_at DESC)`
2. Reduce columns in SELECT
3. Ensure pagination is used
4. Cache results

### Symptom: High Memory Usage

**Diagnosis:**
```bash
# Monitor memory
watch -n 1 'ps aux | grep stateset-api'

# Check for memory leaks
valgrind --leak-check=full ./target/release/stateset-api
```

**Solutions:**
1. Reduce connection pool size
2. Reduce cache size
3. Check for connection leaks
4. Profile with valgrind or heaptrack

### Symptom: Database Connection Exhaustion

**Diagnosis:**
```sql
-- Check active connections
SELECT count(*), state FROM pg_stat_activity
WHERE datname = 'stateset'
GROUP BY state;
```

**Solutions:**
1. Increase max_connections in PostgreSQL
2. Reduce pool size per instance
3. Fix connection leaks (ensure Drop)
4. Use PgBouncer for connection pooling

### Symptom: Slow Writes

**Diagnosis:**
```sql
-- Check for locks
SELECT * FROM pg_locks WHERE NOT granted;

-- Check for long transactions
SELECT * FROM pg_stat_activity
WHERE state = 'active'
  AND xact_start < now() - interval '1 minute';
```

**Solutions:**
1. Optimize indexes (too many slow down writes)
2. Batch inserts/updates
3. Use async processing for non-critical writes
4. Check for lock contention

---

## Performance Checklist

**Database:**
- [ ] Indexes on foreign keys
- [ ] Indexes on filtered columns
- [ ] Indexes on ORDER BY columns
- [ ] Regular ANALYZE/VACUUM
- [ ] Connection pooling configured
- [ ] Query execution plans reviewed

**Caching:**
- [ ] Redis enabled
- [ ] Hot data cached
- [ ] Cache invalidation on updates
- [ ] Cache hit rate >70%

**Application:**
- [ ] Pagination everywhere
- [ ] N+1 queries eliminated
- [ ] Batch operations used
- [ ] Compression enabled
- [ ] HTTP/2 enabled

**Monitoring:**
- [ ] Prometheus metrics enabled
- [ ] Grafana dashboards created
- [ ] Alerts configured
- [ ] APM enabled (optional)

**Load Testing:**
- [ ] Load tests created
- [ ] Baseline performance measured
- [ ] Bottlenecks identified
- [ ] Improvements validated

---

**Need help optimizing?**
- [Database Guide](./DATABASE.md) - Database management
- [Monitoring Guide](./MONITORING.md) - Observability setup
- [Deployment Guide](./DEPLOYMENT.md) - Production deployment
- [Troubleshooting Guide](./TROUBLESHOOTING.md) - Common issues

[← Back to Documentation Index](./DOCUMENTATION_INDEX.md)
