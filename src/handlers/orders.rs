use axum::{
    routing::{post, get, put, delete},
    extract::{State, Path, Query, Json},
    response::IntoResponse,
    Router,
};
use crate::{
    entities::order::{Model as OrderModel, ActiveModel as OrderActiveModel},
    errors::ApiError,
    auth::AuthenticatedUser,
    handlers::common::{
        PaginationParams, validate_input, map_service_error, success_response,
        created_response, no_content_response,
    },
    db::DbPool,
    events::EventSender,
    services::orders::OrderService,
    AppState,
};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use serde_json::json;
use tracing::info;
use uuid::Uuid;

// Import the commands
use crate::commands::orders::{
    CreateOrderCommand, ApplyOrderDiscountCommand, CancelOrderCommand,
    UpdateOrderItemsCommand, PartialCancelOrderCommand, AddItemToOrderCommand,
    RemoveItemFromOrderCommand, ShipOrderCommand, ApplyPromotionToOrderCommand,
    DeactivatePromotionCommand, ExchangeOrderCommand, RefundOrderCommand,
    HoldOrderCommand, ReleaseOrderFromHoldCommand, TagOrderCommand,
    UpdateShippingAddressCommand, UpdateOrderStatusCommand,
};
use crate::models::order::OrderStatus;

// Structs remain the same

// DTO for creating orders
#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrderRequest {
    pub customer_id: uuid::Uuid,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<crate::commands::orders::create_order_command::OrderItem>,
}

#[derive(Debug, Deserialize, Validate)]
struct PromotionRequest {
    pub promotion_id: i32,
}

#[derive(Debug, Deserialize, Validate)]
struct ExchangeOrderRequest {
    #[validate]
    pub return_items: Vec<crate::commands::orders::exchange_order_command::ReturnItemInput>,
    #[validate]
    pub new_items: Vec<crate::commands::orders::exchange_order_command::OrderItemInput>,
}

#[derive(Debug, Deserialize, Validate)]
struct RefundOrderRequest {
    #[validate(range(min = 0.01))]
    pub refund_amount: f64,
    #[validate(length(min = 1, max = 500))]
    pub reason: String,
}

#[derive(Debug, Deserialize, Validate)]
struct HoldOrderRequest {
    #[validate(length(min = 1, max = 500))]
    pub reason: String,
    pub version: i32,
}

#[derive(Debug, Deserialize, Validate)]
struct TagOrderRequest {
    pub tag_id: i32,
}

#[derive(Debug, Deserialize, Validate)]
struct UpdateShippingRequest {
    #[validate(length(min = 5, max = 255))]
    pub new_address: String,
}

#[derive(Debug, Deserialize, Validate)]
struct CreateOrderItemRequest {
    pub product_id: uuid::Uuid,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

async fn create_order(
    State(state): State<Arc<crate::AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(order_info): Json<CreateOrderRequest>,
) -> Result<impl IntoResponse, crate::errors::ApiError> {
    validate_input(&order_info)?;

    let command = crate::commands::orders::create_order_command::CreateOrderCommand {
        customer_id: order_info.customer_id,
        items: order_info.items,
    };

    let order_id = state.services.orders.create_order(command).await
        .map_err(map_service_error)?;
    
    info!("Order created by user {}: {:?}", user.user_id, order_id);
    Ok((axum::http::StatusCode::CREATED, Json(serde_json::json!({
        "order_id": order_id,
        "status": "created"
    }))))
}

async fn get_order(
    State(state): State<Arc<AppState>>,
    Path(id): Path<uuid::Uuid>,
    // AuthenticatedUser check temporarily commented out for testing
    // AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ApiError> {
    let order = state
        .order_repository
        .find_by_id(id)
        .await
        .map_err(map_service_error)?
        .ok_or_else(|| ApiError::NotFound(format!("Order with ID {} not found", id)))?;
    
    tracing::info!("Order retrieved: {}", id);
    
    Ok(Json(order))
}

async fn update_order_items(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(items_info): Json<UpdateOrderItemsCommand>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&items_info)?;

