# StateSet API - Docker Development Environment

Complete Docker Compose setup for local development and testing of the StateSet API.

## What's Included

This Docker Compose configuration provides a complete local development environment:

- **StateSet API Server** - The main API service
- **PostgreSQL 15** - Primary database
- **Redis 7** - Caching and rate limiting
- **Adminer** - Database management UI (http://localhost:8081)
- **RedisInsight** - Redis management UI (http://localhost:8001)
- **MailHog** - Email testing server (http://localhost:8025)

## Quick Start

### Prerequisites

- Docker Desktop (v20.10+)
- Docker Compose (v2.0+)
- 4GB+ RAM allocated to Docker

### Start the Environment

```bash
# From the examples directory
docker-compose -f docker-compose.dev.yml up

# Or run in detached mode
docker-compose -f docker-compose.dev.yml up -d

# View logs
docker-compose -f docker-compose.dev.yml logs -f api
```

### Stop the Environment

```bash
# Stop all services
docker-compose -f docker-compose.dev.yml down

# Stop and remove volumes (clears all data)
docker-compose -f docker-compose.dev.yml down -v
```

## Service URLs

Once started, you can access:

- **API Server**: http://localhost:8080
  - REST API: http://localhost:8080/api/v1
  - Health Check: http://localhost:8080/health
  - Metrics: http://localhost:8080/metrics

- **gRPC Server**: localhost:50051

- **Database UI (Adminer)**: http://localhost:8081
  - System: PostgreSQL
  - Server: postgres
  - Username: stateset
  - Password: stateset_dev_password
  - Database: stateset

- **Redis UI (RedisInsight)**: http://localhost:8001
  - Host: redis
  - Port: 6379
  - Password: redis_dev_password

- **Email UI (MailHog)**: http://localhost:8025
  - SMTP: localhost:1025

## Database Management

### Run Migrations

```bash
# Run migrations
docker-compose -f docker-compose.dev.yml exec api cargo run --bin migration

# Or if migrations are automatic on startup
# Just restart the api service
docker-compose -f docker-compose.dev.yml restart api
```

### Connect to PostgreSQL

```bash
# Using psql in the container
docker-compose -f docker-compose.dev.yml exec postgres psql -U stateset -d stateset

# Or connect from your local machine
psql -h localhost -p 5432 -U stateset -d stateset
```

### Backup Database

```bash
# Create backup
docker-compose -f docker-compose.dev.yml exec postgres pg_dump -U stateset stateset > backup.sql

# Restore backup
docker-compose -f docker-compose.dev.yml exec -T postgres psql -U stateset stateset < backup.sql
```

## Redis Management

### Connect to Redis

```bash
# Using redis-cli in the container
docker-compose -f docker-compose.dev.yml exec redis redis-cli -a redis_dev_password

# Common commands
> PING
> INFO
> KEYS *
> GET some_key
> FLUSHALL  # Clear all keys (use with caution!)
```

### Monitor Redis

```bash
# Monitor all commands in real-time
docker-compose -f docker-compose.dev.yml exec redis redis-cli -a redis_dev_password MONITOR
```

## Development Workflows

### Hot Reload Development

The API service mounts the source code, so you can make changes and rebuild:

```bash
# Make code changes in your editor

# Rebuild and restart the API
docker-compose -f docker-compose.dev.yml restart api

# Or rebuild from scratch
docker-compose -f docker-compose.dev.yml up -d --build api
```

### Run Tests

```bash
# Run all tests
docker-compose -f docker-compose.dev.yml exec api cargo test

# Run specific test
docker-compose -f docker-compose.dev.yml exec api cargo test test_name

# Run with output
docker-compose -f docker-compose.dev.yml exec api cargo test -- --nocapture
```

### Check Logs

```bash
# Follow logs for all services
docker-compose -f docker-compose.dev.yml logs -f

# Follow logs for specific service
docker-compose -f docker-compose.dev.yml logs -f api
docker-compose -f docker-compose.dev.yml logs -f postgres
docker-compose -f docker-compose.dev.yml logs -f redis

# Show last 100 lines
docker-compose -f docker-compose.dev.yml logs --tail=100 api
```

## Testing the API

### Using cURL

```bash
# Health check
curl http://localhost:8080/health

# Register a user
curl -X POST http://localhost:8080/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "SecurePass123!",
    "first_name": "Test",
    "last_name": "User"
  }'

# Login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "email": "test@example.com",
    "password": "SecurePass123!"
  }'

# Save the access token
TOKEN="<access_token_from_login>"

# List orders
curl http://localhost:8080/api/v1/orders \
  -H "Authorization: Bearer $TOKEN"
```

### Using the Examples

All the example clients in this directory can connect to the Docker environment:

```bash
# TypeScript/Node.js
cd examples
npm install
ts-node typescript-example.ts

# Python
pip install requests
python python-example.py

# Go
go run go-example.go

# Ruby
gem install httparty
ruby ruby-example.rb

# PHP (requires composer)
composer install
php php-example.php
```

## Customization

### Environment Variables

Edit `docker-compose.dev.yml` to change:

- Database credentials
- Redis password
- JWT secrets
- API ports
- CORS origins
- Rate limiting settings

### Add Custom Services

Add your own services to the Docker Compose file:

```yaml
  myservice:
    image: myimage:latest
    ports:
      - "3000:3000"
    environment:
      API_URL: http://api:8080
    networks:
      - stateset-network
    depends_on:
      - api
```

## Troubleshooting

### Port Already in Use

If you get "port already in use" errors:

```bash
# Check what's using the port
lsof -i :8080
lsof -i :5432
lsof -i :6379

# Kill the process or change the port in docker-compose.dev.yml
```

### Container Won't Start

```bash
# Check container logs
docker-compose -f docker-compose.dev.yml logs api

# Check container status
docker-compose -f docker-compose.dev.yml ps

# Rebuild from scratch
docker-compose -f docker-compose.dev.yml down -v
docker-compose -f docker-compose.dev.yml build --no-cache
docker-compose -f docker-compose.dev.yml up
```

### Database Connection Issues

```bash
# Check if PostgreSQL is ready
docker-compose -f docker-compose.dev.yml exec postgres pg_isready

# Check database logs
docker-compose -f docker-compose.dev.yml logs postgres

# Restart PostgreSQL
docker-compose -f docker-compose.dev.yml restart postgres
```

### Clear All Data

```bash
# Stop and remove everything including volumes
docker-compose -f docker-compose.dev.yml down -v

# Remove specific volume
docker volume rm examples_postgres_data
docker volume rm examples_redis_data
```

## Performance Optimization

### Allocate More Resources

In Docker Desktop:
1. Go to Settings → Resources
2. Increase CPUs to 4+
3. Increase Memory to 8GB+
4. Click "Apply & Restart"

### Use BuildKit

```bash
# Enable BuildKit for faster builds
export DOCKER_BUILDKIT=1
export COMPOSE_DOCKER_CLI_BUILD=1

# Rebuild
docker-compose -f docker-compose.dev.yml build
```

### Cache Rust Dependencies

The Docker Compose file already mounts `target/` for caching. For faster builds:

```bash
# Pre-build dependencies
docker-compose -f docker-compose.dev.yml run --rm api cargo build --release
```

## Production Differences

This development environment differs from production:

- Uses weaker secrets (change in production!)
- Exposes all ports (restrict in production)
- Includes development tools (Adminer, RedisInsight)
- Runs in debug mode (use release in production)
- No HTTPS/TLS (required in production)
- No load balancing (add in production)
- No backup strategy (implement in production)

See `../docs/DEPLOYMENT.md` for production setup.

## Additional Resources

- [Main README](../README.md)
- [API Examples](./README.md)
- [Advanced Workflows](./ADVANCED_WORKFLOWS.md)
- [Webhook Handlers](./WEBHOOK_HANDLERS.md)
- [Deployment Guide](../docs/DEPLOYMENT.md)

## Support

If you encounter issues:

1. Check the troubleshooting section above
2. Review Docker logs: `docker-compose -f docker-compose.dev.yml logs`
3. Check GitHub issues: https://github.com/stateset/stateset-api/issues
4. Ask in GitHub Discussions: https://github.com/stateset/stateset-api/discussions
