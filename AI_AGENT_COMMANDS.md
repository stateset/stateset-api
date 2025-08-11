# StateSet API Commands Reference for AI Agents

This document provides a comprehensive list of all available API endpoints and commands that an AI agent can use to interact with the StateSet API.

## Authentication

### Public Endpoints (No Auth Required)
- `GET /health` - System health check
- `GET /api/info` - API information and capabilities
- `POST /auth/login` - User authentication (returns JWT token)
- `GET /docs` - Swagger UI documentation

### Authentication Headers
```
Authorization: Bearer <JWT_TOKEN>
X-API-Key: <API_KEY>
```

## API Endpoints by Domain

### Orders Management
- `GET /api/v1/orders` - List orders (filters: status, page, limit)
- `POST /api/v1/orders` - Create new order
- `GET /api/v1/orders/{id}` - Get order details
- `PUT /api/v1/orders/{id}` - Update order
- `DELETE /api/v1/orders/{id}` - Delete order
- `PUT /api/v1/orders/{id}/items` - Update order items
- `POST /api/v1/orders/{id}/cancel` - Cancel order
- `POST /api/v1/orders/{id}/hold` - Place order on hold
- `POST /api/v1/orders/{id}/release` - Release order from hold
- `POST /api/v1/orders/{id}/ship` - Ship order
- `POST /api/v1/orders/{id}/deliver` - Mark as delivered
- `POST /api/v1/orders/{id}/refund` - Refund order
- `POST /api/v1/orders/{id}/split` - Split order
- `POST /api/v1/orders/{id}/merge` - Merge orders
- `POST /api/v1/orders/{id}/archive` - Archive order

### Inventory Management
- `GET /api/v1/inventory` - List inventory
- `GET /api/v1/inventory/{id}` - Get inventory item
- `POST /api/v1/inventory/adjust` - Adjust inventory quantity
- `POST /api/v1/inventory/allocate` - Allocate inventory
- `POST /api/v1/inventory/deallocate` - Deallocate inventory
- `POST /api/v1/inventory/reserve` - Reserve inventory
- `POST /api/v1/inventory/reserve/v2` - Enhanced reservation
- `POST /api/v1/inventory/release` - Release reserved inventory
- `POST /api/v1/inventory/levels` - Set inventory levels
- `GET /api/v1/inventory/levels/{product_id}/{location_id}` - Get levels
- `GET /api/v1/inventory/{product_id}/{location_id}` - Get inventory
- `GET /api/v1/inventory/{product_id}/{location_id}/available` - Check availability
- `POST /api/v1/inventory/transfer` - Transfer inventory
- `POST /api/v1/inventory/receive` - Receive inventory
- `POST /api/v1/inventory/cycle-count` - Cycle count

### Returns Processing
- `GET /api/v1/returns` - List returns
- `GET /api/v1/returns/{id}` - Get return details
- `POST /api/v1/returns` - Create return
- `POST /api/v1/returns/{id}/approve` - Approve return
- `POST /api/v1/returns/{id}/reject` - Reject return
- `POST /api/v1/returns/{id}/cancel` - Cancel return
- `POST /api/v1/returns/{id}/complete` - Complete return
- `POST /api/v1/returns/{id}/refund` - Process refund
- `POST /api/v1/returns/{id}/restock` - Restock items

### Shipments & Logistics
- `GET /api/v1/shipments` - List shipments
- `GET /api/v1/shipments/{id}` - Get shipment details
- `POST /api/v1/shipments` - Create shipment
- `PUT /api/v1/shipments/{id}` - Update shipment
- `POST /api/v1/shipments/{id}/cancel` - Cancel shipment
- `GET /api/v1/shipments/{id}/track` - Track shipment
- `POST /api/v1/shipments/{id}/confirm-delivery` - Confirm delivery
- `POST /api/v1/shipments/{id}/assign-carrier` - Assign carrier
- `POST /api/v1/shipments/{id}/ship` - Ship items
- `GET /api/v1/shipments/order/{order_id}` - Get shipments for order

