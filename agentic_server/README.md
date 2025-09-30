# Agentic Commerce Server

A standalone, lightweight Rust server specifically for OpenAI's ChatGPT Instant Checkout (Agentic Checkout Spec).

## Features

- ✅ **Full Agentic Checkout Spec compliance** - All 5 checkout endpoints
- ✅ **Delegated Payment Spec support** - Mock PSP for testing vault tokens
- ✅ **Lightweight** - Only checkout functionality, no extra features
- ✅ **Fast** - In-memory session storage for low latency
- ✅ **Independent** - Can be deployed separately from main API
- ✅ **Production-ready** - Structured logging, graceful shutdown, CORS support
- ✅ **Single-use tokens** - Vault tokens are consumed after use
- ✅ **Allowance validation** - Max amount and expiry enforcement

## API Endpoints

### Agentic Checkout Endpoints

All checkout endpoints are available at the root path:

- `POST /checkout_sessions` - Create checkout session
- `GET /checkout_sessions/:id` - Retrieve checkout session
- `POST /checkout_sessions/:id` - Update checkout session
- `POST /checkout_sessions/:id/complete` - Complete and create order
- `POST /checkout_sessions/:id/cancel` - Cancel checkout session

### Delegated Payment Endpoint (PSP Mock)

For testing the full Agentic Commerce flow:

- `POST /agentic_commerce/delegate_payment` - Delegate payment (PSP endpoint)
  - Accepts card details and returns a vault token (`vt_*`)
  - Validates card number, expiry, allowances
  - Enforces single-use tokens with max amount limits
  - Tokens can be used in checkout completion

### Health & Monitoring

- `GET /health` - Health check
- `GET /ready` - Readiness check

## Quick Start

### 1. Build

```bash
cd agentic_server
cargo build --release
```

### 2. Configure

Copy the example environment file:

```bash
cp .env.example .env
```

Edit `.env` to customize:

```env
HOST=0.0.0.0
PORT=8080
LOG_LEVEL=info
```

### 3. Run

```bash
cargo run --release
```

Or run the binary directly:

```bash
./target/release/agentic-commerce-server
```

The server will start on `http://0.0.0.0:8080`

## Docker Deployment

### Build Docker image

```bash
docker build -t agentic-commerce-server .
```

### Run container

```bash
docker run -p 8080:8080 \
  -e HOST=0.0.0.0 \
  -e PORT=8080 \
  agentic-commerce-server
```

## Usage Examples

### Create Session

```bash
curl -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_123" \
  -H "API-Version: 2025-09-29" \
  -d '{
    "items": [
      {
        "id": "item_123",
        "quantity": 1
      }
    ],
    "fulfillment_address": {
      "name": "John Doe",
      "line_one": "123 Main St",
      "city": "San Francisco",
      "state": "CA",
      "country": "US",
      "postal_code": "94102"
    }
  }'
```

### Update Session

```bash
curl -X POST http://localhost:8080/checkout_sessions/{session_id} \
  -H "Content-Type: application/json" \
  -d '{
    "buyer": {
      "first_name": "John",
      "last_name": "Doe",
      "email": "john@example.com"
    },
    "fulfillment_option_id": "standard_shipping"
  }'
```

### Delegate Payment (Create Vault Token)

```bash
curl -X POST http://localhost:8080/agentic_commerce/delegate_payment \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer api_key_123" \
  -H "Idempotency-Key: idempotency_key_456" \
  -d '{
    "payment_method": {
      "type": "card",
      "card_number_type": "fpan",
      "number": "4242424242424242",
      "exp_month": "12",
      "exp_year": "2026",
      "name": "John Doe",
      "cvc": "123",
      "display_card_funding_type": "credit",
      "display_brand": "Visa",
      "display_last4": "4242",
      "metadata": {}
    },
    "allowance": {
      "reason": "one_time",
      "max_amount": 10000,
      "currency": "usd",
      "checkout_session_id": "{session_id}",
      "merchant_id": "merchant_123",
      "expires_at": "2025-12-31T23:59:59Z"
    },
    "billing_address": {
      "name": "John Doe",
      "line_one": "123 Main St",
      "city": "San Francisco",
      "state": "CA",
      "country": "US",
      "postal_code": "94102"
    },
    "risk_signals": [
      {
        "type": "card_testing",
        "score": 5,
        "action": "authorized"
      }
    ],
    "metadata": {}
  }'
```

