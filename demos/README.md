# StateSet API Demo Scripts

This directory contains comprehensive demo scripts that showcase various features and workflows of the StateSet API.

## Available Demos

### 1. E-commerce Checkout Flow (`ecommerce_checkout_flow.sh`)
Demonstrates a complete e-commerce transaction flow including:
- Shopping cart management
- Customer creation
- Order placement
- Payment processing
- Order fulfillment

### 2. Inventory Management Flow (`inventory_management_flow.sh`)
Showcases inventory operations including:
- Product creation and warehouse setup
- Inventory level management
- Stock adjustments and transfers
- Inventory allocation and reservations
- Cycle counting and reporting
- Automatic reorder rules

### 3. Returns Processing Flow (`returns_processing_flow.sh`)
Complete RMA (Return Merchandise Authorization) workflow:
- Return request initiation
- RMA generation and approval
- Return shipping label creation
- Item receiving and inspection
- Replacement/refund processing
- Customer notifications

### 4. Warranty Claim Flow (`warranty_claim_flow.sh`)
End-to-end warranty management:
- Warranty registration
- Claim filing with documentation
- Claim review and approval
- Replacement order creation
- Warranty transfer to replacement unit
- Analytics and reporting

### 5. Purchase Order Flow (`purchase_order_flow.sh`)
Comprehensive procurement process:
- Supplier management
- Purchase order creation and approval
- Advanced Shipping Notice (ASN) handling
- Receiving and quality inspection
- Invoice processing and 3-way matching
- Payment scheduling

### 6. Shipment Tracking Flow (`shipment_tracking_flow.sh`)
Multi-carrier shipping and tracking:
- Order fulfillment workflow
- Pick, pack, and ship operations
- Multi-carrier rate comparison
- Real-time tracking updates
- Delivery confirmation
- Exception handling

### 7. Customer Lifecycle Flow (`customer_lifecycle_flow.sh`)
Complete customer journey management:
- Customer onboarding (B2C and B2B)
- Segmentation and targeting
- Loyalty program enrollment
- Customer engagement tracking
- Marketing campaigns
- CLV calculation and analytics
- GDPR compliance

## Running the Demos

### Prerequisites
- StateSet API server running on `http://localhost:8080`
- `jq` installed for JSON parsing
- `curl` for API requests
- Valid authentication token (set via `AUTH_TOKEN` environment variable)

### Basic Usage
```bash
# Run a demo with default settings
./demos/inventory_management_flow.sh

# Run with custom API URL and auth token
API_URL=http://api.example.com AUTH_TOKEN=your-token ./demos/returns_processing_flow.sh
```

### Environment Variables
- `API_URL`: API server URL (default: `http://localhost:8080`)
- `AUTH_TOKEN`: Authentication token (default: `test-token`)

## Demo Features

Each demo script includes:
- Step-by-step progression with clear output
- Realistic business scenarios
- Error handling and edge cases
- Comprehensive API endpoint coverage
- Analytics and reporting examples

## Tips for Using Demos

1. **Learning Tool**: Use these demos to understand API capabilities and best practices
2. **Testing**: Adapt demos for integration testing and API validation
3. **Documentation**: Reference demos when implementing similar workflows
4. **Customization**: Modify demos to match your specific use cases

## Common Patterns

All demos follow these patterns:
- Helper function for consistent API calls
- Clear step numbering and descriptions
- Response parsing with `jq`
- Summary of operations at completion
- Proper error handling with `set -e`

## Extending the Demos

To create a new demo:
1. Copy an existing demo as a template
2. Update the workflow steps for your use case
3. Ensure proper error handling
4. Add clear documentation
5. Test thoroughly before committing

## Support

For questions or issues with these demos, please refer to the main API documentation or create an issue in the repository. 