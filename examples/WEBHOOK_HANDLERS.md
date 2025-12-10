# StateSet API - Webhook Handlers

This document provides webhook handler implementations for the StateSet API in multiple languages and frameworks. Webhooks allow you to receive real-time notifications about events in your StateSet account.

## Table of Contents

- [Webhook Events](#webhook-events)
- [Security Verification](#security-verification)
- [Express.js/Node.js Handler](#expressjsnodejs-handler)
- [Next.js API Route Handler](#nextjs-api-route-handler)
- [Python/Flask Handler](#pythonflask-handler)
- [Python/FastAPI Handler](#pythonfastapi-handler)
- [PHP Handler](#php-handler)
- [Go Handler](#go-handler)
- [Ruby/Sinatra Handler](#rubysinatra-handler)
- [Testing Webhooks](#testing-webhooks)

---

## Webhook Events

StateSet API sends the following webhook events:

### Order Events
- `order.created` - New order created
- `order.updated` - Order details updated
- `order.cancelled` - Order cancelled
- `order.completed` - Order completed

### Payment Events
- `payment.succeeded` - Payment processed successfully
- `payment.failed` - Payment failed
- `payment.refunded` - Payment refunded

### Shipment Events
- `shipment.created` - Shipment created
- `shipment.shipped` - Shipment marked as shipped
- `shipment.delivered` - Shipment delivered

### Inventory Events
- `inventory.updated` - Inventory quantity changed
- `inventory.low_stock` - Inventory below threshold

### Return Events
- `return.created` - Return request created
- `return.approved` - Return approved
- `return.completed` - Return completed

---

## Security Verification

All webhook requests include an `X-StateSet-Signature` header containing an HMAC SHA-256 signature. Always verify this signature before processing the webhook.

### Signature Verification Algorithm

```
signature = HMAC-SHA256(webhook_secret, request_body)
```

The signature should be compared using a constant-time comparison function to prevent timing attacks.

---

## Express.js/Node.js Handler

```typescript
import express, { Request, Response } from 'express';
import crypto from 'crypto';

const app = express();

// IMPORTANT: Use raw body parser for webhook routes
app.post('/webhooks/stateset',
  express.raw({ type: 'application/json' }),
  (req: Request, res: Response) => {
    const signature = req.headers['x-stateset-signature'] as string;
    const webhookSecret = process.env.STATESET_WEBHOOK_SECRET!;

    // Verify signature
    if (!verifySignature(req.body, signature, webhookSecret)) {
      console.error('Invalid webhook signature');
      return res.status(401).send('Invalid signature');
    }

    // Parse the event
    const event = JSON.parse(req.body.toString());
    console.log('Received webhook event:', event.type);

    // Handle the event
    handleWebhookEvent(event)
      .then(() => {
        res.status(200).send('OK');
      })
      .catch((error) => {
        console.error('Error handling webhook:', error);
        res.status(500).send('Internal Server Error');
      });
  }
);

function verifySignature(body: Buffer, signature: string, secret: string): boolean {
  const hmac = crypto.createHmac('sha256', secret);
  const digest = hmac.update(body).digest('hex');

  // Use constant-time comparison
  return crypto.timingSafeEqual(
    Buffer.from(signature),
    Buffer.from(digest)
  );
}

async function handleWebhookEvent(event: any): Promise<void> {
  switch (event.type) {
    case 'order.created':
      await handleOrderCreated(event.data);
      break;

    case 'order.updated':
      await handleOrderUpdated(event.data);
      break;

    case 'payment.succeeded':
      await handlePaymentSucceeded(event.data);
      break;

    case 'shipment.delivered':
      await handleShipmentDelivered(event.data);
      break;

    case 'inventory.low_stock':
      await handleLowStock(event.data);
      break;

    default:
      console.log('Unhandled event type:', event.type);
  }
}

async function handleOrderCreated(order: any): Promise<void> {
  console.log('Order created:', order.id);
  // Your business logic here
  // - Send confirmation email
  // - Create fulfillment task
  // - Update analytics
}

async function handleOrderUpdated(order: any): Promise<void> {
  console.log('Order updated:', order.id, 'Status:', order.status);
  // Your business logic here
}

async function handlePaymentSucceeded(payment: any): Promise<void> {
  console.log('Payment succeeded:', payment.id);
  // Your business logic here
  // - Update order status
  // - Trigger fulfillment
  // - Send receipt
}

async function handleShipmentDelivered(shipment: any): Promise<void> {
  console.log('Shipment delivered:', shipment.id);
  // Your business logic here
  // - Send delivery confirmation
  // - Request review
  // - Update order status
}

async function handleLowStock(inventory: any): Promise<void> {
  console.log('Low stock alert:', inventory.sku, 'Quantity:', inventory.quantity_available);
  // Your business logic here
  // - Send notification to purchasing team
  // - Create purchase order
  // - Update product availability
}

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
  console.log(`Webhook server listening on port ${PORT}`);
});
```

---

## Next.js API Route Handler

Create a file at `pages/api/webhooks/stateset.ts`:

```typescript
import { NextApiRequest, NextApiResponse } from 'next';
import crypto from 'crypto';

export const config = {
  api: {
    bodyParser: false, // We need the raw body for signature verification
  },
};

async function getRawBody(req: NextApiRequest): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    req.on('data', (chunk) => chunks.push(chunk));
    req.on('end', () => resolve(Buffer.concat(chunks)));
    req.on('error', reject);
  });
}

function verifySignature(body: Buffer, signature: string, secret: string): boolean {
  const hmac = crypto.createHmac('sha256', secret);
  const digest = hmac.update(body).digest('hex');
  return crypto.timingSafeEqual(Buffer.from(signature), Buffer.from(digest));
}

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== 'POST') {
    return res.status(405).json({ error: 'Method not allowed' });
  }

  try {
    const rawBody = await getRawBody(req);
    const signature = req.headers['x-stateset-signature'] as string;
    const webhookSecret = process.env.STATESET_WEBHOOK_SECRET!;

    // Verify signature
    if (!verifySignature(rawBody, signature, webhookSecret)) {
      console.error('Invalid webhook signature');
      return res.status(401).json({ error: 'Invalid signature' });
    }

    // Parse and handle event
    const event = JSON.parse(rawBody.toString());
    console.log('Received webhook event:', event.type);

    // Handle event (async, don't await)
    handleWebhookEvent(event).catch((error) => {
      console.error('Error handling webhook:', error);
    });

    // Respond immediately
    res.status(200).json({ received: true });
  } catch (error) {
    console.error('Webhook error:', error);
    res.status(500).json({ error: 'Internal server error' });
  }
}

async function handleWebhookEvent(event: any): Promise<void> {
  // Your event handling logic here
  switch (event.type) {
    case 'order.created':
      // Handle order creation
      break;
    case 'payment.succeeded':
      // Handle successful payment
      break;
    // ... other events
  }
}
```

---

## Python/Flask Handler

```python
from flask import Flask, request, jsonify
import hmac
import hashlib
import os
import json

app = Flask(__name__)

WEBHOOK_SECRET = os.environ.get('STATESET_WEBHOOK_SECRET')

def verify_signature(body, signature, secret):
    """Verify webhook signature using HMAC SHA-256"""
    expected_signature = hmac.new(
        secret.encode('utf-8'),
        body,
        hashlib.sha256
    ).hexdigest()

    # Use constant-time comparison
    return hmac.compare_digest(signature, expected_signature)

@app.route('/webhooks/stateset', methods=['POST'])
def handle_webhook():
    # Get signature from header
    signature = request.headers.get('X-StateSet-Signature')
    if not signature:
        return jsonify({'error': 'Missing signature'}), 401

    # Get raw body
    body = request.get_data()

    # Verify signature
    if not verify_signature(body, signature, WEBHOOK_SECRET):
        app.logger.error('Invalid webhook signature')
        return jsonify({'error': 'Invalid signature'}), 401

    # Parse event
    event = json.loads(body)
    app.logger.info(f"Received webhook event: {event['type']}")

    # Handle event
    try:
        handle_webhook_event(event)
        return jsonify({'received': True}), 200
    except Exception as e:
        app.logger.error(f"Error handling webhook: {e}")
        return jsonify({'error': 'Internal server error'}), 500

def handle_webhook_event(event):
    """Handle different webhook event types"""
    event_type = event['type']
    data = event['data']

    if event_type == 'order.created':
        handle_order_created(data)
    elif event_type == 'order.updated':
        handle_order_updated(data)
    elif event_type == 'payment.succeeded':
        handle_payment_succeeded(data)
    elif event_type == 'shipment.delivered':
        handle_shipment_delivered(data)
    elif event_type == 'inventory.low_stock':
        handle_low_stock(data)
    else:
        app.logger.info(f"Unhandled event type: {event_type}")

def handle_order_created(order):
    """Handle order created event"""
    print(f"Order created: {order['id']}")
    # Your business logic here

def handle_order_updated(order):
    """Handle order updated event"""
    print(f"Order updated: {order['id']}, Status: {order['status']}")
    # Your business logic here

def handle_payment_succeeded(payment):
    """Handle payment succeeded event"""
    print(f"Payment succeeded: {payment['id']}")
    # Your business logic here

def handle_shipment_delivered(shipment):
    """Handle shipment delivered event"""
    print(f"Shipment delivered: {shipment['id']}")
    # Your business logic here

def handle_low_stock(inventory):
    """Handle low stock event"""
    print(f"Low stock alert: {inventory['sku']}, Quantity: {inventory['quantity_available']}")
    # Your business logic here

if __name__ == '__main__':
    app.run(port=3000, debug=False)
```

---

## Python/FastAPI Handler

```python
from fastapi import FastAPI, Request, HTTPException, Header
from typing import Optional
import hmac
import hashlib
import os
import json

app = FastAPI()

WEBHOOK_SECRET = os.environ.get('STATESET_WEBHOOK_SECRET')

def verify_signature(body: bytes, signature: str, secret: str) -> bool:
    """Verify webhook signature using HMAC SHA-256"""
    expected_signature = hmac.new(
        secret.encode('utf-8'),
        body,
        hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(signature, expected_signature)

@app.post("/webhooks/stateset")
async def handle_webhook(
    request: Request,
    x_stateset_signature: Optional[str] = Header(None)
):
    # Get raw body
    body = await request.body()

    # Verify signature
    if not x_stateset_signature:
        raise HTTPException(status_code=401, detail="Missing signature")

    if not verify_signature(body, x_stateset_signature, WEBHOOK_SECRET):
        raise HTTPException(status_code=401, detail="Invalid signature")

    # Parse event
    event = json.loads(body)
    print(f"Received webhook event: {event['type']}")

    # Handle event asynchronously
    await handle_webhook_event(event)

    return {"received": True}

async def handle_webhook_event(event: dict):
    """Handle different webhook event types"""
    event_type = event['type']
    data = event['data']

    handlers = {
        'order.created': handle_order_created,
        'order.updated': handle_order_updated,
        'payment.succeeded': handle_payment_succeeded,
        'shipment.delivered': handle_shipment_delivered,
        'inventory.low_stock': handle_low_stock,
    }

    handler = handlers.get(event_type)
    if handler:
        await handler(data)
    else:
        print(f"Unhandled event type: {event_type}")

async def handle_order_created(order: dict):
    """Handle order created event"""
    print(f"Order created: {order['id']}")
    # Your business logic here

async def handle_order_updated(order: dict):
    """Handle order updated event"""
    print(f"Order updated: {order['id']}, Status: {order['status']}")
    # Your business logic here

async def handle_payment_succeeded(payment: dict):
    """Handle payment succeeded event"""
    print(f"Payment succeeded: {payment['id']}")
    # Your business logic here

async def handle_shipment_delivered(shipment: dict):
    """Handle shipment delivered event"""
    print(f"Shipment delivered: {shipment['id']}")
    # Your business logic here

async def handle_low_stock(inventory: dict):
    """Handle low stock event"""
    print(f"Low stock alert: {inventory['sku']}, Quantity: {inventory['quantity_available']}")
    # Your business logic here

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=3000)
```

---

## PHP Handler

```php
<?php

/**
 * StateSet Webhook Handler for PHP
 */

// Get raw POST body
$rawBody = file_get_contents('php://input');

// Get signature from header
$signature = $_SERVER['HTTP_X_STATESET_SIGNATURE'] ?? '';

// Webhook secret from environment
$webhookSecret = getenv('STATESET_WEBHOOK_SECRET');

// Verify signature
if (!verifySignature($rawBody, $signature, $webhookSecret)) {
    http_response_code(401);
    echo json_encode(['error' => 'Invalid signature']);
    exit;
}

// Parse event
$event = json_decode($rawBody, true);

// Log event
error_log("Received webhook event: {$event['type']}");

// Handle event
try {
    handleWebhookEvent($event);
    http_response_code(200);
    echo json_encode(['received' => true]);
} catch (Exception $e) {
    error_log("Error handling webhook: {$e->getMessage()}");
    http_response_code(500);
    echo json_encode(['error' => 'Internal server error']);
}

/**
 * Verify webhook signature
 */
function verifySignature($body, $signature, $secret) {
    $expectedSignature = hash_hmac('sha256', $body, $secret);
    return hash_equals($signature, $expectedSignature);
}

/**
 * Handle webhook event
 */
function handleWebhookEvent($event) {
    $eventType = $event['type'];
    $data = $event['data'];

    switch ($eventType) {
        case 'order.created':
            handleOrderCreated($data);
            break;
        case 'order.updated':
            handleOrderUpdated($data);
            break;
        case 'payment.succeeded':
            handlePaymentSucceeded($data);
            break;
        case 'shipment.delivered':
            handleShipmentDelivered($data);
            break;
        case 'inventory.low_stock':
            handleLowStock($data);
            break;
        default:
            error_log("Unhandled event type: $eventType");
    }
}

function handleOrderCreated($order) {
    error_log("Order created: {$order['id']}");
    // Your business logic here
}

function handleOrderUpdated($order) {
    error_log("Order updated: {$order['id']}, Status: {$order['status']}");
    // Your business logic here
}

function handlePaymentSucceeded($payment) {
    error_log("Payment succeeded: {$payment['id']}");
    // Your business logic here
}

function handleShipmentDelivered($shipment) {
    error_log("Shipment delivered: {$shipment['id']}");
    // Your business logic here
}

function handleLowStock($inventory) {
    error_log("Low stock alert: {$inventory['sku']}, Quantity: {$inventory['quantity_available']}");
    // Your business logic here
}
```

---

## Go Handler

```go
package main

import (
	"crypto/hmac"
	"crypto/sha256"
	"crypto/subtle"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
)

type WebhookEvent struct {
	Type string                 `json:"type"`
	Data map[string]interface{} `json:"data"`
}

func main() {
	http.HandleFunc("/webhooks/stateset", handleWebhook)

	port := os.Getenv("PORT")
	if port == "" {
		port = "3000"
	}

	log.Printf("Webhook server listening on port %s", port)
	log.Fatal(http.ListenAndServe(":"+port, nil))
}

func handleWebhook(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Read body
	body, err := io.ReadAll(r.Body)
	if err != nil {
		http.Error(w, "Failed to read body", http.StatusBadRequest)
		return
	}
	defer r.Body.Close()

	// Get signature
	signature := r.Header.Get("X-StateSet-Signature")
	if signature == "" {
		http.Error(w, "Missing signature", http.StatusUnauthorized)
		return
	}

	// Verify signature
	webhookSecret := os.Getenv("STATESET_WEBHOOK_SECRET")
	if !verifySignature(body, signature, webhookSecret) {
		log.Println("Invalid webhook signature")
		http.Error(w, "Invalid signature", http.StatusUnauthorized)
		return
	}

	// Parse event
	var event WebhookEvent
	if err := json.Unmarshal(body, &event); err != nil {
		http.Error(w, "Invalid JSON", http.StatusBadRequest)
		return
	}

	log.Printf("Received webhook event: %s", event.Type)

	// Handle event
	if err := handleWebhookEvent(event); err != nil {
		log.Printf("Error handling webhook: %v", err)
		http.Error(w, "Internal server error", http.StatusInternalServerError)
		return
	}

	w.WriteHeader(http.StatusOK)
	json.NewEncoder(w).Encode(map[string]bool{"received": true})
}

func verifySignature(body []byte, signature, secret string) bool {
	mac := hmac.New(sha256.New, []byte(secret))
	mac.Write(body)
	expectedSignature := hex.EncodeToString(mac.Sum(nil))

	// Use constant-time comparison
	return subtle.ConstantTimeCompare(
		[]byte(signature),
		[]byte(expectedSignature),
	) == 1
}

func handleWebhookEvent(event WebhookEvent) error {
	switch event.Type {
	case "order.created":
		return handleOrderCreated(event.Data)
	case "order.updated":
		return handleOrderUpdated(event.Data)
	case "payment.succeeded":
		return handlePaymentSucceeded(event.Data)
	case "shipment.delivered":
		return handleShipmentDelivered(event.Data)
	case "inventory.low_stock":
		return handleLowStock(event.Data)
	default:
		log.Printf("Unhandled event type: %s", event.Type)
	}
	return nil
}

func handleOrderCreated(data map[string]interface{}) error {
	fmt.Printf("Order created: %v\n", data["id"])
	// Your business logic here
	return nil
}

func handleOrderUpdated(data map[string]interface{}) error {
	fmt.Printf("Order updated: %v, Status: %v\n", data["id"], data["status"])
	// Your business logic here
	return nil
}

func handlePaymentSucceeded(data map[string]interface{}) error {
	fmt.Printf("Payment succeeded: %v\n", data["id"])
	// Your business logic here
	return nil
}

func handleShipmentDelivered(data map[string]interface{}) error {
	fmt.Printf("Shipment delivered: %v\n", data["id"])
	// Your business logic here
	return nil
}

func handleLowStock(data map[string]interface{}) error {
	fmt.Printf("Low stock alert: %v, Quantity: %v\n", data["sku"], data["quantity_available"])
	// Your business logic here
	return nil
}
```

---

## Testing Webhooks

### Using cURL

```bash
# Generate signature
WEBHOOK_SECRET="your-webhook-secret"
PAYLOAD='{"type":"order.created","data":{"id":"order-123","status":"pending"}}'
SIGNATURE=$(echo -n "$PAYLOAD" | openssl dgst -sha256 -hmac "$WEBHOOK_SECRET" | sed 's/.* //')

# Send webhook
curl -X POST http://localhost:3000/webhooks/stateset \
  -H "Content-Type: application/json" \
  -H "X-StateSet-Signature: $SIGNATURE" \
  -d "$PAYLOAD"
```

### Using webhook.site

1. Go to https://webhook.site
2. Copy the unique URL
3. Configure this URL in your StateSet account webhook settings
4. View incoming webhooks in real-time

### Local Testing with ngrok

```bash
# Install ngrok
brew install ngrok  # or download from https://ngrok.com

# Start your webhook server
npm start  # or python app.py, etc.

# Expose your local server
ngrok http 3000

# Use the ngrok URL in your webhook settings
# Example: https://abc123.ngrok.io/webhooks/stateset
```

---

## Best Practices

1. **Always Verify Signatures**: Never process webhooks without verifying the signature
2. **Respond Quickly**: Return a 200 response immediately, process async if needed
3. **Idempotency**: Store event IDs to prevent duplicate processing
4. **Retry Logic**: Handle failures gracefully, StateSet will retry failed webhooks
5. **Monitoring**: Log all webhook events and failures for debugging
6. **Security**: Use HTTPS in production, never log sensitive data

---

## Additional Resources

- [Main README](../README.md)
- [API Overview](../API_OVERVIEW.md)
- [Advanced Workflows](./ADVANCED_WORKFLOWS.md)
- [Integration Guide](../docs/INTEGRATION_GUIDE.md)
