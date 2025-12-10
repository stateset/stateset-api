# Environment Variables

This document lists all environment variables used by the StateSet API. Copy these to your `.env` file and update with your actual values.

## Required Variables

The application loads configuration primarily from `config/*.toml`. You can override any value using environment variables with the `APP__` prefix (double underscore as separator). Examples below show both SQLite (default) and Postgres options.

### Database Configuration
```bash
# SQLite (default for development)
APP__DATABASE_URL=sqlite:stateset.db?mode=rwc

# Or PostgreSQL
APP__DATABASE_URL=postgres://username:password@localhost:5432/stateset_db
```

### Redis Configuration
```bash
# Redis connection string for caching and pub/sub
APP__REDIS_URL=redis://localhost:6379
```

### Security Configuration
```bash
# JWT secret key - MUST be changed in production
# Generate with: openssl rand -base64 32
APP__JWT_SECRET=your-super-secret-jwt-key-change-this-in-production

# Token expiration times (in seconds)
APP__JWT_EXPIRATION=3600              # 1 hour
APP__REFRESH_TOKEN_EXPIRATION=604800  # 7 days
```

## Optional Variables

### Server Configuration
```bash
# Server host and port
APP__HOST=0.0.0.0
APP__PORT=8080

# Environment (development, staging, production)
APP__ENVIRONMENT=development

# Logging level (trace, debug, info, warn, error)
APP__LOG_LEVEL=info
```

### Feature Flags
```bash
# Automatically run database migrations on startup
APP__AUTO_MIGRATE=false

# Enable metrics collection
ENABLE_METRICS=true

# Enable distributed tracing
ENABLE_TRACING=false
```

### Rate Limiting
```bash
# Number of requests allowed per minute
RATE_LIMIT_REQUESTS_PER_MINUTE=60

# Burst capacity for rate limiting
RATE_LIMIT_BURST=10

# Enable Redis-backed rate limiter and namespace prefix
APP__RATE_LIMIT_USE_REDIS=true
APP__RATE_LIMIT_NAMESPACE=stateset:rl
```

### Message Queue
```bash
# Backend driver for the internal queue (in-memory, redis)
APP__MESSAGE_QUEUE_BACKEND=redis

# Namespace prefix for Redis queue keys (only if backend=redis)
APP__MESSAGE_QUEUE_NAMESPACE=stateset:mq

# Seconds to block while polling the queue before returning control
APP__MESSAGE_QUEUE_BLOCK_TIMEOUT_SECS=5
```

### Cache Configuration
```bash
# Cache backend (redis, memory)
CACHE_TYPE=redis

# Default cache TTL in seconds
CACHE_TTL_SECONDS=300

# Maximum number of cached items (for memory cache)
CACHE_CAPACITY=1000
```

### CORS Configuration
```bash
# Comma-separated list of allowed origins
APP__CORS_ALLOWED_ORIGINS=http://localhost:3000,http://localhost:8080

# (Development only) opt in to permissive CORS fallback
APP__CORS_ALLOW_ANY_ORIGIN=false
```

### Security Settings
```bash
# Session timeout in minutes
SESSION_TIMEOUT_MINUTES=30

# Password requirements
PASSWORD_MIN_LENGTH=8

# API key prefix for identification
API_KEY_PREFIX=ss_
```

### External Services (if applicable)
```bash
# Stripe payment processing
STRIPE_API_KEY=sk_test_...
STRIPE_WEBHOOK_SECRET=whsec_...

# Email service
SENDGRID_API_KEY=SG...
# OR
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USERNAME=your-email@example.com
SMTP_PASSWORD=your-smtp-password
FROM_EMAIL=noreply@stateset.io

# AWS services
AWS_ACCESS_KEY_ID=...
AWS_SECRET_ACCESS_KEY=...
AWS_REGION=us-east-1
S3_BUCKET_NAME=stateset-uploads

### Agentic Commerce
```bash
# Shared secret for verifying incoming ACP signatures
APP__AGENTIC_COMMERCE_SIGNING_SECRET=acp_shared_secret

# Optional: signature timestamp tolerance in seconds (default 300)
APP__AGENTIC_COMMERCE_SIGNATURE_TOLERANCE_SECS=300

# Optional: outbound webhook configuration
APP__AGENTIC_COMMERCE_WEBHOOK_URL=https://api.openai.com/v1/merchants/webhooks
APP__AGENTIC_COMMERCE_WEBHOOK_SECRET=merchant_webhook_secret
```

# Monitoring
SENTRY_DSN=https://...@sentry.io/...
DATADOG_API_KEY=...
```

### Development-Only Settings
```bash
# WARNING: These should NEVER be enabled in production
DEV_BYPASS_AUTH=false      # Skip authentication checks
DEV_SEED_DATABASE=false    # Populate database with test data
DEV_ENABLE_SWAGGER=true    # Enable Swagger UI documentation
```

## Example Overrides

Minimal overrides for local development (without editing `config/default.toml`):

```bash
APP__DATABASE_URL=sqlite:stateset.db?mode=rwc
APP__REDIS_URL=redis://localhost:6379
APP__JWT_SECRET=development-secret-key-do-not-use-in-production
APP__ENVIRONMENT=development
APP__LOG_LEVEL=debug
APP__AUTO_MIGRATE=true
```

Docker Compose uses the root `.env` file for container variables (e.g., `DATABASE_URL`, `REDIS_URL`, `PORT`). These are separate from `APP__*` variables consumed by the app’s config loader.

## Production Considerations

1. **Generate secure secrets**:
   ```bash
   # Generate a secure JWT secret
   openssl rand -base64 32
   
   # Generate API keys
   openssl rand -hex 32
   ```

2. **Use a secrets manager**: Consider using AWS Secrets Manager, HashiCorp Vault, or similar for production deployments

3. **Validate configuration**: The application will validate required environment variables on startup

4. **Never commit .env files**: Ensure `.env` is in your `.gitignore`

## Configuration Precedence

Environment variables are loaded in the following order (later sources override earlier ones):
1. Default values in code
2. `config/default.toml`
3. Environment-specific config (e.g., `config/production.toml`)
4. Environment variables
5. `.env` file (in development only)

## Troubleshooting

If you encounter configuration errors:

1. Check that all required variables are set
2. Verify database and Redis connectivity
3. Ensure secrets are properly formatted (no extra whitespace)
4. Check logs for specific validation errors
5. Run with `LOG_LEVEL=debug` for more detailed output 
