use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

use crate::auth::consts as perm;
use crate::entities::commerce::product_variant;
use crate::entities::order_item;
use crate::{auth::AuthUser, errors::ServiceError, ApiResponse, AppState, PaginatedResponse};
// Commands are not directly used by handlers at this time
use crate::services::commerce::product_catalog_service::ProductCatalogService;
use crate::services::orders::{
    self as svc_orders, OrderSearchQuery, OrderSortField, SortDirection, UpdateOrderDetails,
};

const DEFAULT_PAGE: u64 = 1;
const DEFAULT_LIMIT: u64 = 20;
const MAX_LIMIT: u64 = 100;

fn default_page() -> u64 {
    DEFAULT_PAGE
}

fn default_limit() -> u64 {
    DEFAULT_LIMIT
}

fn validate_orders_list_query(query: &OrdersListQuery) -> Result<(), ServiceError> {
    if query.page == 0 {
        return Err(ServiceError::ValidationError(
            "page must be greater than zero".to_string(),
        ));
    }

    if query.limit == 0 {
        return Err(ServiceError::ValidationError(
            "limit must be greater than zero".to_string(),
        ));
    }

    if query.limit > MAX_LIMIT {
        return Err(ServiceError::ValidationError(format!(
            "limit cannot exceed {}",
            MAX_LIMIT
        )));
    }

    Ok(())
}

fn parse_query_datetime(
    name: &str,
    value: &Option<String>,
) -> Result<Option<DateTime<Utc>>, ServiceError> {
    if let Some(raw) = value {
        let parsed = DateTime::parse_from_rfc3339(raw).map_err(|_| {
            ServiceError::ValidationError(format!("{name} must be an RFC3339 timestamp"))
        })?;
        Ok(Some(parsed.with_timezone(&Utc)))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(default)]
pub struct OrdersListQuery {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<OrderStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub customer_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<OrderSortField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<SortDirection>,
    pub include_items: bool,
}

impl Default for OrdersListQuery {
    fn default() -> Self {
        Self {
            page: DEFAULT_PAGE,
            limit: DEFAULT_LIMIT,
            status: None,
            customer_id: None,
            search: None,
            from: None,
            to: None,
            sort_by: None,
            sort_order: None,
            include_items: false,
        }
    }
}

fn map_status_str(status: &str) -> Result<OrderStatus, ServiceError> {
    match status.to_ascii_lowercase().as_str() {
        "pending" => Ok(OrderStatus::Pending),
        "confirmed" => Ok(OrderStatus::Confirmed),
        "processing" => Ok(OrderStatus::Processing),
        "on_hold" | "onhold" => Ok(OrderStatus::OnHold),
        "shipped" => Ok(OrderStatus::Shipped),
        "delivered" => Ok(OrderStatus::Delivered),
        "cancelled" | "canceled" => Ok(OrderStatus::Cancelled),
        "refunded" => Ok(OrderStatus::Refunded),
        "exchanged" => Ok(OrderStatus::Exchanged),
        other => Err(ServiceError::InvalidStatus(format!(
            "Unknown order status: {other}"
        ))),
    }
}

fn order_status_to_service_str(status: &OrderStatus) -> &'static str {
    match status {
        OrderStatus::Pending => "pending",
        OrderStatus::Confirmed => "confirmed",
        OrderStatus::Processing => "processing",
        OrderStatus::OnHold => "on_hold",
        OrderStatus::Shipped => "shipped",
        OrderStatus::Delivered => "delivered",
        OrderStatus::Cancelled => "cancelled",
        OrderStatus::Refunded => "refunded",
        OrderStatus::Exchanged => "exchanged",
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
        order_number: order.order_number.clone(),
        customer_id: order.customer_id.to_string(),
        status,
        order_date: order.order_date,
        total_amount: Some(order.total_amount),
        currency: Some(order.currency.clone()),
        payment_status: order.payment_status.clone(),
        fulfillment_status: order.fulfillment_status.clone(),
        items: mapped_items,
        shipping_address,
        billing_address,
        payment_method_id: order.payment_method.clone(),
        shipment_id: order.tracking_number.clone(),
        notes: order.notes.clone(),
        version: order.version,
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
        discount: (!model.discount.is_zero()).then_some(model.discount),
        tax_rate: (!model.tax_rate.is_zero()).then_some(model.tax_rate),
        tax_amount: Some(model.tax_amount),
    }
}

