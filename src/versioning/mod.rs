/*!
 * # API Versioning Module
 *
 * This module provides API versioning capabilities for the Stateset API. It
 * allows routing requests to different API versions based on:
 *
 * - URL path prefix (e.g., `/api/v1/`, `/api/v2/`)
 * - Accept header with version (e.g., `Accept: application/vnd.stateset.v1+json`)
 * - Custom version header (e.g., `X-API-Version: 1`)
 *
 * The module supports:
 * - Routing to the appropriate API version handler
 * - Deprecation warnings for older API versions
 * - Automatic documentation of API versions
 * - Graceful handling of unsupported versions
 */

use async_trait::async_trait;
use axum::{
    http::{
        header::{HeaderValue, ACCEPT},
        HeaderMap, Request, StatusCode,
    },
    middleware::Next,
    response::{IntoResponse, Response},
    routing::Router,
};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use tracing::{debug, info, warn};

/// API version identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ApiVersion {
    /// Version 1 - Initial API version
    V1,
    /// Version 2 - Future API version
    V2,
}

impl ApiVersion {
    /// Gets the version as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::V1 => "v1",
            Self::V2 => "v2",
        }
    }

    /// Gets the numeric version
    pub fn as_number(&self) -> u8 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }

    /// Checks if this version is supported
    pub fn is_supported(&self) -> bool {
        match self {
            Self::V1 => true,
            Self::V2 => false, // V2 not implemented yet
        }
    }

    /// Checks if this version is deprecated
    pub fn is_deprecated(&self) -> bool {
        match self {
            Self::V1 => false,
            Self::V2 => false,
        }
    }

    /// Gets the latest supported version
    pub fn latest() -> Self {
        Self::V1 // Update this when new versions are stable
    }
}

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Default for ApiVersion {
    fn default() -> Self {
        Self::latest()
    }
}

/// Error for invalid API versions
#[derive(Debug, thiserror::Error)]
pub enum ApiVersionError {
    #[error("Unsupported API version: {0}")]
    UnsupportedVersion(String),

    #[error("Invalid API version format: {0}")]
    InvalidFormat(String),
}

impl TryFrom<&str> for ApiVersion {
    type Error = ApiVersionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.trim().to_lowercase().as_str() {
            "v1" | "1" => Ok(Self::V1),
            "v2" | "2" => Ok(Self::V2),
            _ => Err(ApiVersionError::InvalidFormat(value.to_string())),
        }
    }
}

impl IntoResponse for ApiVersionError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::UnsupportedVersion(_) => StatusCode::NOT_FOUND,
            Self::InvalidFormat(_) => StatusCode::BAD_REQUEST,
        };

        (status, self.to_string()).into_response()
    }
}

/// Extractor for API version from request
// TODO: Fix lifetime issues with FromRequestParts implementation
// Commenting out for now to get basic compilation working
/*
#[async_trait]
impl<S> FromRequestParts<S> for ApiVersion
where
    S: Send + Sync,
{
    type Rejection = ApiVersionError;

    async fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut axum::http::request::Parts,
        _state: &'life1 S,
    ) -> core::pin::Pin<Box<dyn core::future::Future<Output = Result<Self, Self::Rejection>> + core::marker::Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async move {
        // 1. Try to extract from URL path
        if let Some(version) = extract_version_from_path(&parts.uri.path()) {
            return Ok(version);
        }

        // 2. Try to extract from Accept header
        if let Some(accept) = parts.headers.get(ACCEPT) {
            if let Ok(accept_str) = accept.to_str() {
                if let Some(version) = extract_version_from_accept(accept_str) {
                    return Ok(version);
                }
            }
        }

        // 3. Try to extract from custom version header
        if let Some(version_header) = parts.headers.get("X-API-Version") {
            if let Ok(version_str) = version_header.to_str() {
                return ApiVersion::try_from(version_str);
            }
        }

        // 4. Default to latest version
        Ok(ApiVersion::latest())
        })
    }
}
*/

/// Extract version from URL path (e.g., /api/v1/...)
fn extract_version_from_path(path: &str) -> Option<ApiVersion> {
    // Split path into segments
    let segments: Vec<&str> = path.split('/').collect();

    // Look for 'api' followed by version
    for i in 0..segments.len().saturating_sub(1) {
        if segments[i] == "api" {
            if let Ok(version) = ApiVersion::try_from(segments[i + 1]) {
                return Some(version);
            }
        }
    }

    None
}

