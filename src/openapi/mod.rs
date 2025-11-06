use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "StateSet API",
        version = "1.0.0",
        description = r#"
# StateSet Supply Chain Management API

A comprehensive API for managing orders, inventory, shipments, returns, warranties, and work orders in a modern supply chain system.

## Features

- **Order Management**: Create, update, and track customer orders
- **Inventory Management**: Real-time inventory tracking and adjustments
- **Shipment Tracking**: End-to-end shipment lifecycle management
- **Return Processing**: Streamlined return and refund workflows
- **Warranty Management**: Warranty claim processing and tracking
- **Work Order Management**: Manufacturing and maintenance work orders
- **Analytics**: Business intelligence and reporting
- **Multi-warehouse Support**: Distributed inventory management
- **Real-time Events**: Event-driven architecture for integrations

## Authentication

All API endpoints require authentication using JWT tokens or API keys. Include the token in the Authorization header:

```
Authorization: Bearer <your-jwt-token>
```

## Rate Limiting

API requests are rate-limited. Check the response headers for rate limit information:
- `X-RateLimit-Limit`: Maximum requests per window
- `X-RateLimit-Remaining`: Remaining requests in current window
- `X-RateLimit-Reset`: Time when the rate limit resets

## Error Handling

The API uses consistent error response formats with appropriate HTTP status codes:

