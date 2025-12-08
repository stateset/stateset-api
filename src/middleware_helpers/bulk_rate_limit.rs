//! Bulk Operation Rate Limiting
//!
//! This module provides specialized rate limiting for bulk operations
//! that have higher resource costs than regular API requests.

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

use crate::rate_limiter::{RateLimitBackend, RateLimitConfig, RateLimiter};

/// Configuration for bulk operation rate limits
#[derive(Debug, Clone)]
pub struct BulkRateLimitConfig {
    /// Maximum items per bulk request
    pub max_items_per_request: usize,
    /// Maximum bulk requests per window
    pub max_requests_per_window: u32,
    /// Time window for rate limiting
    pub window_duration: Duration,
    /// Cost multiplier per item (for weighted rate limiting)
    pub cost_per_item: u32,
}

impl Default for BulkRateLimitConfig {
    fn default() -> Self {
        Self {
            max_items_per_request: 100,
            max_requests_per_window: 10,
            window_duration: Duration::from_secs(60),
            cost_per_item: 1,
        }
    }
}

/// Predefined configurations for different bulk operation types
impl BulkRateLimitConfig {
    /// Configuration for inventory bulk adjustments
    pub fn inventory_bulk() -> Self {
        Self {
            max_items_per_request: 500,
            max_requests_per_window: 20,
            window_duration: Duration::from_secs(60),
            cost_per_item: 1,
        }
    }

    /// Configuration for order bulk operations
    pub fn order_bulk() -> Self {
        Self {
            max_items_per_request: 100,
            max_requests_per_window: 10,
            window_duration: Duration::from_secs(60),
            cost_per_item: 2,
        }
    }

    /// Configuration for payment bulk operations (more restrictive)
    pub fn payment_bulk() -> Self {
        Self {
            max_items_per_request: 50,
            max_requests_per_window: 5,
            window_duration: Duration::from_secs(60),
            cost_per_item: 5,
        }
    }

    /// Configuration for data export operations
    pub fn export_bulk() -> Self {
        Self {
            max_items_per_request: 10000,
            max_requests_per_window: 5,
            window_duration: Duration::from_secs(300), // 5 minutes
            cost_per_item: 1,
        }
    }
}

/// Bulk operation rate limiter
#[derive(Clone)]
pub struct BulkRateLimiter {
    limiter: RateLimiter,
    config: BulkRateLimitConfig,
}

impl BulkRateLimiter {
    pub fn new(config: BulkRateLimitConfig, backend: RateLimitBackend) -> Self {
        let rate_config = RateLimitConfig {
            requests_per_window: config.max_requests_per_window,
            window_duration: config.window_duration,
            burst_limit: Some(config.max_requests_per_window * 2),
            enable_headers: true,
        };

        Self {
            limiter: RateLimiter::new(rate_config, backend),
            config,
        }
    }

    /// Check if a bulk operation is allowed
    pub async fn check_bulk_operation(
        &self,
        key: &str,
        item_count: usize,
    ) -> Result<BulkRateLimitResult, BulkRateLimitError> {
        // First check item count limit
        if item_count > self.config.max_items_per_request {
            return Err(BulkRateLimitError::TooManyItems {
                requested: item_count,
                max: self.config.max_items_per_request,
            });
        }

        // Check rate limit
        let result = self
            .limiter
            .check_rate_limit(&format!("bulk:{}", key))
            .await
            .map_err(|e| BulkRateLimitError::InternalError(e.to_string()))?;

        if !result.allowed {
            return Err(BulkRateLimitError::RateLimitExceeded {
                reset_seconds: result.reset_time.as_secs(),
            });
        }

        Ok(BulkRateLimitResult {
            allowed: true,
            remaining_requests: result.remaining,
            max_items: self.config.max_items_per_request,
            reset_seconds: result.reset_time.as_secs(),
        })
    }

    /// Get the maximum allowed items per request
    pub fn max_items(&self) -> usize {
        self.config.max_items_per_request
    }
}

#[derive(Debug, Serialize)]
pub struct BulkRateLimitResult {
    pub allowed: bool,
    pub remaining_requests: u32,
    pub max_items: usize,
    pub reset_seconds: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum BulkRateLimitError {
    #[error("Too many items in bulk request: {requested} (max: {max})")]
    TooManyItems { requested: usize, max: usize },

    #[error("Bulk rate limit exceeded. Try again in {reset_seconds} seconds")]
    RateLimitExceeded { reset_seconds: u64 },

    #[error("Internal rate limiter error: {0}")]
    InternalError(String),
}

#[derive(Serialize)]
struct BulkErrorResponse {
    error: String,
    error_code: String,
    details: BulkErrorDetails,
}

#[derive(Serialize)]
struct BulkErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    requested: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_allowed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_after_seconds: Option<u64>,
}

