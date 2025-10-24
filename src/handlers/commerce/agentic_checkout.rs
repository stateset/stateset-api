use crate::{
    errors::{ApiError, ServiceError},
    services::commerce::agentic_checkout::{
        CheckoutSession, CheckoutSessionCompleteRequest, CheckoutSessionCreateRequest,
        CheckoutSessionUpdateRequest, CheckoutSessionWithOrder,
    },
    AppState,
};
use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use tracing::error;

/// Creates the router for agentic checkout endpoints
pub fn agentic_checkout_routes() -> Router<AppState> {
    Router::new()
        .route("/checkout_sessions", post(create_checkout_session))
        .route(
            "/checkout_sessions/:checkout_session_id",
            get(get_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id",
            post(update_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id/complete",
            post(complete_checkout_session),
        )
        .route(
            "/checkout_sessions/:checkout_session_id/cancel",
            post(cancel_checkout_session),
        )
}

/// Create a checkout session
async fn create_checkout_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CheckoutSessionCreateRequest>,
) -> Result<Response, ApiError> {
    let idempotency_header = headers.get("Idempotency-Key").cloned();
    let idempotency_key = match idempotency_header.as_ref() {
        Some(value) => Some(
            value
                .to_str()
                .map_err(|_| ApiError::BadRequest {
                    message: "Idempotency-Key must be valid ASCII".to_string(),
                    error_code: Some("INVALID_REQUEST".to_string()),
                })?
                .to_owned(),
        ),
        None => None,
    };

    let create_result = state
        .services
        .agentic_checkout
        .create_session(payload, idempotency_key.as_deref())
        .await
        .map_err(map_service_error)?;
    let session = create_result.session;

    // Build response with headers
    let mut response = Response::builder()
        .status(if create_result.was_created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        })
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store");

    // Echo idempotency key if provided
    if let Some(idempotency_value) = idempotency_header {
        response = response.header("Idempotency-Key", idempotency_value);
    }

    // Echo request ID if provided
    if let Some(request_id) = headers.get("Request-Id") {
        response = response.header("Request-Id", request_id);
    }

    if create_result.was_created {
        response = response.header("Location", format!("/checkout_sessions/{}", session.id));
    }

    let body = serde_json::to_string(&session).map_err(|e| {
        ApiError::ServiceError(ServiceError::InternalError(format!(
            "Serialization error: {}",
            e
        )))
    })?;

    Ok(response.body(body.into()).map_err(|e| {
        ApiError::ServiceError(ServiceError::InternalError(format!(
            "Response build error: {}",
            e
        )))
    })?)
}

/// Get checkout session
async fn get_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
) -> Result<Json<CheckoutSession>, ApiError> {
    let session = state
        .services
        .agentic_checkout
        .get_session(&checkout_session_id)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            _ => map_service_error(e),
        })?;

    Ok(Json(session))
}

/// Update checkout session
async fn update_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    Json(payload): Json<CheckoutSessionUpdateRequest>,
) -> Result<Json<CheckoutSession>, ApiError> {
    let session = state
        .services
        .agentic_checkout
        .update_session(&checkout_session_id, payload)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            crate::errors::ServiceError::InvalidOperation(msg) => ApiError::BadRequest {
                message: msg,
                error_code: Some("INVALID_REQUEST".to_string()),
            },
            _ => map_service_error(e),
        })?;

    Ok(Json(session))
}

/// Complete checkout session
async fn complete_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
    Json(payload): Json<CheckoutSessionCompleteRequest>,
) -> Result<Json<CheckoutSessionWithOrder>, ApiError> {
    let result = state
        .services
        .agentic_checkout
        .complete_session(&checkout_session_id, payload)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            crate::errors::ServiceError::InvalidOperation(msg) => ApiError::BadRequest {
                message: msg,
                error_code: Some("INVALID_REQUEST".to_string()),
            },
            _ => map_service_error(e),
        })?;

    Ok(Json(result))
}

/// Cancel checkout session
async fn cancel_checkout_session(
    State(state): State<AppState>,
    Path(checkout_session_id): Path<String>,
) -> Result<Response, ApiError> {
    let session = state
        .services
        .agentic_checkout
        .cancel_session(&checkout_session_id)
        .await
        .map_err(|e| match e {
            crate::errors::ServiceError::NotFound(_) => ApiError::NotFound(format!(
                "Checkout session {} not found",
                checkout_session_id
            )),
            crate::errors::ServiceError::InvalidOperation(msg) => {
                // If already completed/canceled, return 405
                return ApiError::MethodNotAllowed { message: msg };
            }
            _ => map_service_error(e),
        })?;

    let body = serde_json::to_string(&session).map_err(|e| {
        ApiError::ServiceError(ServiceError::InternalError(format!(
            "Serialization error: {}",
            e
        )))
    })?;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store")
        .body(body.into())
        .map_err(|e| {
            ApiError::ServiceError(ServiceError::InternalError(format!(
                "Response build error: {}",
                e
            )))
        })?)
}

fn map_service_error(error: crate::errors::ServiceError) -> ApiError {
    error!("Service error: {:?}", error);
    match error {
        crate::errors::ServiceError::NotFound(msg) => ApiError::NotFound(msg),
        crate::errors::ServiceError::InvalidInput(msg) => ApiError::BadRequest {
            message: msg,
            error_code: Some("INVALID_REQUEST".to_string()),
        },
        crate::errors::ServiceError::InvalidOperation(msg) => ApiError::BadRequest {
            message: msg,
            error_code: Some("PROCESSING_ERROR".to_string()),
        },
        _ => ApiError::InternalServerError,
    }
}
