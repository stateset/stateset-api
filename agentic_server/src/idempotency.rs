use axum::{
    body::Body,
    extract::{Request, State},
    http::{header::CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use crate::{constants::MAX_REQUEST_BODY_BYTES, redis_store::RedisStore};

/// Idempotency record stored in Redis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyRecord {
    pub request_hash: String,
    pub status_code: u16,
    pub response_body_base64: String,
    pub content_type: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Idempotency service
#[derive(Clone)]
pub struct IdempotencyService {
    redis: Arc<RedisStore>,
}

impl IdempotencyService {
    pub fn new(redis: Arc<RedisStore>) -> Self {
        Self { redis }
    }

    /// Compute hash of request body
    fn compute_request_hash(body: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(body);
        hex::encode(hasher.finalize())
    }

    /// Check if request has been seen before
    pub async fn check_idempotency(
        &self,
        idempotency_key: &str,
        request_body: &[u8],
    ) -> Result<Option<IdempotencyRecord>, crate::errors::ServiceError> {
        let key = format!("idempotency:{}", idempotency_key);

        // Try to get existing record
        let existing: Option<IdempotencyRecord> = self.redis.get(&key).await?;

        if let Some(record) = existing {
            let request_hash = Self::compute_request_hash(request_body);

            // Check if request body matches
            if record.request_hash != request_hash {
                // Same idempotency key, different request = conflict
                return Err(crate::errors::ServiceError::InvalidOperation(
                    "Idempotency key reused with different request".to_string(),
                ));
            }

            debug!("Idempotency key found: {}", idempotency_key);
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Store response for future idempotent requests
    pub async fn store_response(
        &self,
        idempotency_key: &str,
        request_body: &[u8],
        status_code: u16,
        response_body: &[u8],
        content_type: Option<String>,
    ) -> Result<(), crate::errors::ServiceError> {
        let key = format!("idempotency:{}", idempotency_key);

        let record = IdempotencyRecord {
            request_hash: Self::compute_request_hash(request_body),
            status_code,
            response_body_base64: general_purpose::STANDARD.encode(response_body),
            content_type,
            created_at: chrono::Utc::now(),
        };

        // Store for 24 hours
        self.redis
            .set(&key, &record, Some(Duration::from_secs(86400)))
            .await?;

        debug!("Stored idempotency record: {}", idempotency_key);
        Ok(())
    }
}

/// Idempotency middleware (only for mutating operations)
pub async fn idempotency_middleware(
    State(service): State<Option<Arc<IdempotencyService>>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // If idempotency is disabled or no key provided, pass through
    let Some(service) = service else {
        return next.run(request).await;
    };

    let idempotency_key = match headers.get("Idempotency-Key") {
        Some(key) => key.to_str().unwrap_or_default().to_string(),
        None => return next.run(request).await,
    };

    // Only apply to POST requests (mutating operations)
    if request.method() != "POST" {
        return next.run(request).await;
    }

    // Extract body
    let (parts, body) = request.into_parts();
    let body_bytes = match axum::body::to_bytes(body, MAX_REQUEST_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!("Failed to read request body: {}", e);
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                axum::Json(serde_json::json!({
                    "type": "invalid_request",
                    "code": "payload_too_large",
                    "message": format!("Request body exceeds {} bytes", MAX_REQUEST_BODY_BYTES)
                })),
            )
                .into_response();
        }
    };

    // Check for existing idempotent response
    match service
        .check_idempotency(&idempotency_key, &body_bytes)
        .await
    {
        Ok(Some(record)) => {
            // Return cached response
            debug!(
                "Returning cached idempotent response for: {}",
                idempotency_key
            );

            let body_bytes = match general_purpose::STANDARD.decode(&record.response_body_base64) {
                Ok(bytes) => bytes,
                Err(err) => {
                    warn!(
                        "Failed to decode stored idempotent response for {}: {}",
                        idempotency_key, err
                    );
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(serde_json::json!({
                            "type": "processing_error",
                            "code": "idempotency_replay_failed",
                            "message": "Failed to replay stored response"
                        })),
                    )
                        .into_response();
                }
            };

            let mut builder = Response::builder()
                .status(StatusCode::from_u16(record.status_code).unwrap_or(StatusCode::OK));

            if let Some(headers) = builder.headers_mut() {
                headers.insert(
                    HeaderName::from_static("x-idempotent-replay"),
                    HeaderValue::from_static("true"),
                );

                let content_type_value =
                    record.content_type.as_deref().unwrap_or("application/json");
                if let Ok(value) = HeaderValue::from_str(content_type_value) {
                    headers.insert(CONTENT_TYPE, value);
                }

                if let Ok(value) = HeaderValue::from_str(&idempotency_key) {
                    headers.insert(HeaderName::from_static("idempotency-key"), value);
                }
            }

            let response = builder.body(Body::from(body_bytes)).unwrap();
            return response.into_response();
        }
        Ok(None) => {
            // New request, process it
            let request = Request::from_parts(parts, Body::from(body_bytes.clone()));
            let response = next.run(request).await;

            let (mut response_parts, response_body) = response.into_parts();
            let response_bytes =
                match axum::body::to_bytes(response_body, MAX_REQUEST_BODY_BYTES).await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        warn!(
                            "Failed to buffer response for idempotent key {}: {}",
                            idempotency_key, e
                        );
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json(serde_json::json!({
                                "type": "processing_error",
                                "code": "idempotency_store_failed",
                                "message": "Failed to persist response for idempotent request"
                            })),
                        )
                            .into_response();
                    }
                };

            let content_type = response_parts
                .headers
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(|s| s.to_string());

            if response_parts.status.as_u16() < 500 {
                if let Err(err) = service
                    .store_response(
                        &idempotency_key,
                        &body_bytes,
                        response_parts.status.as_u16(),
                        &response_bytes,
                        content_type.clone(),
                    )
                    .await
                {
                    warn!(
                        "Failed to store idempotent response for {}: {}",
                        idempotency_key, err
                    );
                }
            }

            if let Ok(value) = HeaderValue::from_str(&idempotency_key) {
                response_parts
                    .headers
                    .insert(HeaderName::from_static("idempotency-key"), value);
            }

            Response::from_parts(response_parts, Body::from(response_bytes))
        }
        Err(e) => {
            // Idempotency conflict
            warn!("Idempotency conflict: {}", e);
            (
                StatusCode::CONFLICT,
                axum::Json(serde_json::json!({
                    "type": "request_not_idempotent",
                    "code": "idempotency_conflict",
                    "message": "Idempotency key reused with different request parameters"
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_hash() {
        let body1 = b"test data";
        let body2 = b"test data";
        let body3 = b"different data";

        let hash1 = IdempotencyService::compute_request_hash(body1);
        let hash2 = IdempotencyService::compute_request_hash(body2);
        let hash3 = IdempotencyService::compute_request_hash(body3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