Response:
```json
{
  "id": "vt_550e8400-e29b-41d4-a716-446655440000",
  "created": "2025-09-30T12:00:00Z",
  "metadata": {}
}
```

### Complete Checkout (with Vault Token)

```bash
curl -X POST http://localhost:8080/checkout_sessions/{session_id}/complete \
  -H "Content-Type: application/json" \
  -d '{
    "payment_data": {
      "token": "vt_550e8400-e29b-41d4-a716-446655440000",
      "provider": "stripe"
    }
  }'
```

Or with regular payment token:

```bash
curl -X POST http://localhost:8080/checkout_sessions/{session_id}/complete \
  -H "Content-Type: application/json" \
  -d '{
    "payment_data": {
      "token": "tok_visa",
      "provider": "stripe"
    }
  }'
```

## Configuration

### Environment Variables

- `HOST` - Server host (default: `0.0.0.0`)
- `PORT` - Server port (default: `8080`)
- `LOG_LEVEL` - Log level: `trace`, `debug`, `info`, `warn`, `error` (default: `info`)

## Architecture

```
┌─────────────────┐
│   ChatGPT/      │
│   OpenAI        │
└────────┬────────┘
         │
         │ HTTPS/REST
         │
┌────────▼────────────────┐
│  Agentic Commerce       │
│  Server (Axum)          │
│                         │
│  - Handlers             │
│  - Service Layer        │
│  - In-Memory Cache      │
│  - Event System         │
└─────────────────────────┘
```

## Delegated Payment Flow

This server includes a mock PSP implementation for testing the complete Agentic Commerce + Delegated Payment flow:

### Flow Diagram

```
┌─────────┐         ┌──────────┐         ┌──────────┐
│ ChatGPT │         │  Server  │         │   PSP    │
│         │         │(Merchant)│         │  (Mock)  │
└────┬────┘         └─────┬────┘         └────┬─────┘
     │                    │                   │
     │ 1. Create Session  │                   │
     ├───────────────────>│                   │
     │                    │                   │
     │ 2. Save Payment    │                   │
     │    in ChatGPT      │                   │
     │                    │                   │
     │ 3. Delegate Payment│                   │
     ├───────────────────────────────────────>│
     │                    │   4. Vault Token  │
     │                    │<──────────────────┤
     │                    │    (vt_xxx)       │
     │ 5. Complete with   │                   │
     │    vault token     │                   │
     ├───────────────────>│                   │
     │                    │ 6. Validate Token │
     │                    ├──────────────────>│
     │                    │ 7. Token Valid    │
     │                    │<──────────────────┤
     │                    │ 8. Consume Token  │
     │                    ├──────────────────>│
     │ 9. Order Created   │                   │
     │<───────────────────┤                   │
```

### Key Features

1. **Single-Use Tokens** - Each vault token can only be used once
2. **Max Amount Enforcement** - Tokens have a maximum allowed charge amount
3. **Session Binding** - Tokens are tied to specific checkout sessions
4. **Expiry** - Tokens automatically expire based on allowance
5. **Card Validation** - Basic card number and expiry validation
6. **Risk Signals** - Support for blocking based on risk assessment

### Testing the Full Flow

