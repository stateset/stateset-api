use crate::errors::{ApiError, ServiceError};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;
use validator::Validate;

/// Standard success response
pub fn success_response<T: Serialize>(data: T) -> Response {
    (StatusCode::OK, Json(data)).into_response()
}

/// Standard created response
pub fn created_response<T: Serialize>(data: T) -> Response {
    (StatusCode::CREATED, Json(data)).into_response()
}

/// Standard no content response
pub fn no_content_response() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Validate request input
pub fn validate_input<T: Validate>(input: &T) -> Result<(), ApiError> {
    input
        .validate()
        .map_err(|e| ApiError::ValidationError(format!("Validation failed: {}", e)))
}

/// Map service errors to API errors
pub fn map_service_error(err: ServiceError) -> ApiError {
    ApiError::ServiceError(err)
}

/// Pagination parameters for list operations
#[derive(Debug, Deserialize, Serialize, IntoParams)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

impl PaginationParams {
    /// Calculate zero-based offset for pagination
    pub fn offset(&self) -> u64 {
        self.page.saturating_sub(1) * self.per_page
    }
}

/// Standard pagination response metadata
#[derive(Debug, Serialize)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
}

impl PaginationMeta {
    pub fn new(page: u64, per_page: u64, total: u64) -> Self {
        let total_pages = if total == 0 {
            0
        } else {
            (total + per_page - 1) / per_page
        };
        Self {
            page,
            per_page,
            total,
            total_pages,
        }
    }
}

/// Standard paginated response wrapper
#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub pagination: PaginationMeta,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, page: u64, per_page: u64, total: u64) -> Self {
        Self {
            data,
            pagination: PaginationMeta::new(page, per_page, total),
        }
    }
}
