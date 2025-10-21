use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde_json::Value;
use tracing::warn;

/// Maximum allowed request body size (10MB)
const MAX_BODY_SIZE: usize = 10 * 1024 * 1024;

/// Middleware to sanitize and validate input
pub async fn sanitize_middleware(request: Request, next: Next) -> Result<Response, Response> {
    // Check content length
    if let Some(content_length) = request.headers().get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                if length > MAX_BODY_SIZE {
                    warn!("Request body too large: {} bytes", length);
                    return Err(
                        (StatusCode::PAYLOAD_TOO_LARGE, "Request body too large").into_response()
                    );
                }
            }
        }
    }

    // Check for suspicious headers
    if let Some(user_agent) = request.headers().get("user-agent") {
        if let Ok(ua) = user_agent.to_str() {
            if is_suspicious_user_agent(ua) {
                warn!("Suspicious user agent detected: {}", ua);
                // Log but don't block - could be legitimate
            }
        }
    }

    Ok(next.run(request).await)
}

/// Check if user agent appears suspicious
fn is_suspicious_user_agent(ua: &str) -> bool {
    let suspicious_patterns = ["sqlmap", "nikto", "nmap", "masscan", "metasploit"];

    let ua_lower = ua.to_lowercase();
    suspicious_patterns
        .iter()
        .any(|pattern| ua_lower.contains(pattern))
}

/// Sanitize a JSON value by removing potentially dangerous content
pub fn sanitize_json(value: &mut Value) {
    match value {
        Value::String(s) => {
            *s = sanitize_string(s);
        }
        Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                sanitize_json(v);
            }
        }
        Value::Array(arr) => {
            for v in arr.iter_mut() {
                sanitize_json(v);
            }
        }
        _ => {}
    }
}

/// Sanitize a string by removing dangerous characters and patterns
pub fn sanitize_string(input: &str) -> String {
    // Remove null bytes
    let mut sanitized = input.replace('\0', "");

    // Limit string length to prevent DoS
    const MAX_STRING_LENGTH: usize = 10000;
    if sanitized.len() > MAX_STRING_LENGTH {
        sanitized.truncate(MAX_STRING_LENGTH);
    }

    // Remove potential XSS patterns (basic - for more complete XSS prevention use a proper library)
    sanitized = sanitized
        .replace("<script", "&lt;script")
        .replace("</script", "&lt;/script")
        .replace("javascript:", "")
        .replace("onerror=", "")
        .replace("onclick=", "")
        .replace("onload=", "");

    sanitized
}

/// Validate and sanitize SQL identifiers (table names, column names)
pub fn validate_sql_identifier(identifier: &str) -> Result<String, String> {
    // Check length
    if identifier.is_empty() || identifier.len() > 64 {
        return Err("Invalid identifier length".to_string());
    }

    // Check characters - only allow alphanumeric and underscore
    if !identifier.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("Invalid characters in identifier".to_string());
    }

    // Don't allow identifiers that start with numbers
    if identifier.chars().next().map_or(false, |c| c.is_numeric()) {
        return Err("Identifier cannot start with a number".to_string());
    }

    // Check against SQL keywords (basic list)
    let sql_keywords = [
        "select", "insert", "update", "delete", "drop", "create", "alter", "table", "database",
        "union", "join", "where", "from", "order", "group", "having", "limit",
    ];

    if sql_keywords.contains(&identifier.to_lowercase().as_str()) {
        return Err("Identifier is a reserved SQL keyword".to_string());
    }

    Ok(identifier.to_string())
}

/// Validate email format
pub fn validate_email(email: &str) -> bool {
    // Basic email validation
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local = parts[0];
    let domain = parts[1];

    // Check local part
    if local.is_empty() || local.len() > 64 {
        return false;
    }

    // Check domain
    if domain.is_empty() || domain.len() > 255 {
        return false;
    }

    // Check for valid domain structure
    domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

/// Validate UUID format
pub fn validate_uuid(uuid_str: &str) -> bool {
    uuid::Uuid::parse_str(uuid_str).is_ok()
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_string() {
        assert_eq!(sanitize_string("normal text"), "normal text");
        assert_eq!(sanitize_string("text\0with\0nulls"), "textwithnulls");
        assert_eq!(
            sanitize_string("<script>alert('xss')</script>"),
            "&lt;script>alert('xss')&lt;/script>"
        );
    }

    #[test]
    fn test_validate_sql_identifier() {
        assert!(validate_sql_identifier("valid_table_name").is_ok());
        assert!(validate_sql_identifier("table123").is_ok());
        assert!(validate_sql_identifier("").is_err());
        assert!(validate_sql_identifier("123table").is_err());
        assert!(validate_sql_identifier("table-name").is_err());
        assert!(validate_sql_identifier("select").is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("user@example.com"));
        assert!(validate_email("user.name@example.co.uk"));
        assert!(!validate_email("invalid"));
        assert!(!validate_email("@example.com"));
        assert!(!validate_email("user@"));
        assert!(!validate_email("user@.com"));
    }
}