```bash
# 1. Create checkout session
SESSION_ID=$(curl -X POST http://localhost:8080/checkout_sessions \
  -H "Content-Type: application/json" \
  -d '{"items":[{"id":"item_123","quantity":1}]}' | jq -r '.id')

# 2. Delegate payment to get vault token
VAULT_TOKEN=$(curl -X POST http://localhost:8080/agentic_commerce/delegate_payment \
  -H "Content-Type: application/json" \
  -d "{
    \"payment_method\": {
      \"type\": \"card\",
      \"card_number_type\": \"fpan\",
      \"number\": \"4242424242424242\",
      \"exp_month\": \"12\",
      \"exp_year\": \"2026\",
      \"display_card_funding_type\": \"credit\",
      \"display_last4\": \"4242\",
      \"metadata\": {}
    },
    \"allowance\": {
      \"reason\": \"one_time\",
      \"max_amount\": 10000,
      \"currency\": \"usd\",
      \"checkout_session_id\": \"$SESSION_ID\",
      \"merchant_id\": \"merchant_123\",
      \"expires_at\": \"2025-12-31T23:59:59Z\"
    },
    \"risk_signals\": [{\"type\":\"card_testing\",\"score\":5,\"action\":\"authorized\"}],
    \"metadata\": {}
  }" | jq -r '.id')

# 3. Update session with address
curl -X POST http://localhost:8080/checkout_sessions/$SESSION_ID \
  -H "Content-Type: application/json" \
  -d '{
    "buyer": {"first_name":"John","last_name":"Doe","email":"john@example.com"},
    "fulfillment_address": {"name":"John Doe","line_one":"123 Main","city":"SF","state":"CA","country":"US","postal_code":"94102"},
    "fulfillment_option_id": "standard_shipping"
  }'

# 4. Complete with vault token
curl -X POST http://localhost:8080/checkout_sessions/$SESSION_ID/complete \
  -H "Content-Type: application/json" \
  -d "{\"payment_data\":{\"token\":\"$VAULT_TOKEN\",\"provider\":\"stripe\"}}"
```

## Session Storage

Sessions are stored in-memory with a 1-hour TTL. For production use with multiple instances, consider:

- Redis for distributed cache
- Database for persistent storage
- Sticky sessions at load balancer

## Development

### Run in development mode

```bash
cargo run
```

### Run tests

```bash
cargo test
```

### Format code

```bash
cargo fmt
```

### Lint

```bash
cargo clippy
```

## Production Deployment

### Recommendations

1. **Use TLS/HTTPS** - Required by OpenAI spec
2. **Implement authentication** - Validate Bearer tokens
3. **Add rate limiting** - Protect against abuse
4. **Use persistent storage** - Redis or database for sessions
5. **Add monitoring** - Metrics, logging, tracing
6. **Implement webhooks** - Notify OpenAI of order updates
7. **Set up health checks** - For load balancer integration

### Environment Setup

For production:

```env
HOST=0.0.0.0
PORT=8080
LOG_LEVEL=info
```

### Systemd Service

Create `/etc/systemd/system/agentic-commerce.service`:

```ini
[Unit]
Description=Agentic Commerce Server
After=network.target

[Service]
Type=simple
User=agentic
WorkingDirectory=/opt/agentic-commerce
ExecStart=/opt/agentic-commerce/agentic-commerce-server
Restart=always
RestartSec=5
Environment=HOST=0.0.0.0
Environment=PORT=8080
Environment=LOG_LEVEL=info

[Install]
WantedBy=multi-user.target
```

Enable and start:

```bash
sudo systemctl enable agentic-commerce
sudo systemctl start agentic-commerce
```

## Monitoring

The server provides health check endpoints:

- `/health` - Basic health check
- `/ready` - Readiness probe (useful for K8s)

Example Prometheus metrics integration can be added with `axum-prometheus` crate.

## Security

- ✅ Bearer token authentication support (implement validation)
- ✅ CORS enabled
- ✅ Request/response compression
- ✅ Graceful shutdown
- ⚠️ Add request signature verification for production
- ⚠️ Add rate limiting
- ⚠️ Add request size limits

## License

MIT

## Support

For issues or questions about the Agentic Checkout Spec, see:
- OpenAI Agentic Commerce documentation
- `agentic-checkout.yaml` OpenAPI specification 