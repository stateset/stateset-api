# StateSet API Examples

This directory contains practical examples for using the StateSet API in different programming languages and scenarios.

## Available Examples

### 📚 Documentation

- **[api-examples.md](./api-examples.md)** - Comprehensive guide with examples in cURL, JavaScript, and Python covering all major API endpoints

### 💻 Code Examples

- **[javascript-example.js](./javascript-example.js)** - Complete Node.js client with working examples
- **[python-example.py](./python-example.py)** - Complete Python client with working examples
- **[curl-examples.sh](./curl-examples.sh)** - Bash script demonstrating API workflows with cURL

## Quick Start

### JavaScript/Node.js

```bash
# Install dependencies
npm install axios

# Run the example
node javascript-example.js
```

Before running, update the credentials in the script:
```javascript
const config = {
  email: 'your-email@example.com',
  password: 'your-password'
};
```

### Python

```bash
# Install dependencies
pip install requests

# Run the example
python python-example.py
```

Before running, update the credentials in the script:
```python
EMAIL = 'your-email@example.com'
PASSWORD = 'your-password'
```

### cURL (Bash)

```bash
# Make the script executable
chmod +x curl-examples.sh

# Run the script
./curl-examples.sh
```

Before running, update the credentials in the script:
```bash
EMAIL="your-email@example.com"
PASSWORD="your-password"
```

## What's Covered

All examples demonstrate the following workflows:

1. **Authentication**
   - User login with JWT tokens
   - API key creation and usage
   - Token-based authentication

2. **Order Management**
   - Creating orders
   - Listing and filtering orders
   - Updating order status
   - Cancelling orders

3. **Inventory Management**
   - Listing inventory items
   - Checking low stock
   - Reserving inventory
   - Releasing inventory

4. **Returns Processing**
   - Creating return requests
   - Approving returns
   - Restocking items

5. **Shipment Tracking**
   - Creating shipments
   - Marking as shipped
   - Tracking shipments

6. **Payments**
   - Processing payments
   - Handling refunds

7. **Analytics**
   - Dashboard metrics
   - Sales trends

## API Client Classes

The JavaScript and Python examples include full-featured client classes that you can use in your own projects:

### JavaScript
```javascript
const StateSetClient = require('./javascript-example.js');

const client = new StateSetClient('http://localhost:8080/api/v1');
await client.login('user@example.com', 'password');
const orders = await client.listOrders({ status: 'pending' });
```

### Python
```python
from python_example import StateSetClient

client = StateSetClient('http://localhost:8080/api/v1')
client.login('user@example.com', 'password')
orders = client.list_orders(status='pending')
```

## Important Notes

### Replace Placeholder IDs

The examples use placeholder UUIDs for customer IDs, product IDs, etc. Before running, you should:

1. Create test customers and products in your database, OR
2. Replace the placeholder IDs with actual IDs from your database

Example placeholders to replace:
- `customer_id`: `550e8400-e29b-41d4-a716-446655440001`
- `product_id`: `550e8400-e29b-41d4-a716-446655440002`

### Authentication Requirements

Most endpoints require authentication. The examples demonstrate two methods:

1. **JWT Tokens** (recommended for user sessions)
   - Login to get access and refresh tokens
   - Include in `Authorization: Bearer <token>` header

2. **API Keys** (recommended for service-to-service)
   - Create via `/auth/api-keys` endpoint
   - Include in `X-API-Key: <key>` header

### Error Handling

All client implementations include error handling. Check the console output for detailed error messages if requests fail.

### Idempotency

The payment examples demonstrate the use of idempotency keys to prevent duplicate charges. This is recommended for all write operations:

```javascript
headers: {
  'Idempotency-Key': generateUUID()
}
```

## Interactive API Documentation

For interactive API exploration, visit the Swagger UI:

```
http://localhost:8080/swagger-ui
```

## Comprehensive Documentation

### Core Documentation

- **[API Overview](../docs/API_OVERVIEW.md)** - Complete API reference with architecture, capabilities, and features
- **[Use Cases Guide](../docs/USE_CASES.md)** - Real-world implementation scenarios including:
  - E-Commerce Store
  - Omnichannel Retail (BOPIS)
  - Manufacturing & Production
  - Subscription Box Service
  - B2B Wholesale
  - AI-Powered Shopping (ChatGPT)
  - Crypto Commerce
- **[Integration Guide](../docs/INTEGRATION_GUIDE.md)** - Production-ready integration patterns including:
  - Authentication strategies
  - Webhook implementation
  - Error handling patterns
  - Rate limiting & throttling
  - Idempotency implementation
  - Third-party platform integrations

### Additional Resources

- **Main Documentation**: See the root [README.md](../README.md)
- **Deployment Guide**: See [docs/DEPLOYMENT.md](../docs/DEPLOYMENT.md)
- **API Operations Guide**: See [docs/api_operations_overview.md](../docs/api_operations_overview.md)

## Additional Resources

### Testing with the CLI

StateSet also provides a command-line tool for quick API testing:

```bash
# Build and install the CLI
cargo build --bin stateset-cli

# Login
./target/debug/stateset-cli auth login \
  --email admin@stateset.com \
  --password your-password \
  --save

# List orders
./target/debug/stateset-cli orders list --status pending

# Create an order
./target/debug/stateset-cli orders create \
  --customer-id <uuid> \
  --item sku=SKU-123,quantity=2,price=19.99
```

### Common Workflows

**E-commerce Checkout Flow:**
1. Create/update cart
2. Start checkout session
3. Set customer info
4. Set shipping address
5. Select shipping method
6. Process payment
7. Complete checkout (creates order)

**Order Fulfillment Flow:**
1. Create order
2. Reserve inventory
3. Create shipment
4. Mark as shipped with tracking
5. Update order status
6. Customer receives and tracks shipment

**Return Processing Flow:**
1. Customer creates return request
2. Approve return
3. Customer ships back
4. Receive and inspect items
5. Restock inventory
6. Process refund

## Need Help?

If you encounter issues with these examples:

1. Ensure the API server is running (`cargo run`)
2. Check that your database is properly set up
3. Verify your credentials are correct
4. Check the API logs for detailed error messages
5. Refer to the Swagger documentation for endpoint details

## Contributing

Feel free to contribute additional examples or improvements:

- Add examples in other languages (Ruby, Go, PHP, etc.)
- Demonstrate advanced workflows
- Add integration examples with popular frameworks
- Improve error handling and resilience patterns