### Work Orders
- `GET /api/v1/work-orders` - List work orders
- `POST /api/v1/work-orders` - Create work order
- `GET /api/v1/work-orders/{id}` - Get work order
- `PUT /api/v1/work-orders/{id}` - Update work order
- `POST /api/v1/work-orders/{id}/cancel` - Cancel work order
- `POST /api/v1/work-orders/{id}/start` - Start work order
- `POST /api/v1/work-orders/{id}/complete` - Complete work order
- `POST /api/v1/work-orders/{id}/assign` - Assign work order
- `POST /api/v1/work-orders/{id}/unassign` - Unassign work order
- `POST /api/v1/work-orders/{id}/schedule` - Schedule work order
- `GET /api/v1/work-orders/assignee/{user_id}` - Get by assignee
- `GET /api/v1/work-orders/status/{status}` - Get by status
- `GET /api/v1/work-orders/schedule` - Get scheduled work orders

### Warranties
- `POST /api/v1/warranties` - Create warranty
- `GET /api/v1/warranties/{id}` - Get warranty
- `POST /api/v1/warranties/{id}/claim` - Submit warranty claim
- `POST /api/v1/warranties/claims/{id}/approve` - Approve claim
- `POST /api/v1/warranties/claims/{id}/reject` - Reject claim
- `GET /api/v1/warranties/product/{product_id}` - Get warranties for product

### Bill of Materials (BOM)
- `POST /api/v1/bom` - Create BOM
- `GET /api/v1/bom/{id}` - Get BOM
- `PUT /api/v1/bom/{id}` - Update BOM
- `DELETE /api/v1/bom/{id}` - Delete BOM
- `POST /api/v1/bom/{id}/components` - Add component
- `DELETE /api/v1/bom/{id}/components/{component_id}` - Remove component
- `GET /api/v1/bom/product/{product_id}` - Get BOMs for product

### Commerce (E-commerce Features)
- `GET /api/v1/products` - List products
- `POST /api/v1/products` - Create product
- `GET /api/v1/products/{id}` - Get product
- `GET /api/v1/products/{id}/variants` - Get product variants
- `POST /api/v1/products/{id}/variants` - Create variant
- `PUT /api/v1/products/variants/{variant_id}/price` - Update price
- `GET /api/v1/products/search` - Search products

- `POST /api/v1/carts` - Create cart
- `GET /api/v1/carts/{id}` - Get cart
- `POST /api/v1/carts/{id}/items` - Add to cart
- `PUT /api/v1/carts/{id}/items/{item_id}` - Update cart item
- `DELETE /api/v1/carts/{id}/items/{item_id}` - Remove from cart
- `POST /api/v1/carts/{id}/clear` - Clear cart

- `POST /api/v1/checkout` - Process checkout
- `GET /api/v1/customers` - List customers
- `POST /api/v1/customers` - Create customer

### Cash Sales
- `POST /api/v1/cash-sales` - Create cash sale

### Analytics & Reporting
- `GET /api/v1/analytics` - Analytics dashboard

### Real-time Features
- `WS /ws` - WebSocket connection for live updates

## Command Categories

### Order Commands
- CreateOrderCommand
- UpdateOrderStatusCommand
- UpdateOrderItemsCommand
- CancelOrderCommand
- HoldOrderCommand
- ReleaseOrderFromHoldCommand
- ShipOrderCommand
- DeliverOrderCommand
- RefundOrderCommand
- SplitOrderCommand
- MergeOrdersCommand
- ArchiveOrderCommand
- AddItemToOrderCommand
- RemoveItemFromOrderCommand
- ApplyOrderDiscountCommand
- AddOrderNoteCommand
- UpdateBillingAddressCommand
- UpdateShippingAddressCommand
- TagOrderCommand

### Inventory Commands
- AdjustInventoryCommand
- AllocateInventoryCommand
- DeallocateInventoryCommand
- ReserveInventoryCommand
- ReleaseInventoryCommand
- SetInventoryLevelsCommand
- TransferInventoryCommand
- ReceiveInventoryCommand
- CycleCountCommand
- AddLotCommand
- UpdateLotCommand
- MarkLotExpiredCommand
- QuarantineInventoryCommand
- ReleaseFromQuarantineCommand