// Trait for order handler state - blanket implementation for all compatible types
pub trait OrderHandlerState: Clone + Send + Sync + 'static {}

// Blanket implementation for any type that satisfies the bounds
impl<T> OrderHandlerState for T where T: Clone + Send + Sync + 'static {}

// Order DTOs
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "order_number": "ORD-2024-001234",
    "customer_id": "123e4567-e89b-12d3-a456-426614174000",
    "status": "pending",
    "order_date": "2024-12-09T10:30:00Z",
    "total_amount": "149.99",
    "currency": "USD",
    "payment_status": "pending",
    "fulfillment_status": "unfulfilled",
    "items": [{
        "id": "item-001",
        "product_id": "prod-abc123",
        "product_name": "Wireless Bluetooth Headphones",
        "sku": "WBH-BLK-001",
        "quantity": 2,
        "unit_price": "49.99",
        "total_price": "99.98",
        "tax_rate": "0.08",
        "tax_amount": "8.00"
    }],
    "shipping_address": {
        "street": "123 Main Street",
        "city": "San Francisco",
        "state": "CA",
        "postal_code": "94102",
        "country": "US"
    },
    "billing_address": {
        "street": "123 Main Street",
        "city": "San Francisco",
        "state": "CA",
        "postal_code": "94102",
        "country": "US"
    },
    "payment_method_id": "pm_card_visa",
    "notes": "Please deliver to back door",
    "version": 1,
    "created_at": "2024-12-09T10:30:00Z",
    "updated_at": "2024-12-09T10:30:00Z"
}))]
pub struct OrderResponse {
    /// Unique order identifier (UUID)
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: String,
    /// Human-readable order number
    #[schema(example = "ORD-2024-001234")]
    pub order_number: String,
    /// Customer identifier (UUID)
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub customer_id: String,
    /// Current order status
    pub status: OrderStatus,
    /// Date when the order was placed
    pub order_date: DateTime<Utc>,
    /// Total order amount
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "149.99")]
    pub total_amount: Option<rust_decimal::Decimal>,
    /// Currency code (ISO 4217)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "USD")]
    pub currency: Option<String>,
    /// Payment status (pending, paid, failed, refunded)
    #[schema(example = "pending")]
    pub payment_status: String,
    /// Fulfillment status (unfulfilled, partially_fulfilled, fulfilled)
    #[schema(example = "unfulfilled")]
    pub fulfillment_status: String,
    /// Order line items
    pub items: Vec<OrderItem>,
    /// Shipping address
    pub shipping_address: Option<Address>,
    /// Billing address
    pub billing_address: Option<Address>,
    /// Payment method identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "pm_card_visa")]
    pub payment_method_id: Option<String>,
    /// Associated shipment identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "ship_abc123")]
    pub shipment_id: Option<String>,
    /// Order notes
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Please deliver to back door")]
    pub notes: Option<String>,
    /// Version number for optimistic locking
    #[schema(example = 1)]
    pub version: i32,
    /// Creation timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last update timestamp
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(example = json!({
    "customer_id": "123e4567-e89b-12d3-a456-426614174000",
    "items": [
        {
            "product_id": "prod-abc123",
            "quantity": 2,
            "unit_price": "49.99",
            "tax_rate": "0.08"
        },
        {
            "product_id": "SKU-WIDGET-001",
            "quantity": 1,
            "unit_price": "29.99"
        }
    ],
    "shipping_address": {
        "street": "123 Main Street",
        "city": "San Francisco",
        "state": "CA",
        "postal_code": "94102",
        "country": "US"
    },
    "billing_address": {
        "street": "456 Billing Ave",
        "city": "San Francisco",
        "state": "CA",
        "postal_code": "94102",
        "country": "US"
    },
    "payment_method_id": "pm_card_visa",
    "notes": "Please gift wrap"
}))]
pub struct CreateOrderRequest {
    /// Customer UUID
    #[validate(length(min = 1))]
    #[schema(example = "123e4567-e89b-12d3-a456-426614174000")]
    pub customer_id: String,

    /// Order line items (at least one required)
    #[validate(length(min = 1))]
    pub items: Vec<CreateOrderItem>,

