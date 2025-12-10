# Agentic Commerce Webhook Implementation

## Overview

This document describes the implementation of Agentic Commerce Protocol (ACP) webhooks in the stateset-api to match the functionality available in the agentic_server.

## What Was Implemented

### 1. Webhook Delivery Module

**Location**: `src/webhooks/`

Created a new webhook delivery module with the following components:

#### Files Created:
- `src/webhooks/mod.rs` - Module declaration
- `src/webhooks/agentic_commerce.rs` - Full webhook delivery implementation

#### Key Components:

**WebhookEvent Enum**:
```rust
pub enum WebhookEvent {
    OrderCreated { data: OrderEventData },
    OrderUpdated { data: OrderEventData },
}
```

**OrderEventData Struct**:
```rust
pub struct OrderEventData {
    pub data_type: String,              // "order"
    pub checkout_session_id: String,
    pub permalink_url: String,
    pub status: String,                 // "created", "confirmed", "shipped", etc.
    pub refunds: Vec<Refund>,
}
```

**AgenticCommerceWebhookService**:
- HTTP client with 10-second timeout
- HMAC signature generation for webhook authentication
- Retry logic with exponential backoff (3 attempts: 1s, 2s, 4s)
- Fire-and-forget async delivery
- Comprehensive error handling and logging

### 2. Configuration Updates

**Location**: `src/config.rs`

Added two new configuration fields to `AppConfig`:

```rust
/// Agentic Commerce: OpenAI webhook URL for order events
pub agentic_commerce_webhook_url: Option<String>,

/// Agentic Commerce: Webhook secret for HMAC signatures
pub agentic_commerce_webhook_secret: Option<String>,
```

**Environment Variables**:
- `AGENTIC_COMMERCE_WEBHOOK_URL` - OpenAI webhook endpoint
- `AGENTIC_COMMERCE_WEBHOOK_SECRET` - Secret for HMAC signature generation

### 3. Event Processing Integration

**Location**: `src/events/mod.rs`

Updated the `process_events` function to:
1. Accept webhook service and URL parameters
2. Handle `CheckoutCompleted` events
3. Handle `OrderUpdated` events
4. Send webhooks to OpenAI when events occur

**Event Handlers**:

**CheckoutCompleted**:
```rust
Event::CheckoutCompleted { session_id, order_id } => {
    // Sends both order_created and order_updated webhooks
    // - order_created: Initial order creation notification
    // - order_updated: Status update with "created" status
}
```

**OrderUpdated**:
```rust
Event::OrderUpdated(order_id) => {
    // Placeholder for future implementation
    // TODO: Fetch order details from database and send webhook
}
```

### 4. Main Application Initialization

**Location**: `src/main.rs`

Updated to:
1. Initialize webhook service with optional secret
2. Pass webhook service and URL to event processor
3. Log configuration status on startup

**Other Files Updated**:
- `src/lib.rs` - Added webhooks module
- `src/bin/grpc_server.rs` - Updated process_events call
- `tests/common/mod.rs` - Updated process_events call
- `tests/inventory_concurrency_test.rs` - Updated process_events call

## Features

### ✅ Implemented

- **HMAC Signature Authentication**: Uses SHA256 HMAC for webhook security
- **Retry Logic**: Exponential backoff (1s, 2s, 4s) for failed deliveries
- **Async Delivery**: Fire-and-forget to prevent blocking
- **Structured Logging**: All webhook attempts logged with tracing
- **Event-Driven**: Integrates with existing event system
- **Configuration**: Environment variable based configuration

### 📋 Compliance with ACP Spec

- ✅ `order_created` webhook event
- ✅ `order_updated` webhook event
- ✅ Correct event payload structure
- ✅ HMAC signature in `Merchant-Signature` header
- ✅ Timestamp header
- ✅ Retry logic for failed deliveries

## Configuration Example

### Development

```bash
# .env or environment variables
export AGENTIC_COMMERCE_WEBHOOK_URL="https://openai.example.com/webhooks"
export AGENTIC_COMMERCE_WEBHOOK_SECRET="your_webhook_secret_here"
```

### Production

```toml
# config/production.toml
agentic_commerce_webhook_url = "https://api.openai.com/v1/merchants/webhooks"
agentic_commerce_webhook_secret = "prod_secret_key"
```

## Usage

### Automatic Webhook Delivery

Webhooks are automatically sent when:

1. **Checkout Completes** (`Event::CheckoutCompleted`):
   - Sends `order_created` webhook
   - Sends `order_updated` webhook with status "created"

2. **Order Updates** (`Event::OrderUpdated`):
   - Currently logs placeholder message
   - TODO: Implement full order details fetch and webhook delivery

### Example Webhook Payload

**order_created**:
```json
{
  "type": "order_created",
  "data": {
    "type": "order",
    "checkout_session_id": "550e8400-e29b-41d4-a716-446655440000",
    "permalink_url": "https://merchant.example.com/orders/123e4567-e89b-12d3-a456-426614174000",
    "status": "created",
    "refunds": []
  }
}
```

