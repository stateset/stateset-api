use super::common::PaginationParams;
use crate::auth::AuthenticatedUser;
use crate::errors::ServiceError;
use crate::handlers::AppState;
use crate::services::payments::{
    PaymentMethod, PaymentService, PaymentStatus, ProcessPaymentRequest, RefundPaymentRequest,
};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreatePaymentRequest {
    pub order_id: Uuid,

    pub amount: Decimal,
    pub payment_method: String,
    pub payment_method_id: Option<String>,
    pub currency: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Validate, ToSchema)]
pub struct RefundPaymentHandlerRequest {
    pub payment_id: Uuid,

    pub amount: Option<Decimal>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct PaymentStatusFilter {
    pub status: Option<String>,
}

// Handler functions

/// Process a payment for an order
#[utoipa::path(
    post,
    path = "/api/v1/payments",
    request_body = CreatePaymentRequest,
    responses(
        (status = 201, description = "Payment processed", body = crate::services::payments::PaymentResponse),
        (status = 400, description = "Bad request", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "Payments"
)]
async fn process_payment(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<CreatePaymentRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    // Check permissions
    if !user.has_permission("payments:write") {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions".to_string(),
        ));
    }

    request.validate()?;

    // Parse payment method
    let payment_method = match request.payment_method.to_lowercase().as_str() {
        "credit_card" => PaymentMethod::CreditCard,
        "debit_card" => PaymentMethod::DebitCard,
        "paypal" => PaymentMethod::PayPal,
        "bank_transfer" => PaymentMethod::BankTransfer,
        "cash" => PaymentMethod::Cash,
        "check" => PaymentMethod::Check,
        _ => {
            return Err(ServiceError::ValidationError(
                "Invalid payment method".to_string(),
            ))
        }
    };

    let payment_request = ProcessPaymentRequest {
        order_id: request.order_id,
        amount: request.amount,
        payment_method,
        payment_method_id: request.payment_method_id,
        currency: request.currency.unwrap_or_else(|| "USD".to_string()),
        description: request.description,
    };

    let payment_service =
        PaymentService::new(state.db.clone(), Arc::new(state.event_sender.clone()));

    let response = payment_service.process_payment(payment_request).await?;
    Ok((StatusCode::CREATED, Json(response)))
}

/// Get payment by ID
#[utoipa::path(
    get,
    path = "/api/v1/payments/{payment_id}",
    params(
        ("payment_id" = Uuid, Path, description = "Payment ID")
    ),
    responses(
        (status = 200, description = "Payment details", body = crate::services::payments::PaymentResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "Payments"
)]
async fn get_payment(
    State(state): State<AppState>,
    Path(payment_id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    // Check permissions
    if !user.has_permission("payments:read") {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions".to_string(),
        ));
    }

    let payment_service =
        PaymentService::new(state.db.clone(), Arc::new(state.event_sender.clone()));

    let payment = payment_service.get_payment(payment_id).await?;
    Ok(Json(payment))
}

/// Get payments for an order
#[utoipa::path(
    get,
    path = "/api/v1/payments/order/{order_id}",
    params(
        ("order_id" = Uuid, Path, description = "Order ID")
    ),
    responses(
        (status = 200, description = "Payments for order", body = [crate::services::payments::PaymentResponse])
    ),
    security(("bearer_auth" = [])),
    tag = "Payments"
)]
async fn get_order_payments(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    // Check permissions
    if !user.has_permission("payments:read") {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions".to_string(),
        ));
    }

    let payment_service =
        PaymentService::new(state.db.clone(), Arc::new(state.event_sender.clone()));

    let payments = payment_service.get_order_payments(order_id).await?;
    Ok(Json(payments))
}

/// List payments with pagination and filtering
#[utoipa::path(
    get,
    path = "/api/v1/payments",
    params(
        PaginationParams,
        PaymentStatusFilter
    ),
    responses(
        (status = 200, description = "List payments", body = crate::PaginatedResponse<crate::services::payments::PaymentResponse>)
    ),
    security(("bearer_auth" = [])),
    tag = "Payments"
)]
async fn list_payments(
    State(state): State<AppState>,
    Query(params): Query<PaginationParams>,
    Query(filter): Query<PaymentStatusFilter>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    // Check permissions
    if !user.has_permission("payments:read") {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions".to_string(),
        ));
    }

    let page = params.page;
    let limit = params.per_page;

    let status_filter = filter.status.map(|s| match s.to_lowercase().as_str() {
        "pending" => PaymentStatus::Pending,
        "processing" => PaymentStatus::Processing,
        "succeeded" => PaymentStatus::Succeeded,
        "failed" => PaymentStatus::Failed,
        "cancelled" => PaymentStatus::Cancelled,
        "refunded" => PaymentStatus::Refunded,
        _ => PaymentStatus::Pending,
    });

    let payment_service =
        PaymentService::new(state.db.clone(), Arc::new(state.event_sender.clone()));

    let (payments, total) = payment_service
        .list_payments(page, limit, status_filter)
        .await?;

    let response = crate::PaginatedResponse {
        items: payments,
        total,
        page,
        limit,
        total_pages: (total + limit - 1) / limit,
    };

    Ok(Json(response))
}

/// Refund a payment
#[utoipa::path(
    post,
    path = "/api/v1/payments/refund",
    request_body = RefundPaymentHandlerRequest,
    responses(
        (status = 201, description = "Refund processed", body = crate::services::payments::PaymentResponse),
        (status = 400, description = "Bad request", body = crate::errors::ErrorResponse)
    ),
    security(("bearer_auth" = [])),
    tag = "Payments"
)]
async fn refund_payment(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Json(request): Json<RefundPaymentHandlerRequest>,
) -> Result<impl IntoResponse, ServiceError> {
    // Check permissions
    if !user.has_permission("payments:write") {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions".to_string(),
        ));
    }

    request.validate()?;

    let refund_request = RefundPaymentRequest {
        payment_id: request.payment_id,
        amount: request.amount,
        reason: request.reason,
    };

    let payment_service =
        PaymentService::new(state.db.clone(), Arc::new(state.event_sender.clone()));

    let refund = payment_service.refund_payment(refund_request).await?;
    Ok((StatusCode::CREATED, Json(refund)))
}

/// Get total payments for an order
#[utoipa::path(
    get,
    path = "/api/v1/payments/order/{order_id}/total",
    params(
        ("order_id" = Uuid, Path, description = "Order ID")
    ),
    responses(
        (status = 200, description = "Order total paid", body = serde_json::Value)
    ),
    security(("bearer_auth" = [])),
    tag = "Payments"
)]
async fn get_order_payment_total(
    State(state): State<AppState>,
    Path(order_id): Path<Uuid>,
    user: AuthenticatedUser,
) -> Result<impl IntoResponse, ServiceError> {
    // Check permissions
    if !user.has_permission("payments:read") {
        return Err(ServiceError::Forbidden(
            "Insufficient permissions".to_string(),
        ));
    }

    let payment_service =
        PaymentService::new(state.db.clone(), Arc::new(state.event_sender.clone()));

    let total = payment_service.get_order_total_payments(order_id).await?;

    let response = serde_json::json!({
        "order_id": order_id,
        "total_paid": total
    });

    Ok(Json(response))
}

/// Payment routes
pub fn payment_routes() -> Router<AppState> {
    Router::new()
        .route("/", post(process_payment))
        .route("/", get(list_payments))
        .route("/{payment_id}", get(get_payment))
        .route("/order/{order_id}", get(get_order_payments))
        .route("/order/{order_id}/total", get(get_order_payment_total))
        .route("/refund", post(refund_payment))
}