/// Extract version from Accept header value
fn extract_version_from_accept(accept: &str) -> Option<ApiVersion> {
    // Look for pattern: application/vnd.stateset.v1+json
    if accept.contains("application/vnd.stateset.") {
        let parts: Vec<&str> = accept.split('.').collect();

        for part in parts {
            if part.starts_with('v') && part.len() > 1 {
                let version_part = if part.contains('+') {
                    part.split('+').next().unwrap_or(part)
                } else {
                    part
                };

                if let Ok(version) = ApiVersion::try_from(version_part) {
                    return Some(version);
                }
            }
        }
    }

    None
}

/// Middleware to handle API versioning
pub async fn api_version_middleware(req: Request<axum::body::Body>, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let version = extract_version_from_request(&req);

    debug!("API request: path={}, version={:?}", path, version);

    // Check if version is supported
    if let Some(ref v) = version {
        if !v.is_supported() {
            warn!("Unsupported API version requested: {}", v);
            return (
                StatusCode::NOT_FOUND,
                format!(
                    "API version {} is not supported. Please use one of the supported versions.",
                    v
                ),
            )
                .into_response();
        }

        // Add deprecation warning header if needed
        if v.is_deprecated() {
            warn!("Deprecated API version requested: {}", v);
            let mut response = next.run(req).await;

            response.headers_mut().insert(
                "Warning",
                HeaderValue::from_str(&format!(
                    "299 - \"API v{} is deprecated and will be removed. Please migrate to v{}\"",
                    v.as_number(),
                    ApiVersion::latest().as_number()
                ))
                .unwrap_or(HeaderValue::from_static("299 - deprecated")),
            );

            response
                .headers_mut()
                .insert("X-API-Deprecated", HeaderValue::from_static("true"));

            return response;
        }
    }

    // Add version info to response headers
    let mut response = next.run(req).await;

    if let Some(v) = version {
        response.headers_mut().insert(
            "X-API-Version",
            HeaderValue::from_str(&v.to_string()).unwrap_or(HeaderValue::from_static("latest")),
        );
    }

    response
}

/// Extract API version from request
pub fn extract_version_from_request(req: &Request<axum::body::Body>) -> Option<ApiVersion> {
    // 1. Try path version
    let path = req.uri().path();
    if let Some(version) = extract_version_from_path(path) {
        return Some(version);
    }

    // 2. Try Accept header
    if let Some(accept) = req.headers().get(ACCEPT) {
        if let Ok(accept_str) = accept.to_str() {
            if let Some(version) = extract_version_from_accept(accept_str) {
                return Some(version);
            }
        }
    }

    // 3. Try version header
    if let Some(version_header) = req.headers().get("X-API-Version") {
        if let Ok(version_str) = version_header.to_str() {
            if let Ok(version) = ApiVersion::try_from(version_str) {
                return Some(version);
            }
        }
    }

    // Default to latest
    Some(ApiVersion::latest())
}

/// Available API versions information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiVersionInfo {
    pub version: String,
    pub status: ApiVersionStatus,
    pub documentation_url: String,
    pub release_date: String,
    pub end_of_life: Option<String>,
}

/// API version status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiVersionStatus {
    Alpha,
    Beta,
    Stable,
    Deprecated,
    Retired,
}

/// Get information about available API versions
pub fn get_api_versions() -> Vec<ApiVersionInfo> {
    vec![
        ApiVersionInfo {
            version: "v1".to_string(),
            status: ApiVersionStatus::Stable,
            documentation_url: "/docs/v1".to_string(),
            release_date: "2023-01-01".to_string(),
            end_of_life: None,
        },
        ApiVersionInfo {
            version: "v2".to_string(),
            status: ApiVersionStatus::Alpha,
            documentation_url: "/docs/v2".to_string(),
            release_date: "2024-06-01".to_string(),
            end_of_life: None,
        },
    ]
}

/// API versions endpoint handler
pub async fn versions_handler() -> impl IntoResponse {
    let versions = get_api_versions();
    (StatusCode::OK, axum::Json(versions))
}

/// Create router with API versioning information
pub fn api_versions_routes() -> Router {
    Router::new().route("/", axum::routing::get(versions_handler))
}