**order_updated**:
```json
{
  "type": "order_updated",
  "data": {
    "type": "order",
    "checkout_session_id": "550e8400-e29b-41d4-a716-446655440000",
    "permalink_url": "https://merchant.example.com/orders/123e4567-e89b-12d3-a456-426614174000",
    "status": "shipped",
    "refunds": [
      {
        "type": "store_credit",
        "amount": 1000
      }
    ]
  }
}
```

## Webhook Headers

Outbound webhooks include:

```
Content-Type: application/json
Timestamp: 2025-01-15T09:45:12.000Z
Merchant-Signature: abc123def456...
```

**Signature Calculation**:
```
HMAC-SHA256(secret, "{timestamp}.{body}")
```

## Testing

### Unit Tests

Tests are included in `src/webhooks/agentic_commerce.rs`:

```bash
cargo test --lib webhooks
```

**Test Coverage**:
- ✅ Webhook event serialization
- ✅ Refund serialization
- ✅ HMAC signature generation

### Integration Testing

To test webhook delivery:

```bash
# 1. Set environment variables
export AGENTIC_COMMERCE_WEBHOOK_URL="https://webhook.site/your-unique-id"
export AGENTIC_COMMERCE_WEBHOOK_SECRET="test_secret"

# 2. Run the server
cargo run

# 3. Trigger a checkout completion (via API or tests)
# 4. Check webhook.site for received webhooks
```

## Comparison with agentic_server

| Feature | agentic_server | stateset-api |
|---------|---------------|--------------|
| Webhook Events | ✅ order_created, order_updated | ✅ order_created, order_updated |
| HMAC Signatures | ✅ | ✅ |
| Retry Logic | ✅ 3 attempts | ✅ 3 attempts |
| Exponential Backoff | ✅ | ✅ |
| Event Integration | ✅ | ✅ |
| Configuration | ✅ Environment vars | ✅ Environment vars |
| Async Delivery | ✅ | ✅ |

## Future Enhancements

### TODO: Enhanced OrderUpdated Handler

Currently, the `OrderUpdated` event handler is a placeholder. To fully implement:

1. **Fetch Order Details**:
   ```rust
   // Query order from database
   let order = order_service.get_order(order_id).await?;

   // Get associated checkout session
   let session_id = order.checkout_session_id;

   // Get current status
   let status = order.status.to_string();

   // Get refunds
   let refunds = fetch_refunds_for_order(order_id).await?;
   ```

2. **Send Webhook**:
   ```rust
   service.send_order_updated(
       url,
       session_id,
       permalink,
       status,
       refunds,
   ).await?;
   ```

### Dead Letter Queue

For production, consider adding failed webhook delivery to a dead letter queue:

```rust
// In send_async error handler
if let Err(e) = service.send_webhook(&webhook_url, event).await {
    error!("Async webhook delivery failed: {}", e);

    // Add to DLQ for retry
    dlq_service.enqueue(webhook_url, event).await;
}
```

### Monitoring

Add metrics for webhook delivery:

```rust
use metrics::{counter, histogram};

// Track attempts
counter!("webhooks.sent.total", 1, "event_type" => event_type);

// Track failures
counter!("webhooks.failed.total", 1, "event_type" => event_type);

// Track delivery time
histogram!("webhooks.delivery_duration_ms", duration_ms);
```

## Security Considerations

### HMAC Signature Verification

Recipients should verify webhooks using:

```rust
fn verify_signature(timestamp: &str, body: &str, signature: &str, secret: &str) -> bool {
    let signed_payload = format!("{}.{}", timestamp, body);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(signed_payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    // Constant-time comparison
    constant_time_eq(&expected, signature)
}
```

### Timestamp Validation

Recipients should check timestamp to prevent replay attacks:

```rust
let max_age_secs = 300; // 5 minutes
let timestamp_i64 = timestamp.parse::<i64>()?;
let now = chrono::Utc::now().timestamp();

if (now - timestamp_i64).abs() > max_age_secs {
    return Err("Timestamp too old or in future");
}
```

## Files Modified Summary

| File | Change |
|------|--------|
| `src/webhooks/mod.rs` | Created - Module declaration |
| `src/webhooks/agentic_commerce.rs` | Created - Full implementation |
| `src/config.rs` | Modified - Added webhook config fields |
| `src/events/mod.rs` | Modified - Added webhook delivery logic |
| `src/main.rs` | Modified - Initialize webhook service |
| `src/lib.rs` | Modified - Added webhooks module |
| `src/bin/grpc_server.rs` | Modified - Updated process_events call |
| `tests/common/mod.rs` | Modified - Updated process_events call |
| `tests/inventory_concurrency_test.rs` | Modified - Updated process_events call |

**Total Lines Added**: ~400 lines

## References

- [OpenAI Agentic Commerce Protocol](https://developers.openai.com/commerce)
- [Agentic Checkout Spec](https://developers.openai.com/commerce/specs/checkout)
- [HMAC Authentication](https://en.wikipedia.org/wiki/HMAC)
- [agentic_server Implementation](../agentic_server/src/webhook_service.rs)
