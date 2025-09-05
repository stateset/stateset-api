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
        (name = "Analytics", description = "Business intelligence endpoints"),
        (name = "Health", description = "Health check endpoints")
    ),
    paths(
        // Orders
        crate::handlers::orders::list_orders,
        crate::handlers::orders::get_order,
        crate::handlers::orders::create_order,
        crate::handlers::orders::update_order,
        crate::handlers::orders::update_order_status,
        crate::handlers::orders::cancel_order,
        crate::handlers::orders::archive_order,
        crate::handlers::orders::add_order_item,

        // Inventory
        crate::handlers::inventory::list_inventory,
        crate::handlers::inventory::get_inventory,
        crate::handlers::inventory::create_inventory,
        crate::handlers::inventory::update_inventory,
        crate::handlers::inventory::delete_inventory,
        crate::handlers::inventory::get_low_stock_items,
        crate::handlers::inventory::reserve_inventory,
        crate::handlers::inventory::release_inventory,

        // Analytics
        crate::handlers::analytics::get_dashboard_metrics,
        crate::handlers::analytics::get_sales_metrics,
        crate::handlers::analytics::get_sales_trends,
        crate::handlers::analytics::get_inventory_metrics,
        crate::handlers::analytics::get_shipment_metrics,

        // Health
        crate::handlers::health::health_check
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
            crate::handlers::inventory::InventoryResponse,
            crate::handlers::inventory::CreateInventoryRequest,
            crate::handlers::inventory::UpdateInventoryRequest,

            // Analytics types
            crate::services::analytics::DashboardMetrics,
            crate::services::analytics::SalesMetrics,
            crate::services::analytics::InventoryMetrics,
            crate::services::analytics::ShipmentMetrics,

            // Error types
            crate::errors::ErrorResponse
        ),
        security_schemes(
            ("bearer_auth", SecurityScheme::Http(
                Http::Bearer,
                Some(HttpAuthScheme::Bearer)
            )),
            ("api_key", SecurityScheme::ApiKey(
                ApiKey::Header(ApiKeyValue::new("X-API-Key"))
            ))
        )
    ),
    security(
        ("bearer_auth", []),
        ("api_key", [])
    )
)]
pub struct ApiDocV1;

pub fn swagger_ui() -> SwaggerUi {
    SwaggerUi::new("/swagger-ui")
        .url("/api-docs/openapi.json", ApiDocV1::openapi())
        .config(
            utoipa_swagger_ui::Config::from("/api-docs/openapi.json")
                .try_it_out_enabled(true)
                .request_interceptor(
                    r#"
                    function(req) {
                        // Add auth token if available
                        const token = localStorage.getItem('auth_token');
                        if (token) {
                            req.headers.Authorization = 'Bearer ' + token;
                        }
                        return req;
                    }
                    "#
                )
        )
}

#[cfg(test)]
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
