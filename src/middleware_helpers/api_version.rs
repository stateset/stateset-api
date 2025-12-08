//! API Versioning Middleware
//!
//! This module provides API versioning support including:
//! - Version negotiation via headers and URL paths
//! - Deprecation warnings for older API versions
//! - Version-specific feature flags

use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::{info, warn};

/// Current API version
pub const CURRENT_API_VERSION: &str = "1.0.0";

/// Minimum supported API version
pub const MIN_SUPPORTED_VERSION: &str = "1.0.0";

/// API version header names
pub const API_VERSION_HEADER: &str = "x-api-version";
pub const API_VERSION_RESPONSE_HEADER: &str = "x-api-version";
pub const API_DEPRECATION_HEADER: &str = "x-api-deprecation";
pub const API_SUNSET_HEADER: &str = "sunset";

/// Parsed API version
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn current() -> Self {
        Self::from_str(CURRENT_API_VERSION).unwrap_or(Self::new(1, 0, 0))
    }

    pub fn minimum_supported() -> Self {
        Self::from_str(MIN_SUPPORTED_VERSION).unwrap_or(Self::new(1, 0, 0))
    }

    pub fn is_supported(&self) -> bool {
        *self >= Self::minimum_supported()
    }

    pub fn is_deprecated(&self) -> bool {
        // Versions older than current major version are deprecated
        self.major < Self::current().major
    }
}

impl std::fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for ApiVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.trim_start_matches('v').split('.').collect();

        match parts.as_slice() {
            [major] => Ok(Self::new(
                major.parse().map_err(|_| "Invalid major version")?,
                0,
                0,
            )),
            [major, minor] => Ok(Self::new(
                major.parse().map_err(|_| "Invalid major version")?,
                minor.parse().map_err(|_| "Invalid minor version")?,
                0,
            )),
            [major, minor, patch] => Ok(Self::new(
                major.parse().map_err(|_| "Invalid major version")?,
                minor.parse().map_err(|_| "Invalid minor version")?,
                patch.parse().map_err(|_| "Invalid patch version")?,
            )),
            _ => Err("Invalid version format".to_string()),
        }
    }
}

/// Extract API version from request
fn extract_api_version(request: &Request) -> ApiVersion {
    // Try to get version from header first
    if let Some(version_header) = request.headers().get(API_VERSION_HEADER) {
        if let Ok(version_str) = version_header.to_str() {
            if let Ok(version) = ApiVersion::from_str(version_str) {
                return version;
            }
        }
    }

    // Try to extract from URL path (e.g., /api/v1/...)
    let path = request.uri().path();
    if let Some(version_segment) = path.split('/').find(|s| s.starts_with('v')) {
        if let Ok(version) = ApiVersion::from_str(version_segment) {
            return version;
        }
    }

    // Default to current version
    ApiVersion::current()
}

/// API versioning middleware
pub async fn api_version_middleware(request: Request, next: Next) -> Response {
    let requested_version = extract_api_version(&request);

    // Check if version is supported
    if !requested_version.is_supported() {
        warn!(
            "Unsupported API version requested: {}",
            requested_version
        );
        return (
            StatusCode::GONE,
            format!(
                "API version {} is no longer supported. Minimum supported version is {}",
                requested_version,
                ApiVersion::minimum_supported()
            ),
        )
            .into_response();
    }

    // Process the request
    let mut response = next.run(request).await;

    // Add version headers to response
    let headers = response.headers_mut();

    // Current API version
    if let Ok(value) = HeaderValue::from_str(&ApiVersion::current().to_string()) {
        headers.insert(
            HeaderName::from_static(API_VERSION_RESPONSE_HEADER),
            value,
        );
    }

    // Add deprecation warning if using old version
    if requested_version.is_deprecated() {
        warn!(
            "Deprecated API version in use: {}",
            requested_version
        );
        if let Ok(value) = HeaderValue::from_str(&format!(
            "API version {} is deprecated. Please upgrade to version {}",
            requested_version,
            ApiVersion::current()
        )) {
            headers.insert(
                HeaderName::from_static(API_DEPRECATION_HEADER),
                value,
            );
        }
    }

    response
}

/// Get available API versions
pub fn available_versions() -> Vec<ApiVersion> {
    vec![
        ApiVersion::new(1, 0, 0),
    ]
}

/// API version info for documentation
#[derive(Debug, Serialize)]
pub struct ApiVersionInfo {
    pub current_version: String,
    pub minimum_supported: String,
    pub available_versions: Vec<String>,
    pub deprecation_policy: String,
}

impl ApiVersionInfo {
    pub fn new() -> Self {
        Self {
            current_version: ApiVersion::current().to_string(),
            minimum_supported: ApiVersion::minimum_supported().to_string(),
            available_versions: available_versions()
                .iter()
                .map(|v| v.to_string())
                .collect(),
            deprecation_policy: "Major versions are supported for 12 months after a new major version is released. Use the X-API-Deprecation header to check for deprecation notices.".to_string(),
        }
    }
}

impl Default for ApiVersionInfo {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        assert_eq!(
            ApiVersion::from_str("1.0.0").unwrap(),
            ApiVersion::new(1, 0, 0)
        );
        assert_eq!(
            ApiVersion::from_str("v1.2.3").unwrap(),
            ApiVersion::new(1, 2, 3)
        );
        assert_eq!(
            ApiVersion::from_str("2").unwrap(),
            ApiVersion::new(2, 0, 0)
        );
        assert_eq!(
            ApiVersion::from_str("1.5").unwrap(),
            ApiVersion::new(1, 5, 0)
        );
    }

    #[test]
    fn test_version_comparison() {
        assert!(ApiVersion::new(2, 0, 0) > ApiVersion::new(1, 0, 0));
        assert!(ApiVersion::new(1, 1, 0) > ApiVersion::new(1, 0, 0));
        assert!(ApiVersion::new(1, 0, 1) > ApiVersion::new(1, 0, 0));
    }

    #[test]
    fn test_version_support() {
        let current = ApiVersion::current();
        assert!(current.is_supported());

        let old = ApiVersion::new(0, 1, 0);
        assert!(!old.is_supported());
    }

    #[test]
    fn test_version_display() {
        assert_eq!(ApiVersion::new(1, 2, 3).to_string(), "1.2.3");
    }
}
