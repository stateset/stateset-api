# StateSet API Examples

This directory contains practical examples for using the StateSet API in different programming languages and scenarios.

## Available Examples

### 📚 Documentation

- **[api-examples.md](./api-examples.md)** - Comprehensive guide with examples in cURL, JavaScript, and Python covering all major API endpoints
- **[ADVANCED_WORKFLOWS.md](./ADVANCED_WORKFLOWS.md)** - Advanced workflow examples including complete checkout flows, order fulfillment, returns processing, and more
- **[MANUFACTURING_EXAMPLES.md](./MANUFACTURING_EXAMPLES.md)** - Comprehensive manufacturing and production examples with shell scripts, Python, and TypeScript

### 🏭 Manufacturing & Production Examples

- **[manufacturing-client.py](./manufacturing-client.py)** - Complete Python client for manufacturing operations (BOM, work orders, batch production, traceability)
- **[manufacturing-client.ts](./manufacturing-client.ts)** - Fully-typed TypeScript client for manufacturing workflows
- **Shell Script Demos** (in `/demos/`):
  - `demo_1_robot_build.sh` - Robot manufacturing with component traceability
  - `demo_2_quality_issue.sh` - Quality management and NCR workflows
  - `demo_3_production_dashboard.sh` - Real-time production monitoring
  - `demo_4_production_scheduling.sh` - Multi-work order scheduling and MRP
  - `demo_5_batch_production.sh` - Batch/lot tracking for regulated industries
  - `demo_6_supply_chain_integration.sh` - End-to-end supply chain workflows

See **[MANUFACTURING_EXAMPLES.md](./MANUFACTURING_EXAMPLES.md)** for detailed manufacturing documentation and usage guides.

### 💻 Code Examples

#### JavaScript/TypeScript
- **[typescript-example.ts](./typescript-example.ts)** - Modern TypeScript client with comprehensive type definitions and async/await patterns
- **[javascript-example.js](./javascript-example.js)** - Complete Node.js client with working examples

#### Python
- **[python-example.py](./python-example.py)** - Complete Python client with working examples

#### Go
- **[go-example.go](./go-example.go)** - Full-featured Go client implementation

#### Ruby
- **[ruby-example.rb](./ruby-example.rb)** - Complete Ruby client with HTTParty

#### PHP
- **[php-example.php](./php-example.php)** - Full-featured PHP client using Guzzle

#### Shell Scripts
- **[curl-examples.sh](./curl-examples.sh)** - Bash script demonstrating API workflows with cURL

### 🧪 Testing Tools

- **[StateSet-API.postman_collection.json](./StateSet-API.postman_collection.json)** - Complete Postman collection for interactive API testing

### 🔧 Integration Examples

- **[react-nextjs-integration.tsx](./react-nextjs-integration.tsx)** - Complete React/Next.js integration with hooks, context, and TypeScript
- **[WEBHOOK_HANDLERS.md](./WEBHOOK_HANDLERS.md)** - Webhook handler implementations for Express, Next.js, Flask, FastAPI, PHP, Go, and Ruby

### 🐳 Development Tools

- **[docker-compose.dev.yml](./docker-compose.dev.yml)** - Docker Compose configuration for local development
- **[DOCKER_DEVELOPMENT.md](./DOCKER_DEVELOPMENT.md)** - Complete guide to using Docker for local development

## Quick Start

### TypeScript/Node.js (Recommended)

```bash
# Install dependencies
npm install axios uuid @types/node @types/uuid

# If using ts-node
npm install -g ts-node

# Run the example
ts-node typescript-example.ts
```

Before running, update the credentials in the script:
```typescript
await client.login('admin@stateset.com', 'your-password');
```

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

### Go

```bash
# Install dependencies
go get github.com/google/uuid

# Run the example
go run go-example.go
```

Before running, update the credentials in the script:
```go
client.Login("admin@stateset.com", "your-password")
```

### Ruby

```bash
# Install dependencies
gem install httparty

# Make the script executable
chmod +x ruby-example.rb

# Run the example
ruby ruby-example.rb
```

Before running, update the credentials in the script:
```ruby
client.login('admin@stateset.com', 'your-password')
```

### PHP

```bash
# Install dependencies
composer require guzzlehttp/guzzle

# Run the example
php php-example.php
```