    /// Shipping address for delivery
    pub shipping_address: Option<Address>,
    /// Billing address for payment
    pub billing_address: Option<Address>,
    /// Payment method identifier (e.g., Stripe payment method ID)
    #[schema(example = "pm_card_visa")]
    pub payment_method_id: Option<String>,
    /// Optional notes for the order
    #[schema(example = "Please gift wrap")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(example = json!({
    "shipping_address": {
        "street": "789 New Address Blvd",
        "city": "Los Angeles",
        "state": "CA",
        "postal_code": "90001",
        "country": "US"
    },
    "notes": "Updated delivery instructions"
}))]
pub struct UpdateOrderRequest {
    /// Updated shipping address
    pub shipping_address: Option<Address>,
    /// Updated billing address
    pub billing_address: Option<Address>,
    /// Updated payment method ID
    #[schema(example = "pm_card_mastercard")]
    pub payment_method_id: Option<String>,
    /// Updated order notes
    #[schema(example = "Leave at front door")]
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(example = json!({
    "product_id": "SKU-WIDGET-001",
    "quantity": 2,
    "unit_price": "49.99",
    "tax_rate": "0.08"
}))]
pub struct CreateOrderItem {
    /// Variant identifier; accepts either a UUID or SKU string
    #[serde(alias = "sku")]
    #[validate(length(min = 1))]
    #[schema(example = "SKU-WIDGET-001")]
    pub product_id: String,

    /// Quantity to order (must be at least 1)
    #[validate(range(min = 1))]
    #[schema(example = 2)]
    pub quantity: i32,

    /// Unit price (optional, will use catalog price if not provided)
    #[serde(alias = "price")]
    #[schema(example = "49.99")]
    pub unit_price: Option<rust_decimal::Decimal>,
    /// Tax rate as decimal (e.g., 0.08 for 8%)
    #[schema(example = "0.08")]
    pub tax_rate: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(example = json!({
    "id": "item-550e8400-e29b-41d4",
    "product_id": "prod-abc123",
    "product_name": "Wireless Bluetooth Headphones",
    "sku": "WBH-BLK-001",
    "quantity": 2,
    "unit_price": "49.99",
    "total_price": "99.98",
    "discount": "5.00",
    "tax_rate": "0.08",
    "tax_amount": "7.60"
}))]
pub struct OrderItem {
    /// Line item ID
    #[schema(example = "item-550e8400-e29b-41d4")]
    pub id: String,
    /// Product variant ID
    #[schema(example = "prod-abc123")]
    pub product_id: String,
    /// Product display name
    #[schema(example = "Wireless Bluetooth Headphones")]
    pub product_name: String,
    /// Product SKU
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "WBH-BLK-001")]
    pub sku: Option<String>,
    /// Quantity ordered
    #[schema(example = 2)]
    pub quantity: i32,
    /// Price per unit
    #[schema(example = "49.99")]
    pub unit_price: rust_decimal::Decimal,
    /// Total price (unit_price × quantity)
    #[schema(example = "99.98")]
    pub total_price: rust_decimal::Decimal,
    /// Discount amount applied
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "5.00")]
    pub discount: Option<rust_decimal::Decimal>,
    /// Tax rate as decimal
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "0.08")]
    pub tax_rate: Option<rust_decimal::Decimal>,
    /// Calculated tax amount
    #[schema(example = "7.60")]
    pub tax_amount: Option<rust_decimal::Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(example = json!({
    "street": "123 Main Street, Apt 4B",
    "city": "San Francisco",
    "state": "CA",
    "postal_code": "94102",
    "country": "US"
}))]
pub struct Address {
    /// Street address (including apt/suite if applicable)
    #[schema(example = "123 Main Street, Apt 4B")]
    pub street: String,
    /// City name
    #[schema(example = "San Francisco")]
    pub city: String,
    /// State or province code
    #[schema(example = "CA")]
    pub state: String,
    /// Postal/ZIP code
    #[schema(example = "94102")]
    pub postal_code: String,
    /// Country code (ISO 3166-1 alpha-2)
    #[schema(example = "US")]
    pub country: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OrderStatus {
    Pending,
    Confirmed,
    Processing,
    OnHold,
    Shipped,
    Delivered,
    Cancelled,
    Refunded,
    Exchanged,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(example = json!({
    "status": "processing",
    "reason": "Payment confirmed, beginning fulfillment"
}))]
pub struct UpdateOrderStatusRequest {
    /// New order status
    pub status: OrderStatus,
    /// Reason for status change (optional but recommended)
    #[schema(example = "Payment confirmed, beginning fulfillment")]
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(example = json!({
    "reason": "Customer requested cancellation"
}))]
pub struct CancelOrderRequest {
    /// Cancellation reason
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Customer requested cancellation")]
    pub reason: Option<String>,
}

/// List orders with pagination and filtering
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    summary = "List orders",
    description = "Get a paginated list of orders with optional filtering, search, and sorting",
    params(
        ("page" = Option<u64>, Query, description = "Page number (default: 1)"),
        ("limit" = Option<u64>, Query, description = "Items per page (default: 20, max: 100)"),
        ("status" = Option<OrderStatus>, Query, description = "Filter by order status"),
        ("customer_id" = Option<Uuid>, Query, description = "Filter by customer ID"),
        ("search" = Option<String>, Query, description = "Search by order number, notes, or shipping address"),
        ("from" = Option<String>, Query, description = "Only include orders created after this RFC3339 timestamp"),
        ("to" = Option<String>, Query, description = "Only include orders created before this RFC3339 timestamp"),
        ("sort_by" = Option<OrderSortField>, Query, description = "Sort field (created_at, order_date, total_amount, order_number)"),
        ("sort_order" = Option<SortDirection>, Query, description = "Sort direction (asc, desc)"),
        ("include_items" = Option<bool>, Query, description = "If true, include line items for each order in the response"),
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
    Query(query): Query<OrdersListQuery>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<PaginatedResponse<OrderResponse>>>, ServiceError> {
    // Check permissions
    if !auth_user.has_permission(perm::ORDERS_READ) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to read orders".to_string(),
        ));
    }

    validate_orders_list_query(&query)?;

    let svc = state.services.order.clone();

    let status_filter = query
        .status
        .as_ref()
        .map(order_status_to_service_str)
        .map(|s| s.to_string());
    let trimmed_search = query
        .search
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let from = parse_query_datetime("from", &query.from)?;
    let to = parse_query_datetime("to", &query.to)?;
    if let (Some(start), Some(end)) = (from, to) {
        if start > end {
            return Err(ServiceError::ValidationError(
                "`from` must be earlier than or equal to `to`".to_string(),
            ));
        }
    }

    let params = OrderSearchQuery {
        customer_id: query.customer_id,
        status: status_filter,
        from_date: from,
        to_date: to,
        search: trimmed_search,
        sort_field: query.sort_by.unwrap_or(OrderSortField::CreatedAt),
        sort_direction: query.sort_order.unwrap_or(SortDirection::Desc),
        page: query.page,
        per_page: query.limit,
    };

    let result = svc.search_orders(params).await?;
    let include_items = query.include_items;

    let svc_orders::OrderListResponse {
        orders,
        total,
        page,
        per_page,
    } = result;

    let item_lookup: Option<HashMap<Uuid, Vec<order_item::Model>>> =
        if include_items && !orders.is_empty() {
            let ids: Vec<Uuid> = orders.iter().map(|order| order.id).collect();
            Some(svc.get_items_for_orders(&ids).await?)
        } else {
            None
        };

    let mut items = Vec::with_capacity(orders.len());
    for order in &orders {
        let associated_items = item_lookup
            .as_ref()
            .and_then(|lookup| lookup.get(&order.id))
            .map(|vec| vec.as_slice());
        let response = map_service_order(order, associated_items)?;
        items.push(response);
    }

    let total_pages = if per_page == 0 {
        0
    } else {
        (total + per_page - 1) / per_page
    };

    Ok(Json(ApiResponse::success(PaginatedResponse {
        items,
        total,
        page,
        limit: per_page,
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
                let field_name = (*field).to_string();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field_name,
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
        variant_id: Uuid,
        storage_sku: String,
        product_name: String,
        quantity: i32,
        unit_price: Decimal,
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

        total_amount += total_price;
        prepared_items.push(PreparedItem {
            product_name,
            variant_id: variant.id,
            storage_sku,
            quantity: item.quantity,
            unit_price,
            tax_rate: item.tax_rate,
        });
    }

    let item_inputs: Vec<svc_orders::NewOrderItemInput> = prepared_items
        .iter()
        .map(|prepared| svc_orders::NewOrderItemInput {
            sku: prepared.storage_sku.clone(),
            product_id: Some(prepared.variant_id),
            name: Some(prepared.product_name.clone()),
            quantity: prepared.quantity,
            unit_price: prepared.unit_price,
            tax_rate: prepared.tax_rate,
        })
        .collect();

    let shipping_address = request
        .shipping_address
        .as_ref()
        .map(|addr| format_order_address(addr));
    let billing_address = request
        .billing_address
        .as_ref()
        .map(|addr| format_order_address(addr));

    let (created, stored_items) = state
        .services
        .order
        .create_order_with_items(svc_orders::CreateOrderWithItemsInput {
            customer_id: customer_uuid,
            total_amount,
            currency: "USD".to_string(),
            payment_status: "pending".to_string(),
            fulfillment_status: "unfulfilled".to_string(),
            payment_method: request.payment_method_id.clone(),
            shipping_method: None,
            shipping_address,
            billing_address,
            notes: request.notes.clone(),
            items: item_inputs,
        })
        .await?;

    // Build API response using created header, then re-fetch items from DB
    let api_order = map_service_order(&created, Some(stored_items.as_slice()))?;
    Ok((StatusCode::CREATED, Json(ApiResponse::success(api_order))))
}

/// Get order by its public order number (explicit route)
#[utoipa::path(
    get,
    path = "/api/v1/orders/by-number/:order_number}",
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
    path = "/api/v1/orders/:id",
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
    path = "/api/v1/orders/:id",
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
                let field_name = (*field).to_string();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field_name,
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
    path = "/api/v1/orders/:id",
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
    path = "/api/v1/orders/:id/status",
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
                let field_name = (*field).to_string();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field_name,
                        error.message.as_ref().unwrap_or(&"Invalid value".into())
                    )
                })
            })
            .collect();
        return Ok(Json(ApiResponse::validation_errors(errors)));
    }

    // Use service to update status
    let order_id = resolve_order_id(&state, &id).await?;
    let status_str = order_status_to_service_str(&request.status).to_string();
    let svc = state.services.order.clone();
    let updated = svc
        .update_order_status(
            order_id,
            svc_orders::UpdateOrderStatusRequest {
                status: status_str,
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
    path = "/api/v1/orders/:id/items",
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
    path = "/api/v1/orders/:id/items",
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
                let field_name = (*field).to_string();
                errors.iter().map(move |error| {
                    format!(
                        "{}: {}",
                        field_name,
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
#[utoipa::path(
    post,
    path = "/api/v1/orders/:id/cancel",
    summary = "Cancel order",
    description = "Cancel an order and return the updated order",
    params(("id" = String, Path, description = "Order ID")),
    request_body = CancelOrderRequest,
    responses(
        (status = 200, description = "Order cancelled successfully", body = ApiResponse<OrderResponse>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = []))
)]
pub async fn cancel_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
    Json(payload): Json<CancelOrderRequest>,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    if !auth_user.has_permission(perm::ORDERS_CANCEL) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to cancel orders".to_string(),
        ));
    }
    let order_id = resolve_order_id(&state, &id).await?;
    let reason = payload
        .reason
        .filter(|reason| !reason.trim().is_empty())
        .unwrap_or_else(|| "Customer request".to_string());

    let svc = state.services.order.clone();
    let cancelled = svc.cancel_order(order_id, Some(reason.clone())).await?;
    let items = svc.get_order_items(order_id).await?;
    let response = map_service_order(&cancelled, Some(items.as_slice()))?;

    Ok(Json(ApiResponse::success(response)))
}

/// Archive an existing order
#[utoipa::path(
    post,
    path = "/api/v1/orders/:id/archive",
    summary = "Archive order",
    description = "Archive an order and return the updated order record",
    params(("id" = String, Path, description = "Order ID")),
    responses(
        (status = 200, description = "Order archived successfully", body = ApiResponse<OrderResponse>),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Order not found", body = crate::errors::ErrorResponse),
        (status = 422, description = "Validation error", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    security(("Bearer" = []), ("ApiKey" = []))
)]
pub async fn archive_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    auth_user: AuthUser,
) -> Result<Json<ApiResponse<OrderResponse>>, ServiceError> {
    if !auth_user.has_permission(perm::ORDERS_UPDATE) {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions to archive orders".to_string(),
        ));
    }
    let order_id = resolve_order_id(&state, &id).await?;
    let svc = state.services.order.clone();
    let archived = svc.archive_order(order_id).await?;
    let items = svc.get_order_items(order_id).await?;
    let response = map_service_order(&archived, Some(items.as_slice()))?;

    Ok(Json(ApiResponse::success(response)))
}
// (Old command-based cancel/archive removed)
