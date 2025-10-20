use crate::{
    errors::ServiceError,
    services::{
        stablepay_reconciliation_service::{ReconciliationRequest, StablePayReconciliationService},
        stablepay_service::{CreatePaymentRequest, CreateRefundRequest, StablePayService},
    },
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Shared state for StablePay handlers
#[derive(Clone)]
pub struct StablePayState {
    pub service: Arc<StablePayService>,
    pub reconciliation_service: Arc<StablePayReconciliationService>,
}

/// Query parameters for listing payments
#[derive(Debug, Deserialize)]
pub struct ListPaymentsQuery {
    #[serde(default = "default_limit")]
    pub limit: u64,
    #[serde(default)]
    pub offset: u64,
}

fn default_limit() -> u64 {
    20
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Create a payment
pub async fn create_payment(
    State(state): State<Arc<StablePayState>>,
    Json(request): Json<CreatePaymentRequest>,
) -> impl IntoResponse {
    info!("Creating payment: {:?}", request);

    match state.service.create_payment(request).await {
        Ok(payment) => {
            info!("Payment created successfully: {}", payment.id);
            (StatusCode::CREATED, Json(ApiResponse::success(payment))).into_response()
        }
        Err(e) => {
            error!("Failed to create payment: {:?}", e);
            handle_error(e)
        }
    }
}

/// Get a payment by ID
pub async fn get_payment(
    State(state): State<Arc<StablePayState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Getting payment: {}", id);

    match state.service.get_payment(id).await {
        Ok(payment) => {
            info!("Payment retrieved: {}", payment.id);
            (StatusCode::OK, Json(ApiResponse::success(payment))).into_response()
        }
        Err(e) => {
            error!("Failed to get payment: {:?}", e);
            handle_error(e)
        }
    }
}

/// List payments for a customer
pub async fn list_customer_payments(
    State(state): State<Arc<StablePayState>>,
    Path(customer_id): Path<Uuid>,
    Query(query): Query<ListPaymentsQuery>,
) -> impl IntoResponse {
    info!(
        "Listing payments for customer: {} (limit: {}, offset: {})",
        customer_id, query.limit, query.offset
    );

    match state
        .service
        .list_customer_payments(customer_id, query.limit, query.offset)
        .await
    {
        Ok(payments) => {
            info!("Retrieved {} payments", payments.len());
            (StatusCode::OK, Json(ApiResponse::success(payments))).into_response()
        }
        Err(e) => {
            error!("Failed to list payments: {:?}", e);
            handle_error(e)
        }
    }
}

/// Create a refund
pub async fn create_refund(
    State(state): State<Arc<StablePayState>>,
    Json(request): Json<CreateRefundRequest>,
) -> impl IntoResponse {
    info!("Creating refund: {:?}", request);

    match state.service.create_refund(request).await {
        Ok(refund) => {
            info!("Refund created successfully: {}", refund.id);
            (StatusCode::CREATED, Json(ApiResponse::success(refund))).into_response()
        }
        Err(e) => {
            error!("Failed to create refund: {:?}", e);
            handle_error(e)
        }
    }
}

/// Run reconciliation
pub async fn run_reconciliation(
    State(state): State<Arc<StablePayState>>,
    Json(request): Json<ReconciliationRequest>,
) -> impl IntoResponse {
    info!("Running reconciliation: {:?}", request);

    match state.reconciliation_service.reconcile(request).await {
        Ok(result) => {
            info!("Reconciliation completed: {}", result.id);
            (StatusCode::CREATED, Json(ApiResponse::success(result))).into_response()
        }
        Err(e) => {
            error!("Failed to run reconciliation: {:?}", e);
            handle_error(e)
        }
    }
}

/// Get reconciliation by ID
pub async fn get_reconciliation(
    State(state): State<Arc<StablePayState>>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Getting reconciliation: {}", id);

    match state.reconciliation_service.get_reconciliation(id).await {
        Ok(reconciliation) => {
            info!("Reconciliation retrieved: {}", reconciliation.id);
            (StatusCode::OK, Json(ApiResponse::success(reconciliation))).into_response()
        }
        Err(e) => {
            error!("Failed to get reconciliation: {:?}", e);
            handle_error(e)
        }
    }
}

/// List reconciliations for a provider
pub async fn list_reconciliations(
    State(state): State<Arc<StablePayState>>,
    Path(provider_id): Path<Uuid>,
    Query(query): Query<ListPaymentsQuery>,
) -> impl IntoResponse {
    info!(
        "Listing reconciliations for provider: {} (limit: {}, offset: {})",
        provider_id, query.limit, query.offset
    );

    match state
        .reconciliation_service
        .list_reconciliations(provider_id, query.limit, query.offset)
        .await
    {
        Ok(reconciliations) => {
            info!("Retrieved {} reconciliations", reconciliations.len());
            (StatusCode::OK, Json(ApiResponse::success(reconciliations))).into_response()
        }
        Err(e) => {
            error!("Failed to list reconciliations: {:?}", e);
            handle_error(e)
        }
    }
}

/// Get reconciliation stats
pub async fn get_reconciliation_stats(
    State(state): State<Arc<StablePayState>>,
    Path(provider_id): Path<Uuid>,
    Query(query): Query<ReconciliationStatsQuery>,
) -> impl IntoResponse {
    info!(
        "Getting reconciliation stats for provider: {} (days: {})",
        provider_id, query.days
    );

    match state
        .reconciliation_service
        .get_reconciliation_stats(provider_id, query.days)
        .await
    {
        Ok(stats) => {
            info!("Retrieved reconciliation stats");
            (StatusCode::OK, Json(ApiResponse::success(stats))).into_response()
        }
        Err(e) => {
            error!("Failed to get reconciliation stats: {:?}", e);
            handle_error(e)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReconciliationStatsQuery {
    #[serde(default = "default_days")]
    pub days: i64,
}

fn default_days() -> i64 {
    30
}

/// Health check endpoint for StablePay
pub async fn health_check() -> impl IntoResponse {
    #[derive(Serialize)]
    struct Health {
        status: String,
        service: String,
    }

    Json(Health {
        status: "healthy".to_string(),
        service: "StablePay".to_string(),
    })
}

fn handle_error(error: ServiceError) -> axum::response::Response {
    let (status, message) = match error {
        ServiceError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        ServiceError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
        ServiceError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
        ServiceError::ExternalApiError(msg) => (StatusCode::BAD_GATEWAY, msg),
        ServiceError::db_error(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        ),
    };

    (status, Json(ApiResponse::<()>::error(message))).into_response()
}

/// Register StablePay routes
pub fn stablepay_routes() -> axum::Router<Arc<StablePayState>> {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/health", get(health_check))
        .route("/payments", post(create_payment))
        .route("/payments/:id", get(get_payment))
        .route(
            "/customers/:customer_id/payments",
            get(list_customer_payments),
        )
        .route("/refunds", post(create_refund))
        .route("/reconciliations", post(run_reconciliation))
        .route("/reconciliations/:id", get(get_reconciliation))
        .route(
            "/providers/:provider_id/reconciliations",
            get(list_reconciliations),
        )
        .route(
            "/providers/:provider_id/reconciliation-stats",
            get(get_reconciliation_stats),
        )
}