impl IntoResponse for BulkRateLimitError {
    fn into_response(self) -> Response {
        let (status, error_code, details) = match &self {
            BulkRateLimitError::TooManyItems { requested, max } => (
                StatusCode::BAD_REQUEST,
                "BULK_TOO_MANY_ITEMS",
                BulkErrorDetails {
                    requested: Some(*requested),
                    max_allowed: Some(*max),
                    retry_after_seconds: None,
                },
            ),
            BulkRateLimitError::RateLimitExceeded { reset_seconds } => (
                StatusCode::TOO_MANY_REQUESTS,
                "BULK_RATE_LIMIT_EXCEEDED",
                BulkErrorDetails {
                    requested: None,
                    max_allowed: None,
                    retry_after_seconds: Some(*reset_seconds),
                },
            ),
            BulkRateLimitError::InternalError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "BULK_RATE_LIMIT_ERROR",
                BulkErrorDetails {
                    requested: None,
                    max_allowed: None,
                    retry_after_seconds: None,
                },
            ),
        };

        let response = BulkErrorResponse {
            error: self.to_string(),
            error_code: error_code.to_string(),
            details,
        };

        (status, Json(response)).into_response()
    }
}

/// Paths that are considered bulk operations
const BULK_OPERATION_PATHS: &[(&str, &str)] = &[
    ("/api/v1/inventory/bulk-adjust", "inventory"),
    ("/api/v1/orders/bulk", "orders"),
    ("/api/v1/orders/bulk-update", "orders"),
    ("/api/v1/shipments/bulk", "shipments"),
    ("/api/v1/products/bulk", "products"),
    ("/api/v1/customers/bulk-import", "customers"),
    ("/api/v1/payments/bulk-capture", "payments"),
    ("/api/v1/analytics/export", "export"),
];

/// Get the bulk operation type for a path
pub fn get_bulk_operation_type(path: &str) -> Option<&'static str> {
    BULK_OPERATION_PATHS
        .iter()
        .find(|(p, _)| path.starts_with(p))
        .map(|(_, op_type)| *op_type)
}

/// Get the appropriate rate limit config for an operation type
pub fn get_bulk_config_for_type(op_type: &str) -> BulkRateLimitConfig {
    match op_type {
        "inventory" => BulkRateLimitConfig::inventory_bulk(),
        "orders" => BulkRateLimitConfig::order_bulk(),
        "payments" => BulkRateLimitConfig::payment_bulk(),
        "export" => BulkRateLimitConfig::export_bulk(),
        _ => BulkRateLimitConfig::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bulk_rate_limit_items() {
        let config = BulkRateLimitConfig {
            max_items_per_request: 10,
            max_requests_per_window: 100,
            window_duration: Duration::from_secs(60),
            cost_per_item: 1,
        };

        let limiter = BulkRateLimiter::new(config, RateLimitBackend::InMemory);

        // Should allow small batch
        let result = limiter.check_bulk_operation("test", 5).await;
        assert!(result.is_ok());

        // Should reject large batch
        let result = limiter.check_bulk_operation("test", 20).await;
        assert!(matches!(
            result,
            Err(BulkRateLimitError::TooManyItems { .. })
        ));
    }

    #[tokio::test]
    async fn test_bulk_rate_limit_requests() {
        let config = BulkRateLimitConfig {
            max_items_per_request: 100,
            max_requests_per_window: 2,
            window_duration: Duration::from_secs(60),
            cost_per_item: 1,
        };

        let limiter = BulkRateLimiter::new(config, RateLimitBackend::InMemory);

        // First two requests should succeed
        assert!(limiter.check_bulk_operation("test", 5).await.is_ok());
        assert!(limiter.check_bulk_operation("test", 5).await.is_ok());

        // Third request should be rate limited
        let result = limiter.check_bulk_operation("test", 5).await;
        assert!(matches!(
            result,
            Err(BulkRateLimitError::RateLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_get_bulk_operation_type() {
        assert_eq!(
            get_bulk_operation_type("/api/v1/inventory/bulk-adjust"),
            Some("inventory")
        );
        assert_eq!(
            get_bulk_operation_type("/api/v1/payments/bulk-capture"),
            Some("payments")
        );
        assert_eq!(get_bulk_operation_type("/api/v1/orders"), None);
    }
}
