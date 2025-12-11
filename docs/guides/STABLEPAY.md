# StablePay - Enterprise Payment System

## Overview

**StablePay** is an enterprise-grade payment processing system designed for retail businesses that need:

- 🚀 **Instant Global Payments** - Process payments in multiple currencies with instant settlement
- 🔄 **Auto-Reconciliation** - Automatically match and reconcile payments with provider statements
- 💰 **Reduced Costs** - Save up to 47% on payment processing fees (1.5% vs 2.9% industry standard)
- 🌍 **Multi-Currency Support** - Accept payments in USD, EUR, GBP, JPY, CAD, AUD and more
- 🔒 **Enterprise Security** - Built-in fraud detection, idempotency, and risk scoring
- 📊 **Real-time Analytics** - Track payment performance, success rates, and cost optimization

## Key Features

### 1. Intelligent Payment Routing

StablePay automatically routes payments to the optimal provider based on:
- **Currency** - Select providers with best rates for specific currencies
- **Transaction Amount** - Route high-value transactions to lower-fee providers
- **Geography** - Use region-specific providers for better success rates
- **Cost Optimization** - Always choose the lowest-cost provider

### 2. Auto-Reconciliation

Eliminate manual reconciliation work with our automated system:
- **95%+ Match Rate** - Automatically match internal and external transactions
- **Smart Matching** - Uses amount, currency, date, and metadata for accurate matching
- **Discrepancy Detection** - Flags mismatches for review
- **Audit Trail** - Complete history of all reconciliation activities

### 3. Cost Savings

#### Industry Comparison

| Provider | Fee Structure | $500 Transaction | $10,000 Transaction |
|----------|--------------|------------------|---------------------|
| **StablePay** | 1.5% + $0.30 | **$7.80** | **$150.30** |
| Stripe | 2.9% + $0.30 | $14.80 | $290.30 |
| PayPal | 3.49% + $0.49 | $17.94 | $349.49 |

#### Annual Savings Example

For a business processing **10,000 transactions per month** at an average of **$500 per transaction**:

- **StablePay**: $936,000/year in fees
- **Stripe**: $1,776,000/year in fees
- **Savings**: **$840,000 per year**

### 4. Multi-Currency Support

Process payments in 150+ currencies with:
- Real-time exchange rates
- Automatic currency conversion
- Local payment methods
- Transparent fee breakdown

### 5. Enterprise Security

- **PCI DSS Level 1 Compliant**
- **Real-time Fraud Detection** - Risk scoring on every transaction
- **Idempotency Keys** - Prevent duplicate charges
- **3D Secure Support** - Enhanced authentication
- **Encrypted Storage** - All sensitive data encrypted at rest

### 6. Developer-Friendly API

- RESTful API design
- Comprehensive webhooks
- Detailed error messages
- Extensive documentation
- SDKs for major languages

### 7. Crypto Payments (NEW!) 🚀

- **USDC & USDT Support** - Accept stablecoins
- **Multi-Blockchain** - Ethereum, Polygon, Arbitrum, Optimism, Base
- **Ultra-Low Fees** - 0.5% (83% cheaper than cards)
- **Instant Settlement** - Minutes instead of days
- **No Chargebacks** - Irreversible transactions
- **Global Access** - Accept from anywhere in the world

[Learn more about StablePay Crypto →](./STABLEPAY_CRYPTO.md)

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────────┐
│                   StablePay API                         │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐  │
│  │   Payment    │  │ Routing      │  │ Fraud       │  │
│  │   Service    │  │ Engine       │  │ Detection   │  │
│  └──────────────┘  └──────────────┘  └─────────────┘  │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐  │
│  │ Reconciliation│  │   Provider   │  │  Analytics  │  │
│  │   Service    │  │   Gateway    │  │   Engine    │  │
│  └──────────────┘  └──────────────┘  └─────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
                          │
        ┌─────────────────┼─────────────────┐
        ▼                 ▼                 ▼
  ┌──────────┐      ┌──────────┐     ┌──────────┐
  │  Stripe  │      │  PayPal  │     │  Direct  │
  └──────────┘      └──────────┘     └──────────┘
