# Database Guide

This guide covers database management, migrations, backups, and disaster recovery for StateSet API.

## Table of Contents

- [Database Overview](#database-overview)
- [Supported Databases](#supported-databases)
- [Schema Management](#schema-management)
- [Migrations](#migrations)
- [Backup & Restore](#backup--restore)
- [Performance Optimization](#performance-optimization)
- [Disaster Recovery](#disaster-recovery)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

---

## Database Overview

StateSet API uses **SeaORM** as the database abstraction layer, supporting both PostgreSQL and SQLite.

### Current Schema

The database includes tables for:

- **Authentication**: users, api_keys, refresh_tokens, password_reset_tokens
- **Orders**: orders, order_items, fulfillment_orders
- **Inventory**: inventory, inventory_allocations, inventory_reservations, inventory_transactions
- **Returns**: returns, return_items, return_approvals
- **Shipments**: shipments, shipment_items, shipment_tracking
- **Warranties**: warranties, warranty_claims
- **Manufacturing**: bill_of_materials, bom_line_items, work_orders, work_order_operations
- **Procurement**: purchase_orders, purchase_order_items, advanced_shipping_notices
- **Financial**: cash_sales, invoices, payments, item_receipts
- **Commerce**: products, product_variants, carts, cart_items, customers, addresses
- **System**: events, outbox, audit_log

---

## Supported Databases

### PostgreSQL (Recommended for Production)

**Minimum Version**: PostgreSQL 12+
**Recommended Version**: PostgreSQL 15+

**Advantages**:
- Production-grade reliability
- Advanced features (JSONB, full-text search, etc.)
- Better performance for large datasets
- Excellent tooling and monitoring

**Connection String**:
```bash
APP__DATABASE_URL=postgres://username:password@localhost:5432/stateset
```

### SQLite (Development Only)

**Minimum Version**: SQLite 3.35+

**Advantages**:
- Zero configuration
- File-based (portable)
- Perfect for local development

**Connection String**:
```bash
APP__DATABASE_URL=sqlite://stateset.db?mode=rwc
```

⚠️ **Not recommended for production** due to:
- Limited concurrency
- No network access
- Limited scalability

---

## Schema Management

### Viewing Current Schema

```bash
# PostgreSQL
psql -U stateset -d stateset -c "\dt"

# SQLite
sqlite3 stateset.db ".tables"
```

### Entity Relationship Diagram

```
┌─────────┐       ┌──────────────┐       ┌──────────┐
│  User   │──────>│    Order     │──────>│  Product │
└─────────┘       └──────────────┘       └──────────┘
                        │
                        │
                        ▼
                  ┌────────────┐
                  │ OrderItem  │
                  └────────────┘
                        │
                        │
                        ▼
                  ┌────────────┐
                  │ Inventory  │
                  └────────────┘
```

---

## Migrations

### Running Migrations

**Automatic (Development)**:
```bash
# Set in config
APP__AUTO_MIGRATE=true
cargo run
```

**Manual (Production - Recommended)**:
```bash
# Using the migration binary
cargo run --bin migration

# Or using Docker
docker run --rm \
  -e APP__DATABASE_URL="$DATABASE_URL" \
  stateset-api:latest \
  /app/migration
```

### Creating New Migrations

1. **Add migration in `migrations/src/lib.rs`**:

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(YourNewTable::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(YourNewTable::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(YourNewTable::Name).string().not_null())
                    .col(
                        ColumnDef::new(YourNewTable::CreatedAt)
                            .timestamp()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(YourNewTable::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum YourNewTable {
    Table,
    Id,
    Name,
    CreatedAt,
}
```

2. **Register migration**:

```rust
// In migrations/src/lib.rs
pub use m20240101_000001_create_your_table::Migration as M20240101CreateYourTable;

// Add to MigratorTrait implementation
vec![
    // ... existing migrations
    Box::new(M20240101CreateYourTable),
]
```

3. **Test migration**:

```bash
# Apply
cargo run --bin migration up

# Rollback
cargo run --bin migration down

# Check status
cargo run --bin migration status
```

### Migration Best Practices

1. **Always test migrations** on a copy of production data
2. **Make migrations reversible** (implement `down()`)
3. **Use transactions** when possible
4. **Add indexes** for foreign keys and frequently queried columns
5. **Document breaking changes** in CHANGELOG.md
6. **Backup before migrating** production

---

## Backup & Restore

### Automated Backup Script

```bash
#!/bin/bash
# scripts/backup.sh

set -e

BACKUP_DIR="/backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RETENTION_DAYS=30

# Create backup directory
mkdir -p $BACKUP_DIR

# PostgreSQL Backup
if [ "$DB_TYPE" = "postgres" ]; then
    echo "Starting PostgreSQL backup..."

    # Full backup
    pg_dump \
        -h $DB_HOST \
        -U $DB_USER \
        -d $DB_NAME \
        --format=custom \
        --file="$BACKUP_DIR/stateset_$TIMESTAMP.dump"

    # Compress
    gzip "$BACKUP_DIR/stateset_$TIMESTAMP.dump"

    echo "Backup created: stateset_$TIMESTAMP.dump.gz"
fi

# SQLite Backup
if [ "$DB_TYPE" = "sqlite" ]; then
    echo "Starting SQLite backup..."
    sqlite3 stateset.db ".backup '$BACKUP_DIR/stateset_$TIMESTAMP.db'"
    gzip "$BACKUP_DIR/stateset_$TIMESTAMP.db"
fi

# Upload to S3 (optional)
if [ -n "$AWS_S3_BUCKET" ]; then
    echo "Uploading to S3..."
    aws s3 cp \
        "$BACKUP_DIR/stateset_$TIMESTAMP.dump.gz" \
        "s3://$AWS_S3_BUCKET/backups/$(date +%Y/%m/%d)/"
fi

# Cleanup old backups
echo "Cleaning up old backups..."
find $BACKUP_DIR -name "stateset_*.dump.gz" -mtime +$RETENTION_DAYS -delete
find $BACKUP_DIR -name "stateset_*.db.gz" -mtime +$RETENTION_DAYS -delete

echo "Backup completed successfully!"
```

### Cron Schedule

```bash
# Edit crontab
crontab -e

# Daily backup at 2 AM
0 2 * * * /opt/stateset-api/scripts/backup.sh >> /var/log/stateset-backup.log 2>&1

# Weekly full backup on Sunday at 3 AM
0 3 * * 0 /opt/stateset-api/scripts/backup-full.sh >> /var/log/stateset-backup.log 2>&1
```

### Restore from Backup

**PostgreSQL**:

```bash
# Drop and recreate database (CAUTION!)
dropdb stateset
createdb stateset

# Restore from custom format
pg_restore \
    -h localhost \
    -U stateset \
    -d stateset \
    --verbose \
    /backups/stateset_20241103_020000.dump

# Or from SQL dump
gunzip < /backups/stateset_20241103_020000.sql.gz | psql -U stateset -d stateset
```

**SQLite**:

```bash
# Simply replace the database file
gunzip < /backups/stateset_20241103_020000.db.gz > stateset.db
```

### Point-in-Time Recovery (PostgreSQL)

Enable WAL archiving in `postgresql.conf`:

```ini
wal_level = replica
archive_mode = on
archive_command = 'cp %p /var/lib/postgresql/wal_archive/%f'
```

Perform PITR:

```bash
# Stop PostgreSQL
sudo systemctl stop postgresql

# Restore base backup
pg_restore -d stateset /backups/base_backup.dump

# Create recovery.conf
cat > /var/lib/postgresql/data/recovery.conf << EOF
restore_command = 'cp /var/lib/postgresql/wal_archive/%f %p'
recovery_target_time = '2024-11-03 14:30:00'
EOF

# Start PostgreSQL (will replay WAL to target time)
sudo systemctl start postgresql
```

---

## Performance Optimization

### Indexing Strategy

**Create indexes for**:
- Primary keys (automatic)
- Foreign keys
- Frequently queried columns
- WHERE clause columns
- ORDER BY columns

```sql
-- Example: Create index for order lookups by customer
CREATE INDEX idx_orders_customer_id ON orders(customer_id);

-- Create index for date range queries
CREATE INDEX idx_orders_created_at ON orders(created_at);

-- Composite index for common query patterns
CREATE INDEX idx_orders_status_created ON orders(status, created_at DESC);

-- Partial index for active orders only
CREATE INDEX idx_orders_active ON orders(customer_id) WHERE status IN ('pending', 'processing');
```

### Query Optimization

**Enable query analysis**:

```sql
-- PostgreSQL
EXPLAIN ANALYZE SELECT * FROM orders WHERE customer_id = 'xxx';

-- Check slow queries
SELECT
    query,
    calls,
    mean_exec_time,
    max_exec_time
FROM pg_stat_statements
ORDER BY mean_exec_time DESC
LIMIT 10;
```

### Connection Pooling

Configure optimal pool size:

```bash
# Rule of thumb: (CPU cores * 2) + disk spindles
APP__DATABASE_MAX_CONNECTIONS=20
APP__DATABASE_MIN_CONNECTIONS=5
APP__DATABASE_ACQUIRE_TIMEOUT=30
APP__DATABASE_IDLE_TIMEOUT=600
```

### Vacuum and Analyze (PostgreSQL)

```bash
# Manual vacuum
VACUUM ANALYZE;

# Auto-vacuum configuration (postgresql.conf)
autovacuum = on
autovacuum_analyze_scale_factor = 0.05
autovacuum_vacuum_scale_factor = 0.1
```

---

## Disaster Recovery

### Recovery Time Objective (RTO)

**Target**: < 1 hour

**Procedure**:
1. Spin up new database instance (10 min)
2. Restore from latest backup (20 min)
3. Apply any WAL logs if using PITR (10 min)
4. Update application connection string (5 min)
5. Verify data integrity (10 min)
6. Resume operations (5 min)

### Recovery Point Objective (RPO)

**Target**: < 5 minutes

**Strategy**:
- Continuous WAL archiving to S3
- Automated backups every 4 hours
- Transaction log shipping to standby

### Disaster Recovery Plan

1. **Detection**: Monitoring alerts of database unavailability
2. **Assessment**: Determine extent of failure
3. **Failover**: Switch to standby or restore from backup
4. **Verification**: Run health checks and sample queries
5. **Communication**: Notify stakeholders
6. **Post-mortem**: Document incident and improve procedures

### High Availability Setup

**Primary-Replica Configuration**:

```yaml
# Primary Database
postgresql:
  host: db-primary.stateset.internal
  replication:
    enabled: true
    replicas:
      - host: db-replica-1.stateset.internal
      - host: db-replica-2.stateset.internal
```

**Automatic Failover with Patroni**:

```yaml
scope: stateset
name: db-node-1

restapi:
  listen: 0.0.0.0:8008
  connect_address: db-node-1:8008

postgresql:
  listen: 0.0.0.0:5432
  connect_address: db-node-1:5432
  data_dir: /var/lib/postgresql/data

patroni:
  ttl: 30
  loop_wait: 10
  retry_timeout: 10
```

---

## Monitoring

### Key Metrics to Monitor

**Database Health**:
- Connection count
- Active queries
- Longest running query
- Database size
- Table sizes
- Index usage

**Performance**:
- Query latency (avg, p95, p99)
- Transactions per second
- Cache hit ratio
- Disk I/O

**Replication** (if applicable):
- Replication lag
- WAL sender/receiver status

### Prometheus Exporter

```yaml
# docker-compose.monitoring.yml
services:
  postgres-exporter:
    image: prometheuscommunity/postgres-exporter
    environment:
      DATA_SOURCE_NAME: "postgresql://stateset:password@postgres:5432/stateset?sslmode=disable"
    ports:
      - "9187:9187"
```

### Grafana Dashboard

Import dashboard ID: `9628` (PostgreSQL Database)

Or create custom queries:

```promql
# Connection count
pg_stat_database_numbackends{datname="stateset"}

# Transaction rate
rate(pg_stat_database_xact_commit{datname="stateset"}[5m])

# Cache hit ratio
pg_stat_database_blks_hit{datname="stateset"} /
(pg_stat_database_blks_hit{datname="stateset"} + pg_stat_database_blks_read{datname="stateset"})
```

---

## Troubleshooting

### Connection Errors

**Error**: `too many connections`

```sql
-- Check current connections
SELECT count(*) FROM pg_stat_activity;

-- Kill idle connections
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE state = 'idle'
AND state_change < now() - interval '1 hour';

-- Increase max_connections in postgresql.conf
max_connections = 200
```

### Slow Queries

```sql
-- Find slow queries
SELECT pid, now() - pg_stat_activity.query_start AS duration, query
FROM pg_stat_activity
WHERE state = 'active'
AND now() - pg_stat_activity.query_start > interval '5 seconds';

-- Kill a slow query
SELECT pg_terminate_backend(pid);
```

### Lock Contention

```sql
-- Find locks
SELECT
    locktype,
    relation::regclass,
    mode,
    granted,
    pid,
    pg_blocking_pids(pid) as blocked_by
FROM pg_locks
WHERE NOT granted;

-- Release locks
SELECT pg_terminate_backend(blocked_by_pid);
```

### Database Bloat

```sql
-- Check table bloat
SELECT
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename) - pg_relation_size(schemaname||'.'||tablename)) AS external_size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;

-- Vacuum full (requires exclusive lock, use with caution)
VACUUM FULL ANALYZE table_name;
```

### Migration Failures

```bash
# Check migration status
cargo run --bin migration status

# Rollback last migration
cargo run --bin migration down

# Force migration to specific version
cargo run --bin migration fresh
```

---

## Best Practices

1. **Always backup before major changes**
2. **Test migrations on staging first**
3. **Monitor query performance regularly**
4. **Use connection pooling**
5. **Enable query logging in development**
6. **Regular vacuuming (PostgreSQL)**
7. **Index foreign keys**
8. **Use prepared statements** (handled by SeaORM)
9. **Implement read replicas** for high-traffic apps
10. **Document schema changes** in migrations

---

## Additional Resources

- [SeaORM Documentation](https://www.sea-ql.org/SeaORM/)
- [PostgreSQL Performance Tuning](https://wiki.postgresql.org/wiki/Performance_Optimization)
- [Database Reliability Engineering](https://www.oreilly.com/library/view/database-reliability-engineering/9781491925935/)

---

## Support

For database-related issues:
- GitHub Issues: https://github.com/stateset/stateset-api/issues
- Email: database@stateset.io
