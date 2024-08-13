use actix_web::{web, HttpResponse, Scope};
use crate::{
    db::DbPool,
    models::order::{OrderStatus, PaymentMethod},
    errors::{ServiceError},
    auth::AuthenticatedUser,
    utils::pagination::PaginationParams,
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

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateOrderRequest {
    #[validate(range(min = 1))]
    pub customer_id: i32,
    #[validate(length(min = 1, max = 255))]
    pub shipping_address: String,
    #[validate(length(min = 1, max = 255))]
    pub billing_address: String,
    pub payment_method: PaymentMethod,
    #[validate]
    pub items: Vec<CreateOrderItemRequest>,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct CreateOrderItemRequest {
    #[validate(range(min = 1))]
    pub product_id: i32,
    #[validate(range(min = 1))]
    pub quantity: i32,
}

#[derive(Debug, Serialize, Deserialize, Validate)]
pub struct UpdateOrderRequest {
    pub status: Option<OrderStatus>,
    #[validate(length(min = 1, max = 255))]
    pub shipping_address: Option<String>,
    #[validate(length(min = 1, max = 255))]
    pub billing_address: Option<String>,
    pub payment_method: Option<PaymentMethod>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderSearchParams {
    pub status: Option<OrderStatus>,
    pub customer_id: Option<i32>,
    pub date_from: Option<DateTime<Utc>>,
    pub date_to: Option<DateTime<Utc>>,
    pub min_total: Option<f64>,
    pub max_total: Option<f64>,
}

async fn create_order(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    order_info: web::Json<CreateOrderRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    order_info.validate()?;

    let command = CreateOrderCommand {
        customer_id: order_info.customer_id,
        items: order_info.items.clone(),
        shipping_address: order_info.shipping_address.clone(),
        billing_address: order_info.billing_address.clone(),
        payment_method: order_info.payment_method,
    };

    let result = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Order created by user {}: {:?}", user.user_id, result);
    Ok(HttpResponse::Created().json(result))
}

async fn get_order(
    order_service: web::Data<OrderService>,
    id: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let order = order_service.get_order(id.into_inner(), user.user_id).await?;
    info!("Order retrieved by user {}: {:?}", user.user_id, order);
    Ok(HttpResponse::Ok().json(order))
}

async fn update_order_items(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    order_id: web::Path<i32>,
    items_info: web::Json<UpdateOrderItemsCommand>,
) -> Result<HttpResponse, ServiceError> {
    items_info.validate()?;

    let command = UpdateOrderItemsCommand {
        order_id: order_id.into_inner(),
        items: items_info.items.clone(),
    };

    let result = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Order items updated for order ID {}: {:?}", order_id, result);
    Ok(HttpResponse::Ok().json(result))
}

async fn delete_order(
    order_service: web::Data<OrderService>,
    id: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    order_service.delete_order(id.into_inner(), user.user_id).await?;
    info!("Order deleted by user {}: {}", user.user_id, id.into_inner());
    Ok(HttpResponse::NoContent().finish())
}

async fn list_orders(
    order_service: web::Data<OrderService>,
    user: AuthenticatedUser,
    query: web::Query<PaginationParams>,
) -> Result<HttpResponse, ServiceError> {
    let (orders, total) = order_service.list_orders(user.user_id, query.into_inner()).await?;
    info!("Orders listed by user {}: total {}", user.user_id, total);
    Ok(HttpResponse::Ok().json(json!({
        "orders": orders,
        "total": total,
        "page": query.page,
        "per_page": query.per_page
    })))
}

async fn search_orders(
    order_service: web::Data<OrderService>,
    user: AuthenticatedUser,
    query: web::Query<OrderSearchParams>,
) -> Result<HttpResponse, ServiceError> {
    let (orders, total) = order_service.search_orders(user.user_id, query.into_inner()).await?;
    info!("Orders searched by user {}: total {}", user.user_id, total);
    Ok(HttpResponse::Ok().json(json!({
        "orders": orders,
        "total": total,
        "query": query.into_inner()
    })))
}

async fn add_item_to_order(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    order_id: web::Path<i32>,
    item_info: web::Json<CreateOrderItemRequest>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    item_info.validate()?;

    let command = AddItemToOrderCommand {
        order_id: order_id.into_inner(),
        product_id: item_info.product_id,
        quantity: item_info.quantity,
    };

    let order_item = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Item added to order by user {}: {:?}", user.user_id, order_item);
    Ok(HttpResponse::Created().json(order_item))
}

async fn remove_item_from_order(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    path: web::Path<(i32, i32)>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let (order_id, item_id) = path.into_inner();

    let command = RemoveItemFromOrderCommand { order_id, item_id };

    command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Item removed from order by user {}: order_id={}, item_id={}", user.user_id, order_id, item_id);
    Ok(HttpResponse::NoContent().finish())
}

async fn partial_cancel_order(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    order_id: web::Path<i32>,
    cancel_info: web::Json<PartialCancelOrderCommand>,
) -> Result<HttpResponse, ServiceError> {
    cancel_info.validate()?;

    let command = PartialCancelOrderCommand {
        order_id: order_id.into_inner(),
        item_ids: cancel_info.item_ids.clone(),
    };

    let result = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Partial order cancellation by user {}: {:?}", order_id, result);
    Ok(HttpResponse::Ok().json(result))
}

async fn cancel_order(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    order_id: web::Path<i32>,
    cancel_info: web::Json<CancelOrderCommand>,
) -> Result<HttpResponse, ServiceError> {
    cancel_info.validate()?;

    let command = CancelOrderCommand {
        order_id: order_id.into_inner(),
        reason: cancel_info.reason.clone(),
    };

    let result = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Order {} canceled by user {}: reason={}", order_id.into_inner(), user.user_id, cancel_info.reason);

    Ok(HttpResponse::Ok().json(result))
}

async fn ship_order(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    id: web::Path<i32>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let command = ShipOrderCommand {
        order_id: id.into_inner(),
        user_id: user.user_id,
    };

    let shipped_order = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Order {} shipped by user {}", id.into_inner(), user.user_id);
    Ok(HttpResponse::Ok().json(shipped_order))
}

async fn apply_discount(
    db_pool: web::Data<Arc<DbPool>>,
    event_sender: web::Data<Arc<EventSender>>,
    order_id: web::Path<i32>,
    discount_info: web::Json<ApplyOrderDiscountCommand>,
) -> Result<HttpResponse, ServiceError> {
    discount_info.validate()?;

    let command = ApplyOrderDiscountCommand {
        order_id: order_id.into_inner(),
        discount_amount: discount_info.discount_amount,
    };

    let result = command.execute(db_pool.get_ref().clone(), event_sender.get_ref().clone()).await?;
    info!("Discount applied to order {}: amount={}", order_id.into_inner(), discount_info.discount_amount);
    Ok(HttpResponse::Ok().json(result))
}

pub fn configure_routes() -> Scope {
    web::scope("/orders")
        .route("", web::post().to(create_order))
        .route("/{id}", web::get().to(get_order))
        .route("/{id}/items", web::put().to(update_order_items))
        .route("/{id}", web::delete().to(delete_order))
        .route("", web::get().to(list_orders))
        .route("/search", web::get().to(search_orders))
        .route("/{id}/items", web::post().to(add_item_to_order))
        .route("/{order_id}/items/{item_id}", web::delete().to(remove_item_from_order))
        .route("/{id}/partial_cancel", web::post().to(partial_cancel_order))
        .route("/{id}/cancel", web::post().to(cancel_order))
        .route("/{id}/ship", web::post().to(ship_order))
        .route("/{id}/apply_discount", web::put().to(apply_discount))
}