```

### Database Schema

Key tables:
- `stablepay_providers` - Payment provider configurations
- `stablepay_transactions` - Payment transactions
- `stablepay_payment_methods` - Stored customer payment methods
- `stablepay_refunds` - Refund records
- `stablepay_reconciliations` - Reconciliation results
- `stablepay_batches` - Batch processing for bulk operations

## API Reference

### Base URL

```
https://api.stateset.io/api/v1/stablepay
```

### Authentication

All API requests require authentication via Bearer token:

```bash
Authorization: Bearer YOUR_API_KEY
```

### Endpoints

#### Create Payment

```http
POST /payments
```

**Request:**
```json
{
  "customer_id": "550e8400-e29b-41d4-a716-446655440000",
  "order_id": "660e8400-e29b-41d4-a716-446655440000",
  "amount": "499.99",
  "currency": "USD",
  "description": "Premium Subscription",
  "metadata": {
    "subscription_id": "sub_12345"
  },
  "idempotency_key": "unique-key-12345"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "770e8400-e29b-41d4-a716-446655440000",
    "transaction_number": "PAY-20251013-A1B2C3D4",
    "customer_id": "550e8400-e29b-41d4-a716-446655440000",
    "order_id": "660e8400-e29b-41d4-a716-446655440000",
    "amount": "499.99",
    "currency": "USD",
    "status": "succeeded",
    "provider_name": "StablePay Direct",
    "provider_fee": "7.50",
    "platform_fee": "2.50",
    "total_fees": "10.00",
    "net_amount": "489.99",
    "initiated_at": "2025-10-13T12:00:00Z",
    "processed_at": "2025-10-13T12:00:01Z",
    "estimated_settlement_date": "2025-10-15"
  }
}
```

#### Get Payment

```http
GET /payments/{id}
```

#### List Customer Payments

```http
GET /customers/{customer_id}/payments?limit=20&offset=0
```

#### Create Refund

```http
POST /refunds
```

**Request:**
```json
{
  "transaction_id": "770e8400-e29b-41d4-a716-446655440000",
  "amount": "100.00",
  "reason": "requested_by_customer",
  "reason_detail": "Customer requested refund"
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "880e8400-e29b-41d4-a716-446655440000",
    "refund_number": "REF-20251013-E5F6G7H8",
    "transaction_id": "770e8400-e29b-41d4-a716-446655440000",
    "amount": "100.00",
    "currency": "USD",
    "status": "succeeded",
    "refunded_fees": "2.00",
    "net_refund": "98.00",
    "requested_at": "2025-10-13T14:00:00Z"
  }
}
```

#### Run Reconciliation

```http
POST /reconciliations
```

**Request:**
```json
{
  "provider_id": "990e8400-e29b-41d4-a716-446655440000",
  "period_start": "2025-10-01",
  "period_end": "2025-10-31",
  "external_transactions": [
    {
      "external_id": "ext_12345",
      "amount": "499.99",
      "currency": "USD",
      "date": "2025-10-13T12:00:00Z",
      "status": "succeeded"
    }
  ]
}
```

**Response:**
```json
{
  "success": true,
  "data": {
    "id": "aa0e8400-e29b-41d4-a716-446655440000",
    "reconciliation_number": "REC-20251013-I9J0K1L2",
    "provider_id": "990e8400-e29b-41d4-a716-446655440000",
    "period_start": "2025-10-01",
    "period_end": "2025-10-31",
    "total_transactions": 1523,
    "matched_transactions": 1489,
    "unmatched_transactions": 34,
    "discrepancy_count": 2,
    "discrepancy_amount": "0.52",
    "match_rate": "97.77",
    "status": "completed"
  }
}
```

#### Get Reconciliation Stats

```http
GET /providers/{provider_id}/reconciliation-stats?days=30
```

## Quick Start

### 1. Installation

```bash
# Clone the repository
git clone https://github.com/stateset/stateset-api.git
cd stateset-api

# Install dependencies
cargo build

# Run migrations
cargo run --bin migrate

