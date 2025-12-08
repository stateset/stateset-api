/*!
 * # API Versioning Module
 *
 * This module provides comprehensive API versioning support including:
 * - Header-based versioning
 * - URL path versioning
 * - Content negotiation
 * - Version compatibility checking
 * - Deprecation warnings
 * - Version migration helpers
 */

use axum::{
    extract::{Request, State},
    http::{header, HeaderMap, StatusCode, Version},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use regex::Regex;
use once_cell::sync::Lazy;

#[derive(Error, Debug)]
pub enum VersioningError {
    #[error("Unsupported API version: {version}")]
    UnsupportedVersion { version: String },
    
    #[error("API version deprecated: {version}")]
    DeprecatedVersion { version: String },
    
    #[error("Version negotiation failed")]
    NegotiationFailed,
    
    #[error("Invalid version format: {version}")]
    InvalidFormat { version: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ApiVersion {
    V1,
    V2,
    V3,
}

impl ApiVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiVersion::V1 => "v1",
            ApiVersion::V2 => "v2",
            ApiVersion::V3 => "v3",
        }
    }
    
    pub fn from_string(s: &str) -> Result<Self, VersioningError> {
        match s.to_lowercase().as_str() {
            "v1" | "1" | "1.0" => Ok(ApiVersion::V1),
            "v2" | "2" | "2.0" => Ok(ApiVersion::V2),
            "v3" | "3" | "3.0" => Ok(ApiVersion::V3),
            _ => Err(VersioningError::UnsupportedVersion { version: s.to_string() }),
        }
    }
    
    pub fn to_header_value(&self) -> String {
        format!("application/vnd.stateset.{}", self.as_str())
    }
    
    pub fn is_deprecated(&self) -> bool {
        matches!(self, ApiVersion::V1)
    }
    
    pub fn sunset_date(&self) -> Option<&'static str> {
        match self {
            ApiVersion::V1 => Some("2024-12-31"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct VersionConfig {
    pub default_version: ApiVersion,
    pub supported_versions: Vec<ApiVersion>,
    pub deprecated_versions: Vec<ApiVersion>,
    pub version_header: String,
    pub accept_header_pattern: String,
}

impl Default for VersionConfig {
    fn default() -> Self {
        Self {
            default_version: ApiVersion::V2,
            supported_versions: vec![ApiVersion::V1, ApiVersion::V2, ApiVersion::V3],
            deprecated_versions: vec![ApiVersion::V1],
            version_header: "X-API-Version".to_string(),
            accept_header_pattern: r"application/vnd\.stateset\.(\w+)".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    pub status: VersionStatus,
    pub deprecated: bool,
    pub sunset_date: Option<String>,
    pub changelog_url: Option<String>,
    pub migration_guide: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VersionStatus {
    Active,
    Deprecated,
    Sunset,
}

pub struct ApiVersioningService {
    config: VersionConfig,
    version_info: HashMap<ApiVersion, VersionInfo>,
}

impl ApiVersioningService {
    pub fn new(config: VersionConfig) -> Self {
        let mut version_info = HashMap::new();
        
        for version in &config.supported_versions {
            let info = VersionInfo {
                version: version.as_str().to_string(),
                status: if config.deprecated_versions.contains(version) {
                    VersionStatus::Deprecated
                } else {
                    VersionStatus::Active
                },
                deprecated: config.deprecated_versions.contains(version),
                sunset_date: version.sunset_date().map(|s| s.to_string()),
                changelog_url: Some(format!("https://stateset.io/changelog/{}", version.as_str())),
                migration_guide: if matches!(version, ApiVersion::V1) {
                    Some("https://stateset.io/docs/migration/v1-to-v2".to_string())
                } else {
                    None
                },
            };
            version_info.insert(version.clone(), info);
        }
        
        Self {
            config,
            version_info,
        }
    }
    
    pub fn default() -> Self {
        Self::new(VersionConfig::default())
    }
    
    /// Extract version from request
    pub fn extract_version(&self, headers: &HeaderMap) -> Result<ApiVersion, VersioningError> {
        // Try X-API-Version header first
        if let Some(version_header) = headers.get(&self.config.version_header) {
            if let Ok(version_str) = version_header.to_str() {
                return ApiVersion::from_string(version_str);
            }
        }
        
        // Try Accept header content negotiation
        if let Some(accept_header) = headers.get(header::ACCEPT) {
            if let Ok(accept_str) = accept_header.to_str() {
                static ACCEPT_REGEX: Lazy<Regex> = Lazy::new(|| {
                    Regex::new(r"application/vnd\.stateset\.(\w+)").unwrap()
                });
                
                if let Some(captures) = ACCEPT_REGEX.captures(accept_str) {
                    if let Some(version_match) = captures.get(1) {
                        return ApiVersion::from_string(version_match.as_str());
                    }
                }
            }
        }
        
        // Fall back to default version
        Ok(self.config.default_version.clone())
    }
    
    /// Negotiate version based on client capabilities
    pub fn negotiate_version(&self, client_versions: &[ApiVersion]) -> Result<ApiVersion, VersioningError> {
        for client_version in client_versions {
            if self.config.supported_versions.contains(client_version) {
                return Ok(client_version.clone());
            }
        }
        
        Err(VersioningError::NegotiationFailed)
    }
    
    /// Check if version is supported
    pub fn is_supported(&self, version: &ApiVersion) -> bool {
        self.config.supported_versions.contains(version)
    }
    
    /// Check if version is deprecated
    pub fn is_deprecated(&self, version: &ApiVersion) -> bool {
        self.config.deprecated_versions.contains(version)
    }
    
    /// Get version information
    pub fn get_version_info(&self, version: &ApiVersion) -> Option<&VersionInfo> {
        self.version_info.get(version)
    }
    
    /// Get all supported versions
    pub fn get_supported_versions(&self) -> Vec<ApiVersion> {
        self.config.supported_versions.clone()
    }
    
    /// Get version compatibility matrix
    pub fn get_compatibility_matrix(&self) -> HashMap<String, Vec<String>> {
        let mut matrix = HashMap::new();
        
        for version in &self.config.supported_versions {
            let compatible = match version {
                ApiVersion::V1 => vec!["v1".to_string()],
                ApiVersion::V2 => vec!["v1".to_string(), "v2".to_string()],
                ApiVersion::V3 => vec!["v2".to_string(), "v3".to_string()],
            };
            matrix.insert(version.as_str().to_string(), compatible);
        }
        
        matrix
    }
}

/// Versioning middleware
pub mod middleware {
    use super::*;
    use axum::response::Response;
    use std::sync::Arc;
    
    pub async fn versioning_middleware(
        State(versioning_service): State<Arc<ApiVersioningService>>,
        mut request: Request,
        next: Next,
    ) -> Response {
        // Extract version from request
        let version = match versioning_service.extract_version(request.headers()) {
            Ok(v) => v,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
                    "error": "Version negotiation failed",
                    "message": e.to_string(),
                    "supported_versions": versioning_service.get_supported_versions()
                        .iter()
                        .map(|v| v.as_str())
                        .collect::<Vec<_>>()
                }))).into_response();
            }
        };
        
        // Check if version is supported
        if !versioning_service.is_supported(&version) {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({
                "error": "Unsupported API version",
                "version": version.as_str(),
                "supported_versions": versioning_service.get_supported_versions()
                    .iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<_>>()
            }))).into_response();
        }
        
        // Add version information to request extensions
        request.extensions_mut().insert(version.clone());
        
        // Add version headers to response
        let mut response = next.run(request).await;
        
        // Add version headers
        let headers = response.headers_mut();
        if let Ok(version_value) = version.as_str().parse() {
            headers.insert("X-API-Version", version_value);
        }
        if let Ok(content_type_value) = version.to_header_value().parse() {
            headers.insert(header::CONTENT_TYPE, content_type_value);
        }

        // Add deprecation warning if needed
        if versioning_service.is_deprecated(&version) {
            if let Some(sunset_date) = version.sunset_date() {
                if let Ok(deprecated_value) = format!("true; sunset={}", sunset_date).parse() {
                    headers.insert("X-API-Deprecated", deprecated_value);
                }
                if let Ok(warning_value) = format!(
                    "299 stateset \"API version {} is deprecated and will be sunset on {}\"",
                    version.as_str(), sunset_date
                ).parse() {
                    headers.insert("Warning", warning_value);
                }
            }
        }
        
        response
    }
}

