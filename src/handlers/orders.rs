use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Json, IntoResponse},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;
use validator::Validate;
use chrono::Utc;
use utoipa::ToSchema;

use crate::{
    errors::ServiceError,
    AppState, ApiResponse, ListQuery, PaginatedResponse,
    auth::AuthUser,
};

// Trait for order handler state - blanket implementation for all compatible types
pub trait OrderHandlerState: Clone + Send + Sync + 'static {}

// Blanket implementation for any type that satisfies the bounds
impl<T> OrderHandlerState for T where T: Clone + Send + Sync + 'static {}

// Order DTOs
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderResponse {
    pub id: String,
    pub customer_id: String,
    pub status: OrderStatus,
    pub total_amount: Option<rust_decimal::Decimal>,
    pub currency: Option<String>,
    pub items: Vec<OrderItem>,
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub payment_method_id: Option<String>,
    pub shipment_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderRequest {
    #[validate(length(min = 1, message = "Customer ID is required"))]
    pub customer_id: String,
    
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<CreateOrderItem>,
    
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub payment_method_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateOrderRequest {
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub payment_method_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateOrderItem {
    #[validate(length(min = 1, message = "Product ID is required"))]
    pub product_id: String,
    
    #[validate(range(min = 1, message = "Quantity must be at least 1"))]
    pub quantity: i32,
    
    pub unit_price: Option<rust_decimal::Decimal>,
    pub tax_rate: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OrderItem {
    pub id: String,
    pub product_id: String,
    pub product_name: String,
    pub quantity: i32,
    pub unit_price: rust_decimal::Decimal,
    pub total_price: rust_decimal::Decimal,
    pub tax_amount: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Address {
    pub street: String,
    pub city: String,
    pub state: String,
    pub postal_code: String,
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Processing,
    Shipped,
    Delivered,
    Cancelled,
    Refunded,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateOrderStatusRequest {
    pub status: OrderStatus,
    pub reason: Option<String>,
}

/// List orders with pagination and filtering
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    summary = "List orders",
    description = "Get a paginated list of orders with optional filtering",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 20)"),
        ("search" = Option<String>, Query, description = "Search term"),
        ("status" = Option<String>, Query, description = "Filter by order status"),
        ("customer_id" = Option<String>, Query, description = "Filter by customer ID"),
    ),
    responses(
        (status = 200, description = "Orders retrieved successfully", body = ApiResponse<PaginatedResponse<OrderResponse>>,
            headers(
                ("X-Request-Id" = String, description = "Unique request id"),
                ("X-RateLimit-Limit" = String, description = "Requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until reset"),
            )
        ),
        (status = 400, description = "Invalid request parameters", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn list_orders(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<PaginatedResponse<OrderResponse>>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:read") {
        return Err(ServiceError::Forbidden("Insufficient permissions to read orders".to_string()));
    }

    // For now, return mock data
    let mock_orders = create_mock_orders();
    
    // Apply pagination
    let total = mock_orders.len() as u64;
    let start = ((query.page - 1) * query.limit) as usize;
    let end = std::cmp::min(start + query.limit as usize, mock_orders.len());
    
    let items = if start < mock_orders.len() {
        mock_orders[start..end].to_vec()
    } else {
        vec![]
    };
    
    let total_pages = (total + query.limit - 1) / query.limit;
    
    let response = PaginatedResponse {
        items,
        total,
        page: query.page,
        limit: query.limit,
        total_pages,
    };
    
    Ok(Json(ApiResponse::success(response)))
}

/// Create a new order
#[utoipa::path(
    post,
    path = "/api/v1/orders",
    summary = "Create order",
    description = "Create a new order",
    request_body = CreateOrderRequest,
    responses(
        (status = 201, description = "Order created successfully", body = ApiResponse<OrderResponse>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request data", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn create_order(
    State(state): State<AppState>,
    auth_user: AuthUser,
    Json(request): Json<CreateOrderRequest>,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:create") {
        return Err(ServiceError::Forbidden("Insufficient permissions to create orders".to_string()));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!("{}: {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into()))
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    // Create the order
    let order_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now();
    
    // Convert items
    let items: Vec<OrderItem> = request.items.into_iter().enumerate().map(|(i, item)| {
        let unit_price = item.unit_price.unwrap_or_else(|| rust_decimal::Decimal::new(1999, 2)); // $19.99 default
        let total_price = unit_price * rust_decimal::Decimal::from(item.quantity);
        
        OrderItem {
            id: format!("item_{}", i + 1),
            product_id: item.product_id,
            product_name: "Sample Product".to_string(), // Would be fetched from product service
            quantity: item.quantity,
            unit_price,
            total_price,
            tax_amount: item.tax_rate.map(|rate| total_price * rate),
        }
    }).collect();
    
    let total_amount = items.iter().map(|item| item.total_price).sum();
    
    let order = OrderResponse {
        id: order_id,
        customer_id: request.customer_id,
        status: OrderStatus::Pending,
        total_amount: Some(total_amount),
        currency: Some("USD".to_string()),
        items,
        shipping_address: request.shipping_address,
        billing_address: request.billing_address,
        payment_method_id: request.payment_method_id,
        shipment_id: None,
        created_at: now,
        updated_at: now,
    };

    // TODO: Save to database
    // let saved_order = orders_service.create(order).await?;

    Ok(Json(ApiResponse::success(order)))
}

/// Get order by ID
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}",
    summary = "Get order",
    description = "Get an order by its ID",
    params(
        ("id" = String, Path, description = "Order ID"),
    ),
    responses(
        (status = 200, description = "Order retrieved successfully", body = ApiResponse<OrderResponse>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn get_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:read") {
        return Err(ServiceError::Forbidden("Insufficient permissions to read orders".to_string()));
    }

    // For now, return mock data
    if id == "order_123" {
        let order = create_mock_order();
        Ok(Json(ApiResponse::success(order)))
    } else {
        Err(ServiceError::NotFound(format!("Order with ID {} not found", id)))
    }
}

/// Update order
#[utoipa::path(
    put,
    path = "/api/v1/orders/{id}",
    summary = "Update order",
    description = "Update an existing order",
    params(
        ("id" = String, Path, description = "Order ID"),
    ),
    request_body = UpdateOrderRequest,
    responses(
        (status = 200, description = "Order updated successfully", body = ApiResponse<OrderResponse>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn update_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
    Json(request): Json<UpdateOrderRequest>,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:update") {
        return Err(ServiceError::Forbidden("Insufficient permissions to update orders".to_string()));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!("{}: {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into()))
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    // For now, return mock updated order
    let mut order = create_mock_order();
    order.id = id.clone();
    order.updated_at = chrono::Utc::now();
    
    if let Some(shipping) = request.shipping_address {
        order.shipping_address = Some(shipping);
    }
    if let Some(billing) = request.billing_address {
        order.billing_address = Some(billing);
    }
    if let Some(payment_method) = request.payment_method_id {
        order.payment_method_id = Some(payment_method);
    }

    Ok(Json(ApiResponse::success(order)))
}

/// Delete order
#[utoipa::path(
    delete,
    path = "/api/v1/orders/{id}",
    summary = "Delete order",
    description = "Delete an order by its ID",
    params(
        ("id" = String, Path, description = "Order ID"),
    ),
    responses(
        (status = 200, description = "Order deleted successfully",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn delete_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<serde_json::Value>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:delete") {
        return Err(ServiceError::Forbidden("Insufficient permissions to delete orders".to_string()));
    }

    // TODO: Delete from database
    // orders_service.delete(id).await?;

    let response = serde_json::json!({
        "message": format!("Order {} deleted successfully", id)
    });

    Ok(Json(ApiResponse::success(response)))
}

/// Update order status
#[utoipa::path(
    put,
    path = "/api/v1/orders/{id}/status",
    summary = "Update order status",
    description = "Update the status of an order",
    params(
        ("id" = String, Path, description = "Order ID"),
    ),
    request_body = UpdateOrderStatusRequest,
    responses(
        (status = 200, description = "Order status updated successfully", body = ApiResponse<OrderResponse>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn update_order_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
    Json(request): Json<UpdateOrderStatusRequest>,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:update") {
        return Err(ServiceError::Forbidden("Insufficient permissions to update orders".to_string()));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!("{}: {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into()))
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    // For now, return mock updated order
    let mut order = create_mock_order();
    order.id = id;
    order.status = request.status;
    order.updated_at = chrono::Utc::now();

    Ok(Json(ApiResponse::success(order)))
}

/// Get order items
#[utoipa::path(
    get,
    path = "/api/v1/orders/{id}/items",
    summary = "Get order items",
    description = "Get all items for a specific order",
    params(
        ("id" = String, Path, description = "Order ID"),
    ),
    responses(
        (status = 200, description = "Order items retrieved successfully", body = ApiResponse<Vec<OrderItem>>),
        (status = 404, description = "Order not found"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn get_order_items(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<Vec<OrderItem>>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:read") {
        return Err(ServiceError::Forbidden("Insufficient permissions to read orders".to_string()));
    }

    // For now, return mock items
    let items = vec![
        OrderItem {
            id: "item_1".to_string(),
            product_id: "prod_123".to_string(),
            product_name: "Sample Product 1".to_string(),
            quantity: 2,
            unit_price: rust_decimal::Decimal::new(1999, 2),
            total_price: rust_decimal::Decimal::new(3998, 2),
            tax_amount: Some(rust_decimal::Decimal::new(320, 2)),
        },
        OrderItem {
            id: "item_2".to_string(),
            product_id: "prod_456".to_string(),
            product_name: "Sample Product 2".to_string(),
            quantity: 1,
            unit_price: rust_decimal::Decimal::new(2999, 2),
            total_price: rust_decimal::Decimal::new(2999, 2),
            tax_amount: Some(rust_decimal::Decimal::new(240, 2)),
        },
    ];

    Ok(Json(ApiResponse::success(items)))
}

/// Add item to order
#[utoipa::path(
    post,
    path = "/api/v1/orders/{id}/items",
    summary = "Add order item",
    description = "Add a new item to an existing order",
    params(
        ("id" = String, Path, description = "Order ID"),
    ),
    request_body = CreateOrderItem,
    responses(
        (status = 201, description = "Item added successfully", body = ApiResponse<OrderItem>,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse),
    ),
    security(
        ("Bearer" = []),
        ("ApiKey" = [])
    )
)]
pub async fn add_order_item(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
    Json(request): Json<CreateOrderItem>,
) -> Result<Json<ApiResponse<OrderItem>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission("orders:update") {
        return Err(ServiceError::Forbidden("Insufficient permissions to update orders".to_string()));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!("{}: {}", field, error.message.as_ref().unwrap_or(&"Invalid value".into()))
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    // Create the new item
    let unit_price = request.unit_price.unwrap_or_else(|| rust_decimal::Decimal::new(1999, 2));
    let total_price = unit_price * rust_decimal::Decimal::from(request.quantity);
    
    let item = OrderItem {
        id: Uuid::new_v4().to_string(),
        product_id: request.product_id,
        product_name: "Sample Product".to_string(),
        quantity: request.quantity,
        unit_price,
        total_price,
        tax_amount: request.tax_rate.map(|rate| total_price * rate),
    };

    Ok(Json(ApiResponse::success(item)))
}

// Helper functions for mock data
fn create_mock_order() -> OrderResponse {
    let now = chrono::Utc::now();
    OrderResponse {
        id: "order_123".to_string(),
        customer_id: "customer_456".to_string(),
        status: OrderStatus::Processing,
        total_amount: Some(rust_decimal::Decimal::new(6997, 2)), // $69.97
        currency: Some("USD".to_string()),
        items: vec![
            OrderItem {
                id: "item_1".to_string(),
                product_id: "prod_123".to_string(),
                product_name: "Sample Product 1".to_string(),
                quantity: 2,
                unit_price: rust_decimal::Decimal::new(1999, 2),
                total_price: rust_decimal::Decimal::new(3998, 2),
                tax_amount: Some(rust_decimal::Decimal::new(320, 2)),
            },
            OrderItem {
                id: "item_2".to_string(),
                product_id: "prod_456".to_string(),
                product_name: "Sample Product 2".to_string(),
                quantity: 1,
                unit_price: rust_decimal::Decimal::new(2999, 2),
                total_price: rust_decimal::Decimal::new(2999, 2),
                tax_amount: Some(rust_decimal::Decimal::new(240, 2)),
            },
        ],
        shipping_address: Some(Address {
            street: "123 Main St".to_string(),
            city: "Anytown".to_string(),
            state: "CA".to_string(),
            postal_code: "12345".to_string(),
            country: "US".to_string(),
        }),
        billing_address: None,
        payment_method_id: Some("pm_123".to_string()),
        shipment_id: Some("ship_456".to_string()),
        created_at: now - chrono::Duration::hours(2),
        updated_at: now,
    }
}

fn create_mock_orders() -> Vec<OrderResponse> {
    let now = chrono::Utc::now();
    vec![
        create_mock_order(),
        OrderResponse {
            id: "order_789".to_string(),
            customer_id: "customer_101".to_string(),
            status: OrderStatus::Shipped,
            total_amount: Some(rust_decimal::Decimal::new(4499, 2)),
            currency: Some("USD".to_string()),
            items: vec![
                OrderItem {
                    id: "item_3".to_string(),
                    product_id: "prod_789".to_string(),
                    product_name: "Sample Product 3".to_string(),
                    quantity: 1,
                    unit_price: rust_decimal::Decimal::new(4499, 2),
                    total_price: rust_decimal::Decimal::new(4499, 2),
                    tax_amount: Some(rust_decimal::Decimal::new(360, 2)),
                },
            ],
            shipping_address: Some(Address {
                street: "456 Oak Ave".to_string(),
                city: "Springfield".to_string(),
                state: "NY".to_string(),
                postal_code: "67890".to_string(),
                country: "US".to_string(),
            }),
            billing_address: None,
            payment_method_id: Some("pm_789".to_string()),
            shipment_id: Some("ship_101".to_string()),
            created_at: now - chrono::Duration::days(1),
            updated_at: now - chrono::Duration::hours(6),
        },
    ]
}

/// Cancel an existing order
pub async fn cancel_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: Clone + Send + Sync + 'static,
{
    let cancellation_reason = payload.get("reason").and_then(|r| r.as_str()).unwrap_or("Customer request");
    
    let response = json!({
        "message": format!("Order {} has been cancelled", id),
        "order_id": id,
        "status": "cancelled",
        "cancellation_reason": cancellation_reason,
        "cancelled_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Archive an existing order
pub async fn archive_order<S>(
    State(_state): State<S>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ServiceError>
where
    S: Clone + Send + Sync + 'static,
{
    let response = json!({
        "message": format!("Order {} has been archived", id),
        "order_id": id,
        "status": "archived",
        "archived_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}