# Start the server
cargo run --bin stateset-api
```

### 2. Configuration

Create a `.env` file:

```env
DATABASE_URL=postgresql://user:password@localhost/stateset
REDIS_URL=redis://localhost:6379
STABLEPAY_API_KEY=your_api_key_here
```

### 3. Run Demo

```bash
# Run the StablePay demo
./demos/stablepay_demo.sh
```

### 4. Create Your First Payment

```bash
curl -X POST http://localhost:8000/api/v1/stablepay/payments \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_API_KEY" \
  -d '{
    "customer_id": "550e8400-e29b-41d4-a716-446655440000",
    "amount": "99.99",
    "currency": "USD",
    "description": "Test Payment"
  }'
```

## Integration Examples

### Node.js/TypeScript

```typescript
import axios from 'axios';

const stablepay = axios.create({
  baseURL: 'https://api.stateset.io/api/v1/stablepay',
  headers: {
    'Authorization': `Bearer ${process.env.STABLEPAY_API_KEY}`,
    'Content-Type': 'application/json'
  }
});

// Create a payment
async function createPayment() {
  const response = await stablepay.post('/payments', {
    customer_id: '550e8400-e29b-41d4-a716-446655440000',
    amount: '499.99',
    currency: 'USD',
    description: 'Premium Subscription',
    idempotency_key: `payment-${Date.now()}`
  });
  
  return response.data.data;
}

// Create a refund
async function createRefund(transactionId: string, amount: string) {
  const response = await stablepay.post('/refunds', {
    transaction_id: transactionId,
    amount: amount,
    reason: 'requested_by_customer'
  });
  
  return response.data.data;
}
```

### Python

```python
import requests
import os

class StablePayClient:
    def __init__(self, api_key: str):
        self.base_url = "https://api.stateset.io/api/v1/stablepay"
        self.headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json"
        }
    
    def create_payment(self, customer_id: str, amount: str, currency: str = "USD"):
        response = requests.post(
            f"{self.base_url}/payments",
            headers=self.headers,
            json={
                "customer_id": customer_id,
                "amount": amount,
                "currency": currency,
                "description": "Payment via Python SDK"
            }
        )
        return response.json()["data"]
    
    def create_refund(self, transaction_id: str, amount: str):
        response = requests.post(
            f"{self.base_url}/refunds",
            headers=self.headers,
            json={
                "transaction_id": transaction_id,
                "amount": amount,
                "reason": "requested_by_customer"
            }
        )
        return response.json()["data"]

# Usage
client = StablePayClient(os.getenv("STABLEPAY_API_KEY"))
payment = client.create_payment(
    customer_id="550e8400-e29b-41d4-a716-446655440000",
    amount="99.99"
)
print(f"Payment created: {payment['id']}")
```

### Rust

```rust
use serde::{Deserialize, Serialize};
use reqwest;

#[derive(Serialize)]
struct CreatePaymentRequest {
    customer_id: String,
    amount: String,
    currency: String,
    description: String,
}

#[derive(Deserialize)]
struct PaymentResponse {
    id: String,
    transaction_number: String,
    status: String,
    net_amount: String,
}

async fn create_payment(api_key: &str) -> Result<PaymentResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let request = CreatePaymentRequest {
        customer_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        amount: "99.99".to_string(),
        currency: "USD".to_string(),
        description: "Test Payment".to_string(),
    };
    
    let response = client
        .post("https://api.stateset.io/api/v1/stablepay/payments")
        .bearer_auth(api_key)
        .json(&request)
        .send()
        .await?
        .json::<PaymentResponse>()
        .await?;
    
    Ok(response)
}
```

## Webhooks

StablePay sends webhooks for important events:

### Event Types

- `payment.succeeded` - Payment successfully processed
- `payment.failed` - Payment failed
- `payment.refunded` - Payment refunded
- `reconciliation.completed` - Reconciliation completed
- `reconciliation.requires_review` - Reconciliation needs manual review

### Webhook Payload

```json
{
  "id": "evt_12345",
  "type": "payment.succeeded",
  "created_at": "2025-10-13T12:00:00Z",
  "data": {
    "payment_id": "770e8400-e29b-41d4-a716-446655440000",
    "transaction_number": "PAY-20251013-A1B2C3D4",
    "customer_id": "550e8400-e29b-41d4-a716-446655440000",
    "amount": "499.99",
    "currency": "USD",
    "status": "succeeded"
  }
}
```

### Verifying Webhooks

```typescript
import crypto from 'crypto';

