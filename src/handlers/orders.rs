use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::auth::consts as perm;
use crate::entities::commerce::product_variant;
use crate::entities::order_item;
use crate::{
    auth::AuthUser, errors::ServiceError, ApiResponse, AppState, ListQuery, PaginatedResponse,
};
// Commands are not directly used by handlers at this time
use crate::services::commerce::product_catalog_service::ProductCatalogService;
use crate::services::orders::{self as svc_orders, UpdateOrderDetails};

fn map_status_str(status: &str) -> Result<OrderStatus, ServiceError> {
    match status.to_ascii_lowercase().as_str() {
        "pending" => Ok(OrderStatus::Pending),
        "processing" => Ok(OrderStatus::Processing),
        "shipped" => Ok(OrderStatus::Shipped),
        "delivered" => Ok(OrderStatus::Delivered),
        "cancelled" | "canceled" => Ok(OrderStatus::Cancelled),
        "refunded" => Ok(OrderStatus::Refunded),
        "confirmed" => Ok(OrderStatus::Confirmed),
        other => Err(ServiceError::InvalidStatus(format!(
            "Unknown order status: {other}"
        ))),
    }
}

// Resolve an order identifier that may be a UUID or an order_number string
async fn resolve_order_id(state: &AppState, id: &str) -> Result<Uuid, ServiceError> {
    if let Ok(uuid) = Uuid::parse_str(id) {
        return Ok(uuid);
    }
    if let Some(uuid) = state
        .services
        .order
        .find_order_id_by_order_number(id)
        .await?
    {
        return Ok(uuid);
    }
    Err(ServiceError::NotFound(format!(
        "Order with ID {} not found",
        id
    )))
}

async fn resolve_variant_identifier(
    catalog: &ProductCatalogService,
    identifier: &str,
    context: &str,
) -> Result<product_variant::Model, ServiceError> {
    if let Ok(uuid) = Uuid::parse_str(identifier) {
        match catalog.get_variant(uuid).await {
            Ok(variant) => Ok(variant),
            Err(ServiceError::NotFound(_)) => Err(ServiceError::ValidationError(format!(
                "{context} references an unknown product variant ({identifier})"
            ))),
            Err(err) => Err(err),
        }
    } else {
        match catalog.get_variant_by_sku(identifier).await {
            Ok(variant) => Ok(variant),
            Err(ServiceError::NotFound(_)) => Err(ServiceError::ValidationError(format!(
                "{context} references an unknown SKU ({identifier})"
            ))),
            Err(err) => Err(err),
        }
    }
}

fn map_service_order(
    order: &svc_orders::OrderResponse,
    items: Option<&[order_item::Model]>,
) -> Result<OrderResponse, ServiceError> {
    let status = map_status_str(&order.status)?;
    let shipping_address = parse_order_address(order.shipping_address.as_deref());
    let billing_address = parse_order_address(order.billing_address.as_deref());
    let mapped_items = items
        .map(|models| models.iter().map(map_order_item_model).collect())
        .unwrap_or_else(Vec::new);

    Ok(OrderResponse {
        id: order.id.to_string(),
        customer_id: order.customer_id.to_string(),
        status,
        total_amount: Some(order.total_amount),
        currency: Some(order.currency.clone()),
        items: mapped_items,
        shipping_address,
        billing_address,
        payment_method_id: order.payment_method.clone(),
        shipment_id: order.tracking_number.clone(),
        created_at: order.created_at,
        updated_at: order.updated_at.unwrap_or(order.created_at),
    })
}

fn parse_order_address(raw: Option<&str>) -> Option<Address> {
    let raw = raw?.trim();
    if raw.is_empty() {
        return None;
    }

    let mut segments = raw.splitn(4, ',').map(|segment| segment.trim());
    let street = segments.next()?.to_string();
    let city = segments.next()?.to_string();
    let state = segments.next()?.to_string();
    let country_postal = segments.next()?.trim();

    if country_postal.is_empty() {
        return None;
    }

    let mut cp_iter = country_postal.split_whitespace();
    let country = cp_iter.next()?.to_string();
    let postal_code = cp_iter.collect::<Vec<_>>().join(" ");

    Some(Address {
        street,
        city,
        state,
        postal_code,
        country,
    })
}

fn format_order_address(address: &Address) -> String {
    format!(
        "{}, {}, {}, {} {}",
        address.street.trim(),
        address.city.trim(),
        address.state.trim(),
        address.country.trim(),
        address.postal_code.trim()
    )
}

