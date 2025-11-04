# Monitoring and Observability Guide

This guide covers monitoring, alerting, and observability for StateSet API in production.

## Table of Contents

- [Overview](#overview)
- [Metrics](#metrics)
- [Logging](#logging)
- [Tracing](#tracing)
- [Alerting](#alerting)
- [Dashboards](#dashboards)
- [Performance Monitoring](#performance-monitoring)
- [Best Practices](#best-practices)

---

## Overview

StateSet API provides comprehensive observability through:

- **Prometheus Metrics**: Exposed at `/metrics` endpoint
- **Structured Logging**: JSON format with slog
- **Distributed Tracing**: OpenTelemetry integration
- **Health Checks**: `/health` endpoints for monitoring

---

## Metrics

### Available Metrics

The API exposes Prometheus-compatible metrics at `/metrics` (text format) and `/metrics/json` (JSON format).

#### HTTP Metrics

```promql
# Total HTTP requests by method, route, and status
http_requests_total{method="GET", route="/api/v1/orders", status="200"}

# HTTP request duration histogram (milliseconds)
http_request_duration_ms_bucket{method="POST", route="/api/v1/orders", status="201", le="100"}
http_request_duration_ms_sum{method="POST", route="/api/v1/orders", status="201"}
http_request_duration_ms_count{method="POST", route="/api/v1/orders", status="201"}
```

#### Rate Limiting Metrics

```promql
# Rate limit denied/allowed counts
rate_limit_denied_total{key_type="global", path="/api/v1/orders"}
rate_limit_allowed_total{key_type="api_key", path="/api/v1/inventory"}
```

#### Authentication Metrics

```promql
# Authentication failures
auth_failures_total{code="invalid_token", status="401"}
```

#### Business Metrics

```promql
# Orders created
orders_created_total

# Returns processed
returns_processed_total

# Inventory adjustments
inventory_adjustments_total

# Shipments created
shipments_created_total
```

#### System Metrics

```promql
# Cache hits/misses
cache_hits_total
cache_misses_total

# Error counts
errors_total{type="database", severity="error"}

# Database connections
database_connections_active
database_connections_idle
```

### Prometheus Configuration

**prometheus.yml**:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'production'
    app: 'stateset-api'

scrape_configs:
  - job_name: 'stateset-api'
    scrape_interval: 15s
    scrape_timeout: 10s
    metrics_path: '/metrics'
    static_configs:
      - targets:
          - 'api-server-1:8080'
          - 'api-server-2:8080'
          - 'api-server-3:8080'
        labels:
          environment: 'production'
          region: 'us-east-1'

  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']

  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']
```

### Example Queries

**Request Rate (RPS)**:
```promql
rate(http_requests_total[5m])
```

**Success Rate**:
```promql
sum(rate(http_requests_total{status=~"2.."}[5m])) /
sum(rate(http_requests_total[5m])) * 100
```

**Error Rate**:
```promql
sum(rate(http_requests_total{status=~"5.."}[5m])) by (route)
```

**95th Percentile Latency**:
```promql
histogram_quantile(0.95,
  rate(http_request_duration_ms_bucket[5m])
)
```

**99th Percentile Latency**:
```promql
histogram_quantile(0.99,
  rate(http_request_duration_ms_bucket[5m])
)
```

**Average Response Time**:
```promql
rate(http_request_duration_ms_sum[5m]) /
rate(http_request_duration_ms_count[5m])
```

**Top 5 Slowest Endpoints**:
```promql
topk(5,
  sum(rate(http_request_duration_ms_sum[5m])) by (route) /
  sum(rate(http_request_duration_ms_count[5m])) by (route)
)
```

**Cache Hit Rate**:
```promql
sum(rate(cache_hits_total[5m])) /
(sum(rate(cache_hits_total[5m])) + sum(rate(cache_misses_total[5m]))) * 100
```

---

## Logging

### Log Levels

StateSet API uses structured logging with **slog**:

- **trace**: Very detailed debugging information
- **debug**: Debugging information
- **info**: General informational messages (default in production)
- **warn**: Warning messages
- **error**: Error messages
- **critical**: Critical issues requiring immediate attention

### Configuration

```bash
# Set log level
APP__LOG_LEVEL=info

# Set log format (json or text)
APP__LOG_FORMAT=json
```

### Log Format

**JSON Format** (recommended for production):

```json
{
  "timestamp": "2024-11-03T14:30:00.123Z",
  "level": "info",
  "message": "Order created successfully",
  "request_id": "550e8400-e29b-41d4-a716-446655440000",
  "user_id": "123e4567-e89b-12d3-a456-426614174000",
  "order_id": "789abcde-f012-3456-7890-abcdef123456",
  "duration_ms": 45,
  "status": 201
}
```

### Log Aggregation

#### Using Loki

**docker-compose.logging.yml**:

```yaml
version: '3.8'

services:
  loki:
    image: grafana/loki:latest
    ports:
      - "3100:3100"
    volumes:
      - ./loki-config.yml:/etc/loki/local-config.yaml
      - loki_data:/loki

  promtail:
    image: grafana/promtail:latest
    volumes:
      - /var/log:/var/log
      - ./promtail-config.yml:/etc/promtail/config.yml
    command: -config.file=/etc/promtail/config.yml

  grafana:
    image: grafana/grafana:latest
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana

volumes:
  loki_data:
  grafana_data:
```

#### Using ELK Stack

**Filebeat Configuration**:

```yaml
filebeat.inputs:
  - type: log
    enabled: true
    paths:
      - /var/log/stateset-api/*.log
    json.keys_under_root: true
    json.add_error_key: true

output.elasticsearch:
  hosts: ["localhost:9200"]
  index: "stateset-api-%{+yyyy.MM.dd}"

setup.template.name: "stateset-api"
setup.template.pattern: "stateset-api-*"
```

---

## Tracing

### OpenTelemetry Configuration

StateSet API supports distributed tracing with OpenTelemetry.

**Enable tracing**:

```bash
APP__OTEL_ENABLED=true
APP__OTEL_ENDPOINT=http://localhost:4317
APP__OTEL_SERVICE_NAME=stateset-api
```

### Jaeger Setup

```yaml
# docker-compose.tracing.yml
version: '3.8'

services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    environment:
      - COLLECTOR_OTLP_ENABLED=true
    ports:
      - "16686:16686"  # Jaeger UI
      - "4317:4317"    # OTLP gRPC
      - "4318:4318"    # OTLP HTTP
```

### Trace Context

Every request includes a `X-Request-Id` header for correlation:

```bash
curl -H "X-Request-Id: my-unique-id" https://api.stateset.com/api/v1/orders
```

Traces include:
- Request ID
- User ID (if authenticated)
- Request method and path
- Response status
- Duration
- Database queries
- External API calls
- Cache operations

---

## Alerting

### Alert Rules

**alerts.yml**:

```yaml
groups:
  - name: stateset-api-alerts
    interval: 30s
    rules:
      # High Error Rate
      - alert: HighErrorRate
        expr: |
          (
            sum(rate(http_requests_total{status=~"5.."}[5m]))
            /
            sum(rate(http_requests_total[5m]))
          ) > 0.05
        for: 5m
        labels:
          severity: critical
          team: backend
        annotations:
          summary: "High error rate on {{ $labels.instance }}"
          description: "Error rate is {{ $value | humanizePercentage }} (threshold: 5%)"

      # API Server Down
      - alert: APIServerDown
        expr: up{job="stateset-api"} == 0
        for: 1m
        labels:
          severity: critical
          team: backend
        annotations:
          summary: "API server {{ $labels.instance }} is down"
          description: "API server has been down for more than 1 minute"

      # High Latency
      - alert: HighLatency
        expr: |
          histogram_quantile(0.95,
            rate(http_request_duration_ms_bucket[5m])
          ) > 1000
        for: 10m
        labels:
          severity: warning
          team: backend
        annotations:
          summary: "High API latency detected"
          description: "95th percentile latency is {{ $value }}ms (threshold: 1000ms)"

      # Database Connection Issues
      - alert: DatabaseConnectionPoolExhausted
        expr: database_connections_active / database_connections_max > 0.9
        for: 5m
        labels:
          severity: warning
          team: backend
        annotations:
          summary: "Database connection pool nearly exhausted"
          description: "{{ $value | humanizePercentage }} of connections in use"

      # High Memory Usage
      - alert: HighMemoryUsage
        expr: |
          (
            process_resident_memory_bytes
            /
            container_spec_memory_limit_bytes
          ) > 0.9
        for: 5m
        labels:
          severity: warning
          team: backend
        annotations:
          summary: "High memory usage on {{ $labels.instance }}"
          description: "Memory usage is {{ $value | humanizePercentage }}"

      # Cache Miss Rate Too High
      - alert: HighCacheMissRate
        expr: |
          (
            sum(rate(cache_misses_total[5m]))
            /
            (sum(rate(cache_hits_total[5m])) + sum(rate(cache_misses_total[5m])))
          ) > 0.5
        for: 10m
        labels:
          severity: warning
          team: backend
        annotations:
          summary: "High cache miss rate"
          description: "Cache miss rate is {{ $value | humanizePercentage }}"

      # Low Success Rate
      - alert: LowSuccessRate
        expr: |
          (
            sum(rate(http_requests_total{status=~"2.."}[5m]))
            /
            sum(rate(http_requests_total[5m]))
          ) < 0.95
        for: 5m
        labels:
          severity: warning
          team: backend
        annotations:
          summary: "Low success rate"
          description: "Success rate is {{ $value | humanizePercentage }}"

      # Rate Limit Exceeded Frequently
      - alert: FrequentRateLimiting
        expr: rate(rate_limit_denied_total[5m]) > 10
        for: 10m
        labels:
          severity: info
          team: backend
        annotations:
          summary: "Frequent rate limiting on {{ $labels.path }}"
          description: "{{ $value }} requests/sec are being rate limited"
```

### AlertManager Configuration

**alertmanager.yml**:

```yaml
global:
  resolve_timeout: 5m
  slack_api_url: 'YOUR_SLACK_WEBHOOK_URL'

route:
  group_by: ['alertname', 'cluster', 'service']
  group_wait: 10s
  group_interval: 10s
  repeat_interval: 12h
  receiver: 'default'
  routes:
    - match:
        severity: critical
      receiver: 'pagerduty'
      continue: true
    - match:
        severity: warning
      receiver: 'slack'

receivers:
  - name: 'default'
    slack_configs:
      - channel: '#alerts'
        title: 'StateSet API Alert'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

  - name: 'slack'
    slack_configs:
      - channel: '#backend-alerts'
        title: '{{ .GroupLabels.alertname }}'
        text: '{{ range .Alerts }}{{ .Annotations.description }}{{ end }}'

  - name: 'pagerduty'
    pagerduty_configs:
      - service_key: 'YOUR_PAGERDUTY_KEY'
```

---

## Dashboards

### Grafana Dashboard

Import these pre-built dashboards or create custom ones:

#### API Overview Dashboard

Panels:
1. **Request Rate** (RPS)
2. **Error Rate** (%)
3. **Latency** (p50, p95, p99)
4. **Success Rate** (%)
5. **Active Connections**
6. **Top Endpoints** (by request count)
7. **Slowest Endpoints** (by avg latency)
8. **Status Code Distribution**

#### Business Metrics Dashboard

Panels:
1. **Orders Created** (over time)
2. **Returns Processed** (over time)
3. **Shipments Created** (over time)
4. **Revenue** (if tracked)
5. **Active Users**
6. **Inventory Adjustments**

#### System Health Dashboard

Panels:
1. **CPU Usage**
2. **Memory Usage**
3. **Disk I/O**
4. **Network Traffic**
5. **Database Connections**
6. **Cache Hit Rate**
7. **Queue Length**

### Example Dashboard JSON

See `docs/grafana-dashboards/` for complete dashboard definitions.

---

## Performance Monitoring

### Key Performance Indicators (KPIs)

| Metric | Target | Critical Threshold |
|--------|--------|--------------------|
| Availability | 99.9% | 99.0% |
| P95 Latency | < 500ms | > 1000ms |
| P99 Latency | < 1000ms | > 2000ms |
| Error Rate | < 1% | > 5% |
| Success Rate | > 99% | < 95% |

### Application Performance Monitoring (APM)

Consider using APM tools:
- **Datadog APM**
- **New Relic**
- **Elastic APM**
- **Grafana Tempo** (open source)

### Synthetic Monitoring

Set up synthetic checks to monitor API from external locations:

```yaml
# Uptime check
GET /health
Expected: 200 OK

# Functional check
POST /api/v1/auth/login
Body: { "email": "test@example.com", "password": "test" }
Expected: 200 OK with access_token
```

---

## Best Practices

1. **Use Structured Logging**: Always log in JSON format for easy parsing
2. **Include Request IDs**: Track requests across services
3. **Monitor Business Metrics**: Not just technical metrics
4. **Set Realistic Alerts**: Avoid alert fatigue
5. **Dashboard for Each Team**: Create role-specific dashboards
6. **Regular Review**: Review and update alerts monthly
7. **Document Runbooks**: Link alerts to runbooks
8. **Test Alerts**: Periodically test alert firing
9. **Aggregate Logs**: Don't rely on individual server logs
10. **Trace Critical Paths**: Focus on high-value transactions

---

## Troubleshooting with Monitoring

### High Latency

1. Check slow query dashboard
2. Review trace for bottlenecks
3. Check database connection pool
4. Review cache hit rate
5. Check external API latency

### High Error Rate

1. Check error logs for patterns
2. Review recent deployments
3. Check database connectivity
4. Review rate limiting
5. Check dependencies

### Memory Leak

1. Check memory usage trend
2. Enable heap profiling
3. Review recent code changes
4. Check for goroutine leaks
5. Review cache size

---

## Additional Resources

- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [Grafana Tutorials](https://grafana.com/tutorials/)
- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)
- [The Four Golden Signals](https://sre.google/sre-book/monitoring-distributed-systems/)

---

## Support

For monitoring-related questions:
- GitHub Issues: https://github.com/stateset/stateset-api/issues
- Email: monitoring@stateset.io