function verifyWebhook(payload: string, signature: string, secret: string): boolean {
  const hmac = crypto.createHmac('sha256', secret);
  hmac.update(payload);
  const digest = hmac.digest('hex');
  return crypto.timingSafeEqual(Buffer.from(signature), Buffer.from(digest));
}
```

## Monitoring & Analytics

### Key Metrics

Monitor your payment performance with built-in analytics:

- **Success Rate** - % of successful payments
- **Average Processing Time** - Time from initiation to settlement
- **Total Volume** - Total payment volume
- **Fee Analysis** - Breakdown of costs by provider
- **Reconciliation Match Rate** - Auto-reconciliation accuracy

### Dashboard Access

```bash
# View analytics
curl https://api.stateset.io/api/v1/stablepay/analytics \
  -H "Authorization: Bearer YOUR_API_KEY"
```

## Best Practices

### 1. Use Idempotency Keys

Always use idempotency keys for payment creation to prevent duplicate charges:

```javascript
const idempotencyKey = `payment-${customerId}-${Date.now()}`;
```

### 2. Handle Webhook Events

Implement webhook handlers for asynchronous payment updates:

```javascript
app.post('/webhooks/stablepay', async (req, res) => {
  const event = req.body;
  
  switch (event.type) {
    case 'payment.succeeded':
      await fulfillOrder(event.data.order_id);
      break;
    case 'payment.failed':
      await notifyCustomer(event.data.customer_id);
      break;
  }
  
  res.status(200).send('OK');
});
```

### 3. Implement Retry Logic

Implement exponential backoff for API calls:

```javascript
async function createPaymentWithRetry(request, maxRetries = 3) {
  for (let i = 0; i < maxRetries; i++) {
    try {
      return await createPayment(request);
    } catch (error) {
      if (i === maxRetries - 1) throw error;
      await sleep(Math.pow(2, i) * 1000);
    }
  }
}
```

### 4. Store Payment Method Details Securely

Never store full card numbers. Use tokenized payment methods:

```javascript
// Store only safe details
const paymentMethod = {
  id: 'pm_12345',
  last_four: '4242',
  brand: 'visa',
  exp_month: 12,
  exp_year: 2025
};
```

## Troubleshooting

### Common Issues

#### Payment Failed

```json
{
  "success": false,
  "error": "Payment declined by provider"
}
```

**Solution**: Check customer payment method, verify sufficient funds, retry with different method.

#### Reconciliation Discrepancies

**Solution**: Review unmatched transactions manually, verify date ranges, check for timezone differences.

#### High Failure Rate

**Solution**: Enable 3D Secure, review fraud detection settings, contact support for provider optimization.

## Support

### Documentation

- [Full API Documentation](https://docs.stateset.io/stablepay)
- [Integration Guides](https://docs.stateset.io/guides)
- [Code Examples](https://github.com/stateset/stablepay-examples)

### Contact

- **Email**: stablepay@stateset.io
- **Slack**: [Join our community](https://stateset.slack.com)
- **Phone**: +1 (555) 123-4567

## Roadmap

### Q4 2025

- [x] Cryptocurrency payments (USDC, USDT) ✅ **LIVE NOW!**
- [ ] Buy Now, Pay Later (BNPL) integration
- [ ] Advanced fraud detection with ML
- [ ] Mobile SDKs (iOS, Android)

### Q1 2026

- [ ] ACH direct debit
- [ ] International bank transfers (SEPA, SWIFT)
- [ ] Subscription management
- [ ] Enhanced analytics dashboard

## License

StablePay is part of the StateSet API and is licensed under the same Business Source License (BSL) 1.1 as the rest of the project, unless explicitly stated otherwise. See [LICENSE](../../LICENSE).

## Credits

Built with ❤️ by the Stateset team.

---

**Ready to reduce your payment costs by 47%?**

[Get Started](https://stateset.io/stablepay/signup) | [View Demo](./demos/stablepay_demo.sh) | [Read Docs](https://docs.stateset.io/stablepay)