fn map_order_item_model(model: &order_item::Model) -> OrderItem {
    let sku = if model.sku.is_empty() {
        None
    } else {
        Some(model.sku.clone())
    };
    let product_name = if model.name.is_empty() {
        model.product_id.to_string()
    } else {
        model.name.clone()
    };

    OrderItem {
        id: model.id.to_string(),
        product_id: model.product_id.to_string(),
        product_name,
        sku,
        quantity: model.quantity,
        unit_price: model.unit_price,
        total_price: model.total_price,
        tax_amount: Some(model.tax_amount),
    }
}

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
#[serde(deny_unknown_fields)]
pub struct CreateOrderRequest {
    #[validate(length(min = 1))]
    pub customer_id: String,

    #[validate(length(min = 1))]
    pub items: Vec<CreateOrderItem>,

    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub payment_method_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateOrderRequest {
    pub shipping_address: Option<Address>,
    pub billing_address: Option<Address>,
    pub payment_method_id: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateOrderItem {
    /// Variant identifier; accepts either a UUID or SKU string.
    #[serde(alias = "sku")]
    #[validate(length(min = 1))]
    pub product_id: String,

    #[validate(range(min = 1))]
    pub quantity: i32,

    #[serde(alias = "price")]
    pub unit_price: Option<rust_decimal::Decimal>,
    pub tax_rate: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OrderItem {
    pub id: String,
    pub product_id: String,
    pub product_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sku: Option<String>,
    pub quantity: i32,
    pub unit_price: rust_decimal::Decimal,
    pub total_price: rust_decimal::Decimal,
    pub tax_amount: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
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
    if !auth_user.has_permission(perm::ORDERS_READ) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to read orders".to_string(),
        ));
    }

    // Use service layer
    let svc = state.services.order.clone();
    let result = svc.list_orders(query.page, query.limit).await?;
    let total_pages = (result.total + query.limit - 1) / query.limit;
    let items: Vec<OrderResponse> = result
        .orders
        .iter()
        .map(|order| map_service_order(order, None))
        .collect::<Result<_, _>>()?;
    Ok(Json(ApiResponse::success(PaginatedResponse {
        items,
        total: result.total,
        page: query.page,
        limit: query.limit,
        total_pages,
    })))
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
) -> Result<(StatusCode, Json<ApiResponse<OrderResponse>>), ServiceError> {
    // Check permissions
    if !auth_user.has_permission(perm::ORDERS_CREATE) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to create orders".to_string(),
        ));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field,
                        error.message.as_ref().unwrap_or(&"Invalid value".into())
                    )
                })
            })
            .collect();
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::validation_errors(errors)),
        ));
    }

    // Parse and validate identifiers
    let customer_uuid = Uuid::parse_str(&request.customer_id).map_err(|_| {
        ServiceError::ValidationError("customer_id must be a valid UUID".to_string())
    })?;

    struct PreparedItem {
        api_item: OrderItem,
        variant_id: Uuid,
        storage_sku: String,
        tax_rate: Option<Decimal>,
    }

    // Compute totals and validate items against catalog data
    let mut total_amount = Decimal::ZERO;
    let mut prepared_items: Vec<PreparedItem> = Vec::with_capacity(request.items.len());
    for (index, item) in request.items.iter().enumerate() {
        let context = format!("items[{index}].product_id");
        let variant = resolve_variant_identifier(
            state.services.product_catalog.as_ref(),
            &item.product_id,
            &context,
        )
        .await?;

        let storage_sku = if variant.sku.is_empty() {
            variant.id.to_string()
        } else {
            variant.sku.clone()
        };
        let display_sku = if variant.sku.is_empty() {
            None
        } else {
            Some(variant.sku.clone())
        };
        let product_name = if variant.name.is_empty() {
            storage_sku.clone()
        } else {
            variant.name.clone()
        };

        if let Some(provided_price) = item.unit_price {
            if provided_price != variant.price {
                return Err(ServiceError::ValidationError(format!(
                    "items[{index}].unit_price ({}) does not match catalog price ({}) for SKU {}",
                    provided_price, variant.price, storage_sku
                )));
            }
        }

        let unit_price = item.unit_price.unwrap_or(variant.price);
        let total_price = unit_price * Decimal::from(item.quantity);
        let tax_amount = item.tax_rate.map(|rate| total_price * rate);

        total_amount += total_price;
        prepared_items.push(PreparedItem {
            api_item: OrderItem {
                id: format!("item_{}", index + 1),
                product_id: variant.id.to_string(),
                product_name,
                sku: display_sku,
                quantity: item.quantity,
                unit_price,
                total_price,
                tax_amount,
            },
            variant_id: variant.id,
            storage_sku,
            tax_rate: item.tax_rate,
        });
    }

    // Persist minimal order header via service
    let created = state
        .services
        .order
        .create_order_minimal(
            customer_uuid,
            total_amount,
            Some("USD".to_string()),
            request.notes.clone(),
            request.shipping_address.as_ref().map(format_order_address),
            request.billing_address.as_ref().map(format_order_address),
            request.payment_method_id.clone(),
        )
        .await?;

    // Persist items for this order
    let created_id = created.id;
    for prepared in &prepared_items {
        let _ = state
            .services
            .order
            .add_order_item(
                created_id,
                prepared.storage_sku.clone(),
                Some(prepared.variant_id),
                Some(prepared.api_item.product_name.clone()),
                prepared.api_item.quantity,
                prepared.api_item.unit_price,
                prepared.tax_rate,
            )
            .await?;
    }

    // Build API response using created header, then re-fetch items from DB
    let persisted = state.services.order.get_order_items(created_id).await?;
    let api_order = map_service_order(&created, Some(persisted.as_slice()))?;
    Ok((StatusCode::CREATED, Json(ApiResponse::success(api_order))))
}

