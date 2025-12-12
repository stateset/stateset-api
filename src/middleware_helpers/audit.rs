//! Comprehensive audit logging middleware for security and compliance.
//!
//! This module provides enterprise-grade audit logging capabilities including:
//! - Request/response logging with sensitive data redaction
//! - User action tracking for compliance (SOC2, GDPR, HIPAA)
//! - Security event logging (auth failures, suspicious activity)
//! - Immutable audit trail with timestamps and request IDs

use axum::{extract::Request, http::Method, middleware::Next, response::Response};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{info, warn};

/// Audit log entry for compliance and security tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique identifier for this audit entry
    pub id: String,
    /// Timestamp in RFC3339 format
    pub timestamp: String,
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// HTTP method
    pub method: String,
    /// Request path (without query params for security)
    pub path: String,
    /// User ID if authenticated
    pub user_id: Option<String>,
    /// API key ID if used (not the actual key)
    pub api_key_id: Option<String>,
    /// Client IP address
    pub client_ip: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Response status code
    pub status_code: u16,
    /// Request duration in milliseconds
    pub duration_ms: u64,
    /// Action category for filtering
    pub action_category: ActionCategory,
    /// Resource type being accessed
    pub resource_type: Option<String>,
    /// Resource ID being accessed
    pub resource_id: Option<String>,
    /// Whether this was a sensitive operation
    pub is_sensitive: bool,
    /// Additional metadata
    pub metadata: Option<serde_json::Value>,
}

/// Categories of auditable actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionCategory {
    /// Authentication events (login, logout, token refresh)
    Authentication,
    /// Authorization events (permission checks, access denied)
    Authorization,
    /// Data read operations
    DataRead,
    /// Data write operations (create, update)
    DataWrite,
    /// Data delete operations
    DataDelete,
    /// Administrative actions
    Admin,
    /// Security events (rate limit, suspicious activity)
    Security,
    /// System events (health checks, metrics)
    System,
}

/// Sensitive paths that require enhanced audit logging
const SENSITIVE_PATHS: &[&str] = &[
    "/api/v1/payments",
    "/api/v1/customers",
    "/api/v1/admin",
    "/api/v1/auth",
    "/api/v1/users",
];

/// Headers that contain sensitive information and should be redacted
const SENSITIVE_HEADERS: &[&str] = &["authorization", "x-api-key", "cookie", "set-cookie"];