```json
{
  "success": false,
  "error": "Bad Request",
  "message": "Validation failed",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

## Pagination

List endpoints support pagination with the following query parameters:
- `page`: Page number (default: 1)
- `limit`: Items per page (default: 20, max: 100)
- `search`: Search term for filtering results
- `sort_by`: Field to sort by
- `sort_order`: Sort order (asc/desc)
        "#,
        contact(
            name = "StateSet Support",
            email = "support@stateset.io",
            url = "https://stateset.io"
        ),
        license(
            name = "MIT",
            url = "https://opensource.org/licenses/MIT"
        )
    ),
    servers(
        (url = "https://api.stateset.io/v1", description = "Production server"),
        (url = "https://staging-api.stateset.io/v1", description = "Staging server"),
        (url = "http://localhost:8080/api/v1", description = "Local development")
    ),
    tags(
        (name = "Orders", description = "Order management endpoints"),
        (name = "Inventory", description = "Inventory management endpoints"),
        (name = "Shipments", description = "Shipment tracking endpoints"),
        (name = "Returns", description = "Return processing endpoints"),
        (name = "Warranties", description = "Warranty management endpoints"),
        (name = "Work Orders", description = "Work order management endpoints"),
        (name = "Payments", description = "Payment processing endpoints"),
        (name = "Analytics", description = "Business intelligence endpoints"),
        (name = "Health", description = "Health check endpoints"),
        (name = "Admin", description = "Administrative endpoints")
    ),
    paths(
        // Orders
        crate::handlers::orders::list_orders,
        crate::handlers::orders::get_order_by_number,
        crate::handlers::orders::get_order,
        crate::handlers::orders::create_order,
        crate::handlers::orders::update_order,
        crate::handlers::orders::update_order_status,
        crate::handlers::orders::get_order_items,
        crate::handlers::orders::add_order_item,
        crate::handlers::orders::delete_order,
        crate::handlers::orders::cancel_order,
        crate::handlers::orders::archive_order,

        // Inventory
        crate::handlers::inventory::list_inventory,
        crate::handlers::inventory::get_inventory,
        crate::handlers::inventory::create_inventory,
        crate::handlers::inventory::update_inventory,
        crate::handlers::inventory::delete_inventory,
        crate::handlers::inventory::reserve_inventory::<crate::AppState>,
        crate::handlers::inventory::release_inventory::<crate::AppState>,
        crate::handlers::inventory::get_low_stock_items,

        // Returns
        crate::handlers::returns::list_returns,
        crate::handlers::returns::get_return,
        crate::handlers::returns::create_return,
        crate::handlers::returns::approve_return,
        crate::handlers::returns::restock_return,

        // Shipments
        crate::handlers::shipments::list_shipments,
        crate::handlers::shipments::get_shipment,
        crate::handlers::shipments::create_shipment,
        crate::handlers::shipments::mark_shipped,
        crate::handlers::shipments::mark_delivered,
        crate::handlers::shipments::track_shipment,
        crate::handlers::shipments::track_by_number,

        // Warranties
        crate::handlers::warranties::list_warranties,
        crate::handlers::warranties::get_warranty,
        crate::handlers::warranties::create_warranty,
        crate::handlers::warranties::create_warranty_claim,
        crate::handlers::warranties::approve_warranty_claim,
        crate::handlers::warranties::extend_warranty,

        // Payments
        crate::handlers::payments::process_payment,
        crate::handlers::payments::get_payment,
        crate::handlers::payments::get_order_payments,
        crate::handlers::payments::list_payments,
        crate::handlers::payments::refund_payment,
        crate::handlers::payments::get_order_payment_total,

        // Work Orders
        crate::handlers::work_orders::list_work_orders::<crate::AppState>,
        crate::handlers::work_orders::create_work_order::<crate::AppState>,
        crate::handlers::work_orders::get_work_order::<crate::AppState>,
        crate::handlers::work_orders::update_work_order::<crate::AppState>,
        crate::handlers::work_orders::delete_work_order::<crate::AppState>,
        crate::handlers::work_orders::assign_work_order::<crate::AppState>,
        crate::handlers::work_orders::complete_work_order::<crate::AppState>,
        crate::handlers::work_orders::update_work_order_status::<crate::AppState>,

        // Admin Outbox
        crate::handlers::outbox_admin::list_outbox,
        crate::handlers::outbox_admin::retry_outbox,

        // Webhooks
        crate::handlers::payment_webhooks::payment_webhook,

        // Analytics
        crate::handlers::analytics::get_dashboard_metrics,
        crate::handlers::analytics::get_sales_metrics,
        crate::handlers::analytics::get_sales_trends,
        crate::handlers::analytics::get_inventory_metrics,
        crate::handlers::analytics::get_shipment_metrics,

        // Manufacturing BOM
        crate::handlers::bom::create_bom,
        crate::handlers::bom::get_bom,
        crate::handlers::bom::update_bom,
        crate::handlers::bom::audit_bom,
        crate::handlers::bom::list_boms,
        crate::handlers::bom::get_bom_components,
        crate::handlers::bom::add_component_to_bom,
        crate::handlers::bom::remove_component_from_bom,

        // Procurement
        crate::handlers::purchase_orders::create_purchase_order,
        crate::handlers::purchase_orders::get_purchase_order,
        crate::handlers::purchase_orders::update_purchase_order,
        crate::handlers::purchase_orders::approve_purchase_order,
        crate::handlers::purchase_orders::cancel_purchase_order,
        crate::handlers::purchase_orders::receive_purchase_order,
        crate::handlers::purchase_orders::get_purchase_orders_by_supplier,
        crate::handlers::purchase_orders::get_purchase_orders_by_status,
        crate::handlers::purchase_orders::get_purchase_orders_by_delivery_date,
        crate::handlers::purchase_orders::get_total_purchase_value,

        // ASNs
        crate::handlers::asn::create_asn,
        crate::handlers::asn::list_asns,
        crate::handlers::asn::get_asn,
        crate::handlers::asn::mark_in_transit,
        crate::handlers::asn::mark_delivered,

        // Health intentionally omitted from OpenAPI paths for now
    ),
    components(
        schemas(
            // Common types
            crate::ApiResponse<serde_json::Value>,
            crate::PaginatedResponse<serde_json::Value>,
            crate::ListQuery,

            // Order types
            crate::handlers::orders::OrderResponse,
            crate::handlers::orders::CreateOrderRequest,
            crate::handlers::orders::UpdateOrderRequest,
            crate::handlers::orders::OrderStatus,
            crate::handlers::orders::OrderItem,
            crate::handlers::orders::Address,

            // Inventory types
            crate::handlers::inventory::InventoryItem,
            crate::handlers::inventory::CreateInventoryRequest,
            crate::handlers::inventory::UpdateInventoryRequest,

            // Payments types
            crate::handlers::payments::CreatePaymentRequest,
            crate::handlers::payments::RefundPaymentHandlerRequest,
            crate::services::payments::PaymentResponse,
            crate::handlers::payments::PaymentStatusFilter,

            // Analytics types
            crate::services::analytics::DashboardMetrics,
            crate::services::analytics::SalesMetrics,
            crate::services::analytics::InventoryMetrics,
            crate::services::analytics::ShipmentMetrics,
            crate::services::analytics::SalesTrendPoint,

            // Error types
            crate::errors::ErrorResponse
        )
    )
)]
pub struct ApiDocV1;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDocV1::openapi())
        .config(utoipa_swagger_ui::Config::from("/api-docs/openapi.json").try_it_out_enabled(true))
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_generation() {
        let openapi = ApiDocV1::openapi();
        let json = serde_json::to_string_pretty(&openapi).unwrap();
        assert!(json.contains("StateSet API"));
        assert!(json.contains("/api/v1/orders"));
        assert!(json.contains("bearer_auth"));
    }
}