/// Get order by its public order number (explicit route)
#[utoipa::path(
    get,
    path = "/api/v1/orders/by-number/{order_number}",
    summary = "Get order by number",
    description = "Retrieve an order by its public order number (e.g., ORD-ABC123)",
    params(("order_number" = String, Path, description = "Public order number")),
    responses(
        (status = 200, description = "Order retrieved successfully", body = ApiResponse<OrderResponse>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = []))
)]
pub async fn get_order_by_number(
    State(state): State<AppState>,
    Path(order_number): Path<String>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    if !auth_user.has_permission(perm::ORDERS_READ) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to read orders".to_string(),
        ));
    }
    let svc = state.services.order.clone();
    match svc.get_order_by_order_number(&order_number).await? {
        Some(order) => {
            let order_id = order.id;
            let items = svc.get_order_items(order_id).await?;
            let response = map_service_order(&order, Some(items.as_slice()))?;
            Ok(Json(ApiResponse::success(response)))
        }
        None => Err(ServiceError::NotFound(format!(
            "Order with number {} not found",
            order_number
        ))),
    }
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
    Path(id): Path<Uuid>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission(perm::ORDERS_READ) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to read orders".to_string(),
        ));
    }
    let svc = state.services.order.clone();
    match svc.get_order(id).await? {
        Some(order) => {
            let items = svc.get_order_items(order.id).await?;
            let response = map_service_order(&order, Some(items.as_slice()))?;
            Ok(Json(ApiResponse::success(response)))
        }
        None => Err(ServiceError::NotFound(format!(
            "Order with ID {} not found",
            id
        ))),
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
    if !auth_user.has_permission(perm::ORDERS_UPDATE) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to update orders".to_string(),
        ));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field,
                        error.message.as_ref().unwrap_or(&"Invalid value".into())
                    )
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    let order_id = resolve_order_id(&state, &id).await?;
    let update_details = UpdateOrderDetails {
        shipping_address: request.shipping_address.as_ref().map(format_order_address),
        billing_address: request.billing_address.as_ref().map(format_order_address),
        payment_method: request.payment_method_id.clone(),
        notes: request.notes.clone(),
    };

    let updated = state
        .services
        .order
        .update_order_details(order_id, update_details)
        .await?;
    let items = state.services.order.get_order_items(order_id).await?;
    let response = map_service_order(&updated, Some(items.as_slice()))?;

    Ok(Json(ApiResponse::success(response)))
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
        (status = 204, description = "Order deleted successfully"),
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
) -> Result<impl IntoResponse, ServiceError> {
    // Check permissions
    if !auth_user.has_permission(perm::ORDERS_DELETE) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to delete orders".to_string(),
        ));
    }

    let order_id = resolve_order_id(&state, &id).await?;
    state.services.order.delete_order(order_id).await?;

    Ok(StatusCode::NO_CONTENT)
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
    if !auth_user.has_permission(perm::ORDERS_UPDATE) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to update orders".to_string(),
        ));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field,
                        error.message.as_ref().unwrap_or(&"Invalid value".into())
                    )
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    // Use service to update status
    let order_id = resolve_order_id(&state, &id).await?;
    let status_str = match request.status {
        OrderStatus::Pending => "pending",
        OrderStatus::Confirmed => "processing",
        OrderStatus::Processing => "processing",
        OrderStatus::Shipped => "shipped",
        OrderStatus::Delivered => "delivered",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Refunded => "refunded",
    };
    let svc = state.services.order.clone();
    let updated = svc
        .update_order_status(
            order_id,
            svc_orders::UpdateOrderStatusRequest {
                status: status_str.to_string(),
                notes: request.reason,
            },
        )
        .await?;
    let items = svc.get_order_items(order_id).await?;
    let order = map_service_order(&updated, Some(items.as_slice()))?;
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
    if !auth_user.has_permission(perm::ORDERS_READ) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to read orders".to_string(),
        ));
    }

    let order_id = resolve_order_id(&state, &id).await?;
    let svc = state.services.order.clone();
    let items = svc.get_order_items(order_id).await?;
    let mapped: Vec<OrderItem> = items.iter().map(map_order_item_model).collect();
    Ok(Json(ApiResponse::success(mapped)))
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
    if !auth_user.has_permission(perm::ORDERS_UPDATE) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to update orders".to_string(),
        ));
    }

    // Validate the request
    if let Err(validation_errors) = request.validate() {
        let errors: Vec<String> = validation_errors
            .field_errors()
            .iter()
            .flat_map(|(field, errors)| {
                let field = field.clone();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field,
                        error.message.as_ref().unwrap_or(&"Invalid value".into())
                    )
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    let order_id = resolve_order_id(&state, &id).await?;

    let variant = resolve_variant_identifier(
        state.services.product_catalog.as_ref(),
        &request.product_id,
        "product_id",
    )
    .await?;

    if let Some(provided_price) = request.unit_price {
        if provided_price != variant.price {
            let sku_display = if variant.sku.is_empty() {
                variant.id.to_string()
            } else {
                variant.sku.clone()
            };
            return Err(ServiceError::ValidationError(format!(
                "unit_price ({}) does not match catalog price ({}) for SKU {}",
                provided_price, variant.price, sku_display
            )));
        }
    }

    let unit_price = request.unit_price.unwrap_or(variant.price);
    let storage_sku = if variant.sku.is_empty() {
        variant.id.to_string()
    } else {
        variant.sku.clone()
    };
    let product_name = if variant.name.is_empty() {
        storage_sku.clone()
    } else {
        variant.name.clone()
    };

    let saved = state
        .services
        .order
        .add_order_item(
            order_id,
            storage_sku,
            Some(variant.id),
            Some(product_name),
            request.quantity,
            unit_price,
            request.tax_rate,
        )
        .await?;

    let item = map_order_item_model(&saved);
    Ok(Json(ApiResponse::success(item)))
}

/// Cancel an existing order
pub async fn cancel_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
    Json(payload): Json<serde_json::Value>,
) -> Result<impl IntoResponse, ServiceError> {
    if !auth_user.has_permission(perm::ORDERS_CANCEL) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to cancel orders".to_string(),
        ));
    }
    let order_id = resolve_order_id(&state, &id).await?;
    let reason = payload
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("Customer request")
        .to_string();

    let _ = state
        .services
        .order
        .cancel_order(order_id, Some(reason.clone()))
        .await?;

    let response = json!({
        "message": format!("Order {} has been cancelled", id),
        "order_id": id,
        "status": "cancelled",
        "cancellation_reason": reason,
        "cancelled_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}

/// Archive an existing order
pub async fn archive_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
) -> Result<impl IntoResponse, ServiceError> {
    if !auth_user.has_permission(perm::ORDERS_UPDATE) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to archive orders".to_string(),
        ));
    }
    let order_id = resolve_order_id(&state, &id).await?;
    let _ = state.services.order.archive_order(order_id).await?;

    let response = json!({
        "message": format!("Order {} has been archived", id),
        "order_id": id,
        "status": "archived",
        "archived_at": Utc::now()
    });

    Ok((StatusCode::OK, Json(response)))
}
// (Old command-based cancel/archive removed)