### Return Commands
- InitiateReturnCommand
- ApproveReturnCommand
- RejectReturnCommand
- CancelReturnCommand
- CompleteReturnCommand
- RefundReturnCommand
- RestockReturnedItemsCommand
- InspectReturnCommand
- ReceiveReturnCommand
- GenerateShippingLabelCommand

### Shipment Commands
- CreateShipmentCommand
- UpdateShipmentCommand
- CancelShipmentCommand
- TrackShipmentCommand
- ConfirmShipmentDeliveryCommand
- AssignShipmentCarrierCommand
- ShipOrderCommand
- HoldShipmentCommand
- ReleaseHoldShipmentCommand
- RescheduleShipmentCommand

### Work Order Commands
- CreateWorkOrderCommand
- UpdateWorkOrderCommand
- CancelWorkOrderCommand
- StartWorkOrderCommand
- CompleteWorkOrderCommand
- AssignWorkOrderCommand
- UnassignWorkOrderCommand
- ScheduleWorkOrderCommand
- PickWorkOrderCommand
- IssueWorkOrderCommand

### Customer Commands
- CreateCustomerCommand
- UpdateCustomerCommand
- DeleteCustomerCommand
- ActivateCustomerCommand
- DeactivateCustomerCommand
- SuspendCustomerCommand
- FlagCustomerCommand
- MergeCustomersCommand
- SearchCustomersCommand

### Analytics Commands
- GenerateSalesReportCommand
- GenerateInventoryTurnoverCommand
- GenerateCustomerActivityReportCommand

## Query Parameters

### Pagination
- `page` - Page number (default: 1)
- `limit` - Items per page (default: 20, max: 100)

### Filtering
- `status` - Filter by status
- `created_after` - Filter by creation date
- `created_before` - Filter by creation date
- `customer_id` - Filter by customer
- `product_id` - Filter by product

### Sorting
- `sort` - Sort field
- `order` - Sort order (asc/desc)

## Response Formats

### Success Response
```json
{
  "data": {...},
  "message": "Success",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### Error Response
```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Error description",
    "details": {...}
  },
  "timestamp": "2024-01-01T00:00:00Z"
}
```

### Pagination Response
```json
{
  "data": [...],
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 100,
    "pages": 5
  }
}
```

## Rate Limiting

- General endpoints: 100 requests/minute
- Authentication endpoints: 10 requests/minute
- Write operations: 50 requests/minute
- Headers: `X-RateLimit-Limit`, `X-RateLimit-Remaining`, `X-RateLimit-Reset`

## Permissions

### Admin Role
- Full access to all endpoints

### Manager Role
- All read operations
- Create/update orders, inventory, returns
- Cannot delete resources

### Operator Role
- Read orders, inventory, shipments
- Update order status
- Process returns

### Viewer Role
- Read-only access to all resources

## WebSocket Events

### Channels
- `orders` - Order events
- `inventory` - Inventory updates
- `shipments` - Shipment tracking
- `analytics` - Real-time metrics

### Event Types
- `created`
- `updated`
- `deleted`
- `status_changed`

## Usage Examples

### Create Order
```bash
curl -X POST http://localhost:8080/api/v1/orders \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "123e4567-e89b-12d3-a456-426614174000",
    "items": [
      {"product_id": "123e4567-e89b-12d3-a456-426614174001", "quantity": 2}
    ]
  }'
```

### Reserve Inventory
```bash
curl -X POST http://localhost:8080/api/v1/inventory/reserve \
  -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  -d '{
    "warehouse_id": "WH001",
    "reference_id": "123e4567-e89b-12d3-a456-426614174000",
    "reference_type": "SALES_ORDER",
    "items": [
      {"product_id": "123e4567-e89b-12d3-a456-426614174001", "quantity": 10}
    ]
  }'
```

### Track Shipment
```bash
curl -X GET http://localhost:8080/api/v1/shipments/123/track \
  -H "Authorization: Bearer <token>"
```

## Notes for AI Agents

1. Always include proper authentication headers
2. Use pagination for list endpoints to avoid large responses
3. Check rate limit headers to avoid hitting limits
4. Handle both success and error responses appropriately
5. Use WebSocket for real-time updates when needed
6. Validate input data before sending requests
7. Use appropriate HTTP methods (GET for read, POST for create, etc.)
8. Include proper Content-Type headers for requests with body 