Before running, update the credentials in the script:
```php
$client->login('admin@stateset.com', 'your-password');
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

### Postman Collection

1. Import `StateSet-API.postman_collection.json` into Postman
2. Set the `base_url` environment variable to your API endpoint
3. Run the "Login" request first to automatically set the `access_token`
4. All subsequent requests will use the token automatically

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

8. **Manufacturing & Production** (See [MANUFACTURING_EXAMPLES.md](./MANUFACTURING_EXAMPLES.md))
   - Bill of Materials (BOM) management
   - Work order lifecycle (create, start, complete, hold, resume, cancel)
   - Batch production and lot tracking
   - Component serial number tracking
   - Robot manufacturing and traceability
   - Quality control and testing
   - Production scheduling and capacity planning
   - Supply chain integration (procurement to fulfillment)
   - Production analytics and metrics

## API Client Classes

All code examples include full-featured client classes that you can use in your own projects:

### TypeScript (Recommended)
```typescript
import StateSetClient from './typescript-example';

const client = new StateSetClient('http://localhost:8080/api/v1');
await client.login('user@example.com', 'password');

// All methods are fully typed
const orders = await client.listOrders({ status: 'pending', page: 1, limit: 10 });
const cart = await client.createCart(customerId);
```

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

### Go
```go
package main

import "github.com/stateset/stateset-api/examples"

client := NewStateSetClient("http://localhost:8080/api/v1")
err := client.Login("user@example.com", "password")
orders, err := client.ListOrders(1, 10)
```

### Ruby
```ruby
require './ruby-example'

client = StateSetClient.new('http://localhost:8080/api/v1')
client.login('user@example.com', 'password')
orders = client.list_orders(page: 1, limit: 10)
```

### PHP
```php
<?php
require 'vendor/autoload.php';

$client = new StateSetClient('http://localhost:8080/api/v1');
$client->login('user@example.com', 'password');
$orders = $client->listOrders(['page' => 1, 'limit' => 10]);
```

## React/Next.js Integration

For React and Next.js applications, see [react-nextjs-integration.tsx](./react-nextjs-integration.tsx) which includes:

- **Auth Context** - Complete authentication state management
- **Custom Hooks** - `useOrders()`, `useCart()`, `useProducts()`, etc.
- **Example Components** - Login form, order list, shopping cart, product grid
- **TypeScript Support** - Fully typed API client and responses
- **SWR Integration** - Automatic caching and revalidation

```typescript
import { AuthProvider, useOrders, useCart } from './react-nextjs-integration';

// Wrap your app
function App() {
  return (
    <AuthProvider>
      <YourApp />
    </AuthProvider>
  );
}

// Use hooks in your components
function OrdersPage() {
  const { orders, isLoading, error } = useOrders({ page: 1, limit: 10 });
  // ...
}
```

## Webhook Handlers

For webhook integration, see [WEBHOOK_HANDLERS.md](./WEBHOOK_HANDLERS.md) with implementations for:

- Express.js/Node.js
- Next.js API Routes
- Python (Flask & FastAPI)
- PHP
- Go
- Ruby/Sinatra

All examples include proper signature verification and event handling.

## Docker Development

For local development with Docker, see [DOCKER_DEVELOPMENT.md](./DOCKER_DEVELOPMENT.md):

```bash
# Start the complete development environment
docker-compose -f docker-compose.dev.yml up

# Access services:
# - API: http://localhost:8080
# - Database UI: http://localhost:8081
# - Redis UI: http://localhost:8001
# - Email Testing: http://localhost:8025
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

## Advanced Workflows

For detailed, production-ready workflow implementations, see [ADVANCED_WORKFLOWS.md](./ADVANCED_WORKFLOWS.md), which includes:

- **Complete E-Commerce Checkout Flow** - Full checkout from cart to order completion
- **Order Fulfillment Workflow** - End-to-end fulfillment with inventory reservations
- **Returns Processing Workflow** - Complete returns with inspection and refunds
- **Inventory Management with Reservations** - Automatic reservation handling
- **Subscription Order Management** - Recurring order implementation
- **Error Handling and Retry Patterns** - Robust error handling with exponential backoff
- **Idempotency Best Practices** - Prevent duplicate operations
- **Webhooks Integration** - Handle webhook events securely
- **Batch Operations** - Process multiple operations efficiently

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
