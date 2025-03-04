use axum::{
    routing::{post, get, put, delete},
    extract::{State, Path, Query, Json},
    response::IntoResponse,
    Router,
};
use crate::{
    entities::order::{Model as OrderModel, ActiveModel as OrderActiveModel},
    errors::{ServiceError, AppError, ApiError},
    auth::AuthenticatedUser,
    handlers::common::PaginationParams,
    AppState,
};
use std::sync::Arc;
use serde::{Serialize, Deserialize};
use validator::Validate;
use chrono::{DateTime, Utc};
use serde_json::json;
use tracing::info;

// Import the commands
use crate::commands::orders::{
    CreateOrderCommand, ApplyOrderDiscountCommand, CancelOrderCommand,
    UpdateOrderItemsCommand, PartialCancelOrderCommand, AddItemToOrderCommand,
    RemoveItemFromOrderCommand, ShipOrderCommand,
};

// Structs remain the same

// DTO for creating orders
#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrderRequest {
    pub customer_id: uuid::Uuid,
    #[validate(length(min = 1, message = "At least one item is required"))]
    pub items: Vec<crate::commands::orders::create_order_command::OrderItem>,
}

async fn create_order(
    State(state): State<Arc<crate::AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(order_info): Json<CreateOrderRequest>,
) -> Result<impl IntoResponse, crate::errors::ApiError> {
    order_info.validate().map_err(|e| crate::errors::ApiError::BadRequest(e.to_string()))?;

    let command = crate::commands::orders::create_order_command::CreateOrderCommand {
        customer_id: order_info.customer_id,
        items: order_info.items,
    };

    let order_id = state.services.orders.create_order(command).await
        .map_err(|e| crate::errors::ApiError::from(e))?;
    
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
    let order = state.order_repository.find_by_id(id).await?
        .ok_or_else(|| ApiError::NotFound(format!("Order with ID {} not found", id)))?;
    
    tracing::info!("Order retrieved: {}", id);
    
    Ok(Json(order))
}

async fn update_order_items(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(items_info): Json<UpdateOrderItemsCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    items_info.validate()?;

    let command = UpdateOrderItemsCommand {
        order_id,
        items: items_info.items,
    };

    let result = command.execute(db_pool, event_sender).await?;
    info!("Order items updated for order ID {}: {:?}", order_id, result);
    Ok(Json(result))
}

async fn delete_order(
    State(order_service): State<Arc<OrderService>>,
    Path(id): Path<i32>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    order_service.delete_order(id, user.user_id).await?;
    info!("Order deleted by user {}: {}", user.user_id, id);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn list_orders(
    State(order_service): State<Arc<OrderService>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Query(query): Query<PaginationParams>,
) -> Result<impl IntoResponse, ServiceError> {
    let (orders, total) = order_service.list_orders(user.user_id, query).await?;
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
) -> Result<impl IntoResponse, ServiceError> {
    let (orders, total) = order_service.search_orders(user.user_id, query).await?;
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
    Path(order_id): Path<i32>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(item_info): Json<CreateOrderItemRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    item_info.validate()?;

    let command = AddItemToOrderCommand {
        order_id,
        product_id: item_info.product_id,
        quantity: item_info.quantity,
    };

    let order_item = command.execute(db_pool, event_sender).await?;
    info!("Item added to order by user {}: {:?}", user.user_id, order_item);
    Ok((axum::http::StatusCode::CREATED, Json(order_item)))
}

async fn remove_item_from_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path((order_id, item_id)): Path<(i32, i32)>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let command = RemoveItemFromOrderCommand { order_id, item_id };

    command.execute(db_pool, event_sender).await?;
    info!("Item removed from order by user {}: order_id={}, item_id={}", user.user_id, order_id, item_id);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn partial_cancel_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(cancel_info): Json<PartialCancelOrderCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    cancel_info.validate()?;

    let command = PartialCancelOrderCommand {
        order_id,
        item_ids: cancel_info.item_ids,
    };

    let result = command.execute(db_pool, event_sender).await?;
    info!("Partial order cancellation by user {}: {:?}", order_id, result);
    Ok(Json(result))
}

async fn cancel_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(cancel_info): Json<CancelOrderCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    cancel_info.validate()?;

    let command = CancelOrderCommand {
        order_id,
        reason: cancel_info.reason,
    };

    let result = command.execute(db_pool, event_sender).await?;
    info!("Order {} canceled by user {}: reason={}", order_id, user.user_id, cancel_info.reason);

    Ok(Json(result))
}

async fn ship_order(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(id): Path<i32>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    let command = ShipOrderCommand {
        order_id: id,
        user_id: user.user_id,
    };

    let shipped_order = command.execute(db_pool, event_sender).await?;
    info!("Order {} shipped by user {}", id, user.user_id);
    Ok(Json(shipped_order))
}

async fn apply_discount(
    State(db_pool): State<Arc<DbPool>>,
    State(event_sender): State<Arc<EventSender>>,
    Path(order_id): Path<i32>,
    Json(discount_info): Json<ApplyOrderDiscountCommand>,
) -> Result<impl IntoResponse, ServiceError> {
    discount_info.validate()?;

    let command = ApplyOrderDiscountCommand {
        order_id,
        discount_amount: discount_info.discount_amount,
    };

    let result = command.execute(db_pool, event_sender).await?;
    info!("Discount applied to order {}: amount={}", order_id, discount_info.discount_amount);
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