    let command = UpdateOrderItemsCommand {
        order_id,
        items: items_info.items,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;
    info!("Order items updated for order ID {}: {:?}", order_id, result);
    Ok(Json(result))
}

async fn delete_order(
    State(order_service): State<Arc<OrderService>>,
    Path(id): Path<i32>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ApiError> {
    order_service
        .delete_order(id, user.user_id)
        .await
        .map_err(map_service_error)?;
    info!("Order deleted by user {}: {}", user.user_id, id);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_orders(
    State(order_service): State<Arc<OrderService>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Query(query): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (orders, total) = order_service
        .list_orders(user.user_id, query)
        .await
        .map_err(map_service_error)?;
    info!("Orders listed by user {}: total {}", user.user_id, total);
    Ok(Json(json!({
        "orders": orders,
        "total": total,
        "page": query.page,
        "per_page": query.per_page
    })))
}

async fn search_orders(
    State(order_service): State<Arc<OrderService>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Query(query): Query<OrderSearchParams>,
) -> Result<impl IntoResponse, ApiError> {
    let (orders, total) = order_service
        .search_orders(user.user_id, query)
        .await
        .map_err(map_service_error)?;
    info!("Orders searched by user {}: total {}", user.user_id, total);
    Ok(Json(json!({
        "orders": orders,
        "total": total,
        "query": query
    })))
}

async fn add_item_to_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<uuid::Uuid>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(item_info): Json<CreateOrderItemRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&item_info)?;

    let command = AddItemToOrderCommand {
        order_id,
        product_id: item_info.product_id,
        quantity: item_info.quantity,
    };

    let order_item = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;
    info!("Item added to order by user {}: {:?}", user.user_id, order_item);
    Ok((axum::http::StatusCode::CREATED, Json(order_item)))
}

async fn remove_item_from_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path((order_id, item_id)): Path<(i32, i32)>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ApiError> {
    let command = RemoveItemFromOrderCommand { order_id, item_id };

    command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;
    info!("Item removed from order by user {}: order_id={}, item_id={}", user.user_id, order_id, item_id);
    Ok(no_content_response())
}

async fn partial_cancel_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(cancel_info): Json<PartialCancelOrderCommand>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&cancel_info)?;

    let command = PartialCancelOrderCommand {
        order_id,
        item_ids: cancel_info.item_ids,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;
    info!("Partial order cancellation by user {}: {:?}", order_id, result);
    Ok(Json(result))
}

async fn cancel_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(cancel_info): Json<CancelOrderCommand>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&cancel_info)?;

    let command = CancelOrderCommand {
        order_id,
        reason: cancel_info.reason,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;
    info!("Order {} canceled by user {}: reason={}", order_id, user.user_id, cancel_info.reason);

    Ok(Json(result))
}

async fn ship_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(id): Path<i32>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ApiError> {
    let command = ShipOrderCommand {
        order_id: id,
        user_id: user.user_id,
    };

    let shipped_order = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;
    info!("Order {} shipped by user {}", id, user.user_id);
    Ok(Json(shipped_order))
}

async fn apply_discount(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(discount_info): Json<ApplyOrderDiscountCommand>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&discount_info)?;

    let command = ApplyOrderDiscountCommand {
        order_id,
        discount_amount: discount_info.discount_amount,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;
    info!("Discount applied to order {}: amount={}", order_id, discount_info.discount_amount);
    Ok(Json(result))
}

async fn apply_promotion(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(payload): Json<PromotionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = ApplyPromotionToOrderCommand {
        order_id,
        promotion_id: payload.promotion_id,
    };

    let order = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(order))
}

async fn remove_promotion(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(_order_id): Path<i32>,
    Json(payload): Json<PromotionRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = DeactivatePromotionCommand {
        promotion_id: payload.promotion_id,
    };

    let promo = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(promo))
}

async fn exchange_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<uuid::Uuid>,
    Json(payload): Json<ExchangeOrderRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = ExchangeOrderCommand {
        order_id,
        return_items: payload.return_items,
        new_items: payload.new_items,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(result))
}

async fn refund_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<uuid::Uuid>,
    Json(payload): Json<RefundOrderRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = RefundOrderCommand {
        order_id,
        refund_amount: payload.refund_amount,
        reason: payload.reason,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(result))
}

async fn complete_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let command = UpdateOrderStatusCommand {
        order_id,
        new_status: OrderStatus::Delivered,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(result))
}

async fn hold_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<uuid::Uuid>,
    Json(payload): Json<HoldOrderRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = HoldOrderCommand {
        order_id,
        reason: payload.reason,
        version: payload.version,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(result))
}

async fn release_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<uuid::Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let command = ReleaseOrderFromHoldCommand { order_id };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(result))
}

async fn tag_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(payload): Json<TagOrderRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = TagOrderCommand {
        order_id,
        tag_id: payload.tag_id,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(result))
}

async fn update_shipping(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(payload): Json<UpdateShippingRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    let command = UpdateShippingAddressCommand {
        order_id,
        new_address: payload.new_address,
    };

    let result = command
        .execute(db_pool, event_sender)
        .await
        .map_err(map_service_error)?;

    Ok(Json(result))
}

pub fn orders_routes() -> Router {
    Router::new()
        .route("/", post(create_order))
        .route("/", get(list_orders))
        .route("/search", get(search_orders))
        .route("/create", post(create_order))
        .route("/:id", get(get_order))
        .route("/:id", delete(delete_order))
        .route("/:id/items", put(update_order_items))
        .route("/:id/items", post(add_item_to_order))
        .route("/:order_id/items/:item_id", delete(remove_item_from_order))
        .route("/:id/partial_cancel", post(partial_cancel_order))
        .route("/:id/cancel", post(cancel_order))
        .route("/:id/ship", post(ship_order))
        .route("/:id/apply_discount", put(apply_discount))
        .route("/:id/apply_promotion", put(apply_promotion))
        .route("/:id/remove_promotion", delete(remove_promotion))
        .route("/:id/exchange", post(exchange_order))
        .route("/:id/refund", post(refund_order))
        .route("/:id/delete", delete(delete_order))
        .route("/:id/complete", post(complete_order))
        .route("/:id/hold", post(hold_order))
        .route("/:id/release", post(release_order))
        .route("/:id/tag", post(tag_order))
        .route("/:id/update_shipping", put(update_shipping))
}