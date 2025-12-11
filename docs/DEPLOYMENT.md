# Production Deployment Guide

This guide covers deploying StateSet API to production environments.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Environment Variables](#environment-variables)
- [Deployment Options](#deployment-options)
  - [Docker](#docker)
  - [Kubernetes](#kubernetes)
  - [AWS ECS](#aws-ecs)
  - [Bare Metal](#bare-metal)
- [Database Setup](#database-setup)
- [Load Balancing](#load-balancing)
- [TLS/SSL Configuration](#tlsssl-configuration)
- [Monitoring & Alerts](#monitoring--alerts)
- [Backup & Disaster Recovery](#backup--disaster-recovery)
- [Security Hardening](#security-hardening)
- [Performance Tuning](#performance-tuning)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

Before deploying to production, ensure you have:

- **Database**: PostgreSQL 14+ (recommended) or SQLite for small deployments
- **Cache**: Redis 6+ for caching and rate limiting
- **Resources**:
  - Minimum: 2 CPU cores, 4GB RAM
  - Recommended: 4+ CPU cores, 8GB+ RAM
- **Domain**: Registered domain with DNS configured
- **TLS Certificates**: Valid SSL/TLS certificates
- **Monitoring**: Prometheus-compatible monitoring system

---

## Environment Variables

### Required Variables

```bash
# Database Configuration
APP__DATABASE_URL=postgres://user:password@localhost:5432/stateset

# JWT Authentication
APP__JWT_SECRET=your-secure-random-secret-min-64-chars
APP__JWT_ACCESS_EXPIRATION=900        # 15 minutes in seconds
APP__JWT_REFRESH_EXPIRATION=604800    # 7 days in seconds

# Redis Configuration
# Use `rediss://` for TLS-enabled Redis.
APP__REDIS_URL=redis://localhost:6379/0

# Server Configuration
APP__HOST=0.0.0.0
APP__PORT=8080
APP__ENVIRONMENT=production
```

### Optional but Recommended

```bash
# Rate Limiting
APP__RATE_LIMIT_REQUESTS_PER_WINDOW=1000
APP__RATE_LIMIT_WINDOW_SECONDS=60
APP__RATE_LIMIT_ENABLE_HEADERS=true

# CORS Configuration
APP__CORS_ALLOWED_ORIGINS=https://yourdomain.com,https://app.yourdomain.com

# Database Pool
APP__DATABASE_MAX_CONNECTIONS=100
APP__DATABASE_MIN_CONNECTIONS=10
APP__DATABASE_ACQUIRE_TIMEOUT=30
APP__DATABASE_IDLE_TIMEOUT=600

# Logging
APP__LOG_LEVEL=info
APP__LOG_FORMAT=json

# OpenTelemetry (optional)
APP__OTEL_ENABLED=true
APP__OTEL_ENDPOINT=http://localhost:4317
APP__OTEL_SERVICE_NAME=stateset-api

# Auto-migration (set to false in production)
APP__AUTO_MIGRATE=false
```

### Security Variables

```bash
# API Keys Encryption (required for API key feature)
APP__API_KEY_ENCRYPTION_KEY=your-32-byte-encryption-key

# Password Policy
APP__PASSWORD_MIN_LENGTH=12
APP__PASSWORD_REQUIRE_UPPERCASE=true
APP__PASSWORD_REQUIRE_LOWERCASE=true
APP__PASSWORD_REQUIRE_NUMBERS=true
APP__PASSWORD_REQUIRE_SPECIAL=true

# Session Security
APP__SECURE_COOKIES=true
APP__SAME_SITE=strict
```

---

## Deployment Options

### Docker

#### Single Container

```bash
# Build the image
docker build -t stateset-api:latest .

# Run the container
docker run -d \
  --name stateset-api \
  -p 8080:8080 \
  --env-file .env.production \
  --restart unless-stopped \
  stateset-api:latest
```

#### Docker Compose (with PostgreSQL and Redis)

```yaml
# docker-compose.production.yml
version: '3.8'

services:
  api:
    image: stateset-api:latest
    build: .
    ports:
      - "8080:8080"
    environment:
      - APP__DATABASE_URL=postgres://stateset:${DB_PASSWORD}@postgres:5432/stateset
      - APP__REDIS_URL=redis://redis:6379/0
      - APP__JWT_SECRET=${JWT_SECRET}
      - APP__ENVIRONMENT=production
    depends_on:
      - postgres
      - redis
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s

  postgres:
    image: postgres:15-alpine
    environment:
      - POSTGRES_DB=stateset
      - POSTGRES_USER=stateset
      - POSTGRES_PASSWORD=${DB_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./backups:/backups
    restart: unless-stopped
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U stateset"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    command: redis-server --appendonly yes
    volumes:
      - redis_data:/data
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 5s
      retries: 5

volumes:
  postgres_data:
  redis_data:
```

Deploy:

```bash
docker-compose -f docker-compose.production.yml up -d
```

### Kubernetes

#### Deployment Manifest

```yaml
# k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: stateset-api
  labels:
    app: stateset-api
spec:
  replicas: 3
  selector:
    matchLabels:
      app: stateset-api
  template:
    metadata:
      labels:
        app: stateset-api
    spec:
      containers:
      - name: api
        image: stateset-api:latest
        ports:
        - containerPort: 8080
        env:
        - name: APP__DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: stateset-secrets
              key: database-url
        - name: APP__JWT_SECRET
          valueFrom:
            secretKeyRef:
              name: stateset-secrets
              key: jwt-secret
        - name: APP__REDIS_URL
          valueFrom:
            secretKeyRef:
              name: stateset-secrets
              key: redis-url
        - name: APP__ENVIRONMENT
          value: "production"
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "2Gi"
            cpu: "2000m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /health/readiness
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: stateset-api
spec:
  selector:
    app: stateset-api
  ports:
  - protocol: TCP
    port: 80
    targetPort: 8080
  type: LoadBalancer
---
apiVersion: v1
kind: Secret
metadata:
  name: stateset-secrets
type: Opaque
stringData:
  database-url: postgres://user:password@postgres-service:5432/stateset
  jwt-secret: your-jwt-secret-here
  redis-url: redis://redis-service:6379/0
```

Deploy:

```bash
kubectl apply -f k8s/deployment.yaml
```

### AWS ECS

#### Task Definition

```json
{
  "family": "stateset-api",
  "networkMode": "awsvpc",
  "requiresCompatibilities": ["FARGATE"],
  "cpu": "1024",
  "memory": "2048",
  "containerDefinitions": [
    {
      "name": "stateset-api",
      "image": "your-ecr-repo/stateset-api:latest",
      "portMappings": [
        {
          "containerPort": 8080,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {
          "name": "APP__ENVIRONMENT",
          "value": "production"
        }
      ],
      "secrets": [
        {
          "name": "APP__DATABASE_URL",
          "valueFrom": "arn:aws:secretsmanager:region:account:secret:database-url"
        },
        {
          "name": "APP__JWT_SECRET",
          "valueFrom": "arn:aws:secretsmanager:region:account:secret:jwt-secret"
        }
      ],
      "logConfiguration": {
        "logDriver": "awslogs",
        "options": {
          "awslogs-group": "/ecs/stateset-api",
          "awslogs-region": "us-east-1",
          "awslogs-stream-prefix": "ecs"
        }
      },
      "healthCheck": {
        "command": ["CMD-SHELL", "curl -f http://localhost:8080/health || exit 1"],
        "interval": 30,
        "timeout": 5,
        "retries": 3,
        "startPeriod": 60
      }
    }
  ]
}
```

### Bare Metal

#### Systemd Service

```ini
# /etc/systemd/system/stateset-api.service
[Unit]
Description=StateSet API Server
After=network.target postgresql.service redis.service

[Service]
Type=simple
User=stateset
Group=stateset
WorkingDirectory=/opt/stateset-api
EnvironmentFile=/opt/stateset-api/.env.production
ExecStart=/opt/stateset-api/stateset-api
Restart=on-failure
RestartSec=10
StandardOutput=journal
StandardError=journal
SyslogIdentifier=stateset-api

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/stateset-api/data

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable stateset-api
sudo systemctl start stateset-api
sudo systemctl status stateset-api
```

---

## Database Setup

### PostgreSQL Production Configuration

```bash
# Run migrations
cargo run --bin migration

# Or using Docker
docker run --rm \
  -e APP__DATABASE_URL="postgres://user:pass@host:5432/stateset" \
  stateset-api:latest \
  /app/migration
```

### Database Tuning (postgresql.conf)

```ini
# Memory
shared_buffers = 4GB
effective_cache_size = 12GB
maintenance_work_mem = 1GB
work_mem = 64MB

# Connections
max_connections = 200

# WAL
wal_buffers = 16MB
checkpoint_completion_target = 0.9

# Query Planner
random_page_cost = 1.1  # For SSD
effective_io_concurrency = 200
```

---

## Load Balancing

### Nginx Configuration

```nginx
upstream stateset_api {
    least_conn;
    server 10.0.1.10:8080 max_fails=3 fail_timeout=30s;
    server 10.0.1.11:8080 max_fails=3 fail_timeout=30s;
    server 10.0.1.12:8080 max_fails=3 fail_timeout=30s;
}

server {
    listen 443 ssl http2;
    server_name api.stateset.com;

    ssl_certificate /etc/ssl/certs/stateset.crt;
    ssl_certificate_key /etc/ssl/private/stateset.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;
    ssl_prefer_server_ciphers on;

    # Security Headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
    add_header X-Frame-Options "DENY" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    location / {
        proxy_pass http://stateset_api;
        proxy_http_version 1.1;

        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;

        # Health check
        proxy_next_upstream error timeout invalid_header http_500 http_502 http_503;
    }

    # Metrics endpoint (restrict access)
    location /metrics {
        allow 10.0.0.0/8;  # Internal network only
        deny all;
        proxy_pass http://stateset_api;
    }
}

# HTTP to HTTPS redirect
server {
    listen 80;
    server_name api.stateset.com;
    return 301 https://$server_name$request_uri;
}
```

---

## TLS/SSL Configuration

### Using Let's Encrypt

```bash
# Install certbot
sudo apt-get install certbot python3-certbot-nginx

# Obtain certificate
sudo certbot --nginx -d api.stateset.com

# Auto-renewal
sudo certbot renew --dry-run
```

### Certificate in Kubernetes (cert-manager)

```yaml
apiVersion: cert-manager.io/v1
kind: Certificate
metadata:
  name: stateset-api-tls
spec:
  secretName: stateset-api-tls
  issuerRef:
    name: letsencrypt-prod
    kind: ClusterIssuer
  dnsNames:
  - api.stateset.com
```

---

## Monitoring & Alerts

### Prometheus Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'stateset-api'
    scrape_interval: 15s
    static_configs:
      - targets: ['api-server-1:8080', 'api-server-2:8080', 'api-server-3:8080']
    metrics_path: '/metrics'
```

### Alert Rules

```yaml
# alerts.yml
groups:
  - name: stateset-api
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value }} errors/sec"

      - alert: APIDown
        expr: up{job="stateset-api"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "API server is down"

      - alert: HighLatency
        expr: histogram_quantile(0.95, rate(http_request_duration_ms_bucket[5m])) > 1000
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High API latency detected"
          description: "95th percentile latency is {{ $value }}ms"
```

---

## Backup & Disaster Recovery

See [DATABASE.md](DATABASE.md) for detailed backup procedures.

### Quick Backup Script

```bash
#!/bin/bash
# backup.sh

BACKUP_DIR="/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# PostgreSQL backup
pg_dump -h localhost -U stateset stateset | gzip > "$BACKUP_DIR/stateset_$TIMESTAMP.sql.gz"

# Redis backup
redis-cli --rdb "$BACKUP_DIR/dump_$TIMESTAMP.rdb"

# Upload to S3 (optional)
aws s3 cp "$BACKUP_DIR/stateset_$TIMESTAMP.sql.gz" s3://stateset-backups/

# Cleanup old backups (keep last 30 days)
find $BACKUP_DIR -name "stateset_*.sql.gz" -mtime +30 -delete
```

---

## Security Hardening

### Firewall Rules

```bash
# Allow only necessary ports
ufw allow 22/tcp    # SSH
ufw allow 443/tcp   # HTTPS
ufw allow 80/tcp    # HTTP (for redirect)
ufw enable
```

### Application Security

1. **Enable all security headers** (done in Nginx config above)
2. **Use strong JWT secrets** (min 64 characters)
3. **Enable rate limiting** (configured in env vars)
4. **Restrict metrics endpoint** (Nginx config)
5. **Use separate database user** with limited privileges
6. **Enable Redis authentication**
7. **Regular security audits**: `cargo audit`

---

## Performance Tuning

### Rust Build Optimization

```bash
# Build with optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

### Connection Pool Tuning

```bash
# Adjust based on your traffic
APP__DATABASE_MAX_CONNECTIONS=100
APP__DATABASE_MIN_CONNECTIONS=10
```

### Redis Tuning

```bash
# redis.conf
maxmemory 2gb
maxmemory-policy allkeys-lru
```

---

## Troubleshooting

### High Memory Usage

```bash
# Check memory usage
docker stats stateset-api

# Adjust container limits
docker update --memory 4g stateset-api
```

### Database Connection Issues

```bash
# Check connections
SELECT count(*) FROM pg_stat_activity WHERE datname = 'stateset';

# Kill idle connections
SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE state = 'idle' AND state_change < now() - interval '1 hour';
```

### API Not Responding

```bash
# Check logs
journalctl -u stateset-api -f

# Check health
curl http://localhost:8080/health

# Check database
curl http://localhost:8080/health/readiness
```

### Performance Issues

```bash
# Enable query logging (temporarily)
APP__LOG_LEVEL=debug

# Check slow queries in PostgreSQL
SELECT query, mean_exec_time FROM pg_stat_statements ORDER BY mean_exec_time DESC LIMIT 10;
```

---

## Rollback Procedure

If deployment fails:

1. **Stop new version**:
   ```bash
   kubectl rollout undo deployment/stateset-api
   ```

2. **Restore database** (if schema changed):
   ```bash
   psql stateset < /backups/stateset_pre_deploy.sql
   ```

3. **Clear Redis cache**:
   ```bash
   redis-cli FLUSHDB
   ```

4. **Verify health**:
   ```bash
   curl https://api.stateset.com/health
   ```

---

## Checklist Before Going Live

- [ ] All environment variables configured
- [ ] Database migrations run successfully
- [ ] TLS/SSL certificates installed and valid
- [ ] Load balancer configured and tested
- [ ] Monitoring and alerts set up
- [ ] Backup procedure tested
- [ ] Disaster recovery plan documented
- [ ] Security scan completed (`cargo audit`)
- [ ] Load testing completed
- [ ] Health checks returning 200
- [ ] DNS configured correctly
- [ ] Firewall rules applied
- [ ] Log aggregation configured
- [ ] Team trained on deployment procedures
- [ ] Rollback procedure tested

---

## Support

For deployment issues:
- Open an issue: https://github.com/stateset/stateset-api/issues
- Email: support@stateset.io
- Documentation: https://docs.stateset.com
