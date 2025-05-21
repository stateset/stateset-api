pub mod orders;
pub mod inventory;
pub mod returns;
pub mod shipments;
pub mod warranties;
pub mod work_orders;
pub mod bom;
pub mod suppliers;
pub mod purchase_orders;
pub mod customers;
pub mod reports;
pub mod auth;
pub mod users;
pub mod notifications;
pub mod asn;

// Re-export common utility functions for handlers
pub mod common {
    use axum::{
        extract::{Query, Json, Path, State},
        http::StatusCode,
        response::{IntoResponse, Response},
    };
    use serde::{Serialize, Deserialize};
    use validator::Validate;
    use crate::errors::{ServiceError, ApiError};
    use std::sync::Arc;
    use crate::AppState;

    /// Pagination parameters for list endpoints
    #[derive(Debug, Deserialize)]
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

    impl PaginationParams {
        pub fn offset(&self) -> u64 {
            (self.page - 1) * self.per_page
        }
    }

    /// Common validation function for handler inputs
    pub fn validate_input<T: Validate>(input: &T) -> Result<(), ApiError> {
        input.validate().map_err(|err| {
            ApiError::BadRequest(format!("Validation error: {}", err))
        })
    }

    /// Convert ServiceError to ApiError for responses
    pub fn map_service_error(err: ServiceError) -> ApiError {
        ApiError::from(err)
    }

    /// Utility for creating a standard success response
    pub fn success_response<T: Serialize>(data: T) -> impl IntoResponse {
        (StatusCode::OK, Json(data))
    }

    /// Utility for creating a standard created response
    pub fn created_response<T: Serialize>(data: T) -> impl IntoResponse {
        (StatusCode::CREATED, Json(data))
    }

    /// Utility for creating a standard no content response
    pub fn no_content_response() -> impl IntoResponse {
        StatusCode::NO_CONTENT
    }
}