/// Version information endpoints
pub mod handlers {
    use super::*;
    use axum::{extract::Path, Json};
    
    /// Get information about all API versions
    pub async fn list_versions(
        State(versioning_service): State<Arc<ApiVersioningService>>,
    ) -> Json<serde_json::Value> {
        let versions: Vec<serde_json::Value> = versioning_service.get_supported_versions()
            .iter()
            .filter_map(|v| versioning_service.get_version_info(v))
            .filter_map(|info| serde_json::to_value(info).ok())
            .collect();

        Json(serde_json::json!({
            "versions": versions,
            "default_version": versioning_service.config.default_version.as_str(),
            "current_version": versioning_service.config.default_version.as_str()
        }))
    }

    /// Get information about a specific version
    pub async fn get_version(
        State(versioning_service): State<Arc<ApiVersioningService>>,
        Path(version_str): Path<String>,
    ) -> Result<Json<serde_json::Value>, VersioningError> {
        let version = ApiVersion::from_string(&version_str)?;

        if let Some(info) = versioning_service.get_version_info(&version) {
            serde_json::to_value(info)
                .map(Json)
                .map_err(|_| VersioningError::UnsupportedVersion { version: version_str })
        } else {
            Err(VersioningError::UnsupportedVersion { version: version_str })
        }
    }
    
    /// Get version compatibility matrix
    pub async fn get_compatibility_matrix(
        State(versioning_service): State<Arc<ApiVersioningService>>,
    ) -> Json<serde_json::Value> {
        let matrix = versioning_service.get_compatibility_matrix();
        Json(serde_json::json!({
            "compatibility_matrix": matrix,
            "note": "Lists which older versions are compatible with newer versions"
        }))
    }
}

/// Version migration helpers
pub mod migration {
    use super::*;
    
    /// Migrate data from one version to another
    pub trait VersionMigrator {
        fn migrate_up(&self, data: serde_json::Value, from_version: &ApiVersion, to_version: &ApiVersion) -> Result<serde_json::Value, VersioningError>;
        fn migrate_down(&self, data: serde_json::Value, from_version: &ApiVersion, to_version: &ApiVersion) -> Result<serde_json::Value, VersioningError>;
    }
    
    /// Default migrator for common transformations
    pub struct DefaultMigrator;
    
    impl VersionMigrator for DefaultMigrator {
        fn migrate_up(&self, data: serde_json::Value, from_version: &ApiVersion, to_version: &ApiVersion) -> Result<serde_json::Value, VersioningError> {
            match (from_version, to_version) {
                (ApiVersion::V1, ApiVersion::V2) => self.migrate_v1_to_v2(data),
                (ApiVersion::V2, ApiVersion::V3) => self.migrate_v2_to_v3(data),
                _ => Ok(data), // No migration needed
            }
        }
        
        fn migrate_down(&self, data: serde_json::Value, from_version: &ApiVersion, to_version: &ApiVersion) -> Result<serde_json::Value, VersioningError> {
            match (from_version, to_version) {
                (ApiVersion::V2, ApiVersion::V1) => self.migrate_v2_to_v1(data),
                (ApiVersion::V3, ApiVersion::V2) => self.migrate_v3_to_v2(data),
                _ => Ok(data), // No migration needed
            }
        }
    }
    
    impl DefaultMigrator {
        fn migrate_v1_to_v2(&self, mut data: serde_json::Value) -> Result<serde_json::Value, VersioningError> {
            // Example: Add new required fields with defaults
            if let Some(obj) = data.as_object_mut() {
                obj.insert("created_at".to_string(), serde_json::json!("2023-01-01T00:00:00Z"));
                obj.insert("updated_at".to_string(), serde_json::json!("2023-01-01T00:00:00Z"));
            }
            Ok(data)
        }
        
        fn migrate_v2_to_v3(&self, mut data: serde_json::Value) -> Result<serde_json::Value, VersioningError> {
            // Example: Rename fields or change structure
            if let Some(obj) = data.as_object_mut() {
                // Rename 'name' to 'display_name'
                if let Some(name) = obj.remove("name") {
                    obj.insert("display_name".to_string(), name);
                }
            }
            Ok(data)
        }
        
        fn migrate_v2_to_v1(&self, mut data: serde_json::Value) -> Result<serde_json::Value, VersioningError> {
            // Reverse migration
            if let Some(obj) = data.as_object_mut() {
                obj.remove("created_at");
                obj.remove("updated_at");
            }
            Ok(data)
        }
        
        fn migrate_v3_to_v2(&self, mut data: serde_json::Value) -> Result<serde_json::Value, VersioningError> {
            // Reverse migration
            if let Some(obj) = data.as_object_mut() {
                if let Some(display_name) = obj.remove("display_name") {
                    obj.insert("name".to_string(), display_name);
                }
            }
            Ok(data)
        }
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use axum::http::HeaderMap;
    
    #[test]
    fn test_version_parsing() {
        assert_eq!(ApiVersion::from_string("v1").unwrap(), ApiVersion::V1);
        assert_eq!(ApiVersion::from_string("v2").unwrap(), ApiVersion::V2);
        assert_eq!(ApiVersion::from_string("1").unwrap(), ApiVersion::V1);
        assert_eq!(ApiVersion::from_string("2.0").unwrap(), ApiVersion::V2);
        
        assert!(ApiVersion::from_string("v4").is_err());
    }
    
    #[test]
    fn test_version_extraction_from_headers() {
        let service = ApiVersioningService::default();
        
        // Test X-API-Version header
        let mut headers = HeaderMap::new();
        headers.insert("X-API-Version", "v2".parse().unwrap());
        
        let version = service.extract_version(&headers).unwrap();
        assert_eq!(version, ApiVersion::V2);
        
        // Test Accept header
        let mut headers = HeaderMap::new();
        headers.insert("Accept", "application/vnd.stateset.v1+json".parse().unwrap());
        
        let version = service.extract_version(&headers).unwrap();
        assert_eq!(version, ApiVersion::V1);
        
        // Test default fallback
        let headers = HeaderMap::new();
        let version = service.extract_version(&headers).unwrap();
        assert_eq!(version, ApiVersion::V2); // Default version
    }
    
    #[test]
    fn test_version_deprecation() {
        assert!(ApiVersion::V1.is_deprecated());
        assert!(!ApiVersion::V2.is_deprecated());
        assert!(!ApiVersion::V3.is_deprecated());
    }
    
    #[tokio::test]
    async fn test_version_info() {
        let service = ApiVersioningService::default();
        
        let v1_info = service.get_version_info(&ApiVersion::V1).unwrap();
        assert_eq!(v1_info.version, "v1");
        assert!(v1_info.deprecated);
        assert!(v1_info.sunset_date.is_some());
        
        let v2_info = service.get_version_info(&ApiVersion::V2).unwrap();
        assert_eq!(v2_info.version, "v2");
        assert!(!v2_info.deprecated);
        assert!(v2_info.sunset_date.is_none());
    }
}