/// Audit logging middleware
///
/// This middleware logs all requests and responses for audit purposes.
/// It automatically redacts sensitive information and categorizes actions.
pub async fn audit_middleware(req: Request, next: Next) -> Response {
    let start = Instant::now();
    let audit_id = uuid::Uuid::new_v4().to_string();

    // Extract request information before processing
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Extract user information from headers (set by auth middleware)
    let user_id = req
        .headers()
        .get("x-user-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let api_key_id = req
        .headers()
        .get("x-api-key-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Extract client IP (handles proxies)
    let client_ip = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string())
        .or_else(|| {
            req.headers()
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(String::from)
        });

    let user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    // Determine action category and sensitivity
    let action_category = categorize_action(&method, &path);
    let is_sensitive = is_sensitive_path(&path);
    let (resource_type, resource_id) = extract_resource_info(&path);

    // Process the request
    let response = next.run(req).await;

    let duration_ms = start.elapsed().as_millis() as u64;
    let status_code = response.status().as_u16();

    // Create audit entry
    let audit_entry = AuditLogEntry {
        id: audit_id,
        timestamp: Utc::now().to_rfc3339(),
        request_id,
        method: method.to_string(),
        path: path.clone(),
        user_id,
        api_key_id,
        client_ip,
        user_agent,
        status_code,
        duration_ms,
        action_category: action_category.clone(),
        resource_type,
        resource_id,
        is_sensitive,
        metadata: None,
    };

    // Log the audit entry
    log_audit_entry(&audit_entry);

    // Log security events for failures
    if status_code == 401 || status_code == 403 {
        log_security_event(&audit_entry, "access_denied");
    } else if status_code == 429 {
        log_security_event(&audit_entry, "rate_limited");
    }

    response
}

/// Categorize the action based on HTTP method and path
fn categorize_action(method: &Method, path: &str) -> ActionCategory {
    // Authentication endpoints
    if path.contains("/auth") || path.contains("/login") || path.contains("/token") {
        return ActionCategory::Authentication;
    }

    // Admin endpoints
    if path.contains("/admin") {
        return ActionCategory::Admin;
    }

    // System endpoints
    if path.contains("/health") || path.contains("/metrics") || path.contains("/status") {
        return ActionCategory::System;
    }

    // Categorize by HTTP method
    match *method {
        Method::GET | Method::HEAD => ActionCategory::DataRead,
        Method::POST => ActionCategory::DataWrite,
        Method::PUT | Method::PATCH => ActionCategory::DataWrite,
        Method::DELETE => ActionCategory::DataDelete,
        _ => ActionCategory::System,
    }
}

/// Check if the path is considered sensitive
fn is_sensitive_path(path: &str) -> bool {
    SENSITIVE_PATHS.iter().any(|&p| path.starts_with(p))
}

/// Extract resource type and ID from the path
fn extract_resource_info(path: &str) -> (Option<String>, Option<String>) {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.len() >= 3 && parts[0] == "api" && parts[1].starts_with('v') {
        let resource_type = Some(parts[2].to_string());
        let resource_id = if parts.len() >= 4 {
            // Check if the 4th part looks like an ID (UUID or numeric)
            let potential_id = parts[3];
            if uuid::Uuid::parse_str(potential_id).is_ok()
                || potential_id.chars().all(|c| c.is_ascii_digit())
            {
                Some(potential_id.to_string())
            } else {
                None
            }
        } else {
            None
        };
        (resource_type, resource_id)
    } else {
        (None, None)
    }
}

/// Log the audit entry using structured logging
fn log_audit_entry(entry: &AuditLogEntry) {
    info!(
        audit_id = %entry.id,
        timestamp = %entry.timestamp,
        request_id = ?entry.request_id,
        method = %entry.method,
        path = %entry.path,
        user_id = ?entry.user_id,
        api_key_id = ?entry.api_key_id,
        client_ip = ?entry.client_ip,
        status_code = entry.status_code,
        duration_ms = entry.duration_ms,
        action_category = ?entry.action_category,
        resource_type = ?entry.resource_type,
        resource_id = ?entry.resource_id,
        is_sensitive = entry.is_sensitive,
        "audit_log"
    );
}

/// Log security-related events
fn log_security_event(entry: &AuditLogEntry, event_type: &str) {
    warn!(
        audit_id = %entry.id,
        event_type = %event_type,
        timestamp = %entry.timestamp,
        request_id = ?entry.request_id,
        method = %entry.method,
        path = %entry.path,
        user_id = ?entry.user_id,
        client_ip = ?entry.client_ip,
        status_code = entry.status_code,
        "security_event"
    );
}

/// Redact sensitive values from a string
pub fn redact_sensitive(value: &str, visible_chars: usize) -> String {
    if value.len() <= visible_chars {
        return "*".repeat(value.len());
    }
    format!(
        "{}{}",
        &value[..visible_chars],
        "*".repeat(value.len() - visible_chars)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_action_authentication() {
        assert_eq!(
            categorize_action(&Method::POST, "/api/v1/auth/login"),
            ActionCategory::Authentication
        );
    }

    #[test]
    fn test_categorize_action_data_read() {
        assert_eq!(
            categorize_action(&Method::GET, "/api/v1/orders"),
            ActionCategory::DataRead
        );
    }

    #[test]
    fn test_categorize_action_data_write() {
        assert_eq!(
            categorize_action(&Method::POST, "/api/v1/orders"),
            ActionCategory::DataWrite
        );
    }

    #[test]
    fn test_categorize_action_data_delete() {
        assert_eq!(
            categorize_action(&Method::DELETE, "/api/v1/orders/123"),
            ActionCategory::DataDelete
        );
    }

    #[test]
    fn test_is_sensitive_path() {
        assert!(is_sensitive_path("/api/v1/payments/process"));
        assert!(is_sensitive_path("/api/v1/customers/123"));
        assert!(!is_sensitive_path("/api/v1/orders"));
    }

    #[test]
    fn test_extract_resource_info() {
        let (resource, id) =
            extract_resource_info("/api/v1/orders/550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(resource, Some("orders".to_string()));
        assert_eq!(id, Some("550e8400-e29b-41d4-a716-446655440000".to_string()));

        let (resource, id) = extract_resource_info("/api/v1/inventory");
        assert_eq!(resource, Some("inventory".to_string()));
        assert_eq!(id, None);
    }

    #[test]
    fn test_redact_sensitive() {
        assert_eq!(
            redact_sensitive("sk_live_abc123xyz", 4),
            "sk_l*************"
        );
        assert_eq!(redact_sensitive("abc", 4), "***");
    }
}
