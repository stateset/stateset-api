use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, warn};

/// API Key store (in production, use database)
#[derive(Clone)]
pub struct ApiKeyStore {
    keys: Arc<RwLock<HashMap<String, ApiKeyInfo>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    pub key: String,
    pub merchant_id: String,
    pub name: String,
    pub is_active: bool,
    pub rate_limit: Option<u32>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl ApiKeyStore {
    pub fn new() -> Self {
        let mut keys = HashMap::new();
        
        // Add default test keys
        keys.insert(
            "api_key_demo_123".to_string(),
            ApiKeyInfo {
                key: "api_key_demo_123".to_string(),
                merchant_id: "merchant_001".to_string(),
                name: "Demo Merchant".to_string(),
                is_active: true,
                rate_limit: Some(100),
                created_at: chrono::Utc::now(),
            },
        );
        
        keys.insert(
            "psp_api_key_456".to_string(),
            ApiKeyInfo {
                key: "psp_api_key_456".to_string(),
                merchant_id: "psp_001".to_string(),
                name: "PSP Demo".to_string(),
                is_active: true,
                rate_limit: Some(500),
                created_at: chrono::Utc::now(),
            },
        );
        
        Self {
            keys: Arc::new(RwLock::new(keys)),
        }
    }
    
    pub fn validate(&self, api_key: &str) -> Option<ApiKeyInfo> {
        let keys = self.keys.read().unwrap();
        keys.get(api_key).filter(|info| info.is_active).cloned()
    }
    
    pub fn add_key(&self, info: ApiKeyInfo) {
        let mut keys = self.keys.write().unwrap();
        keys.insert(info.key.clone(), info);
    }
    
    pub fn revoke_key(&self, api_key: &str) -> bool {
        let mut keys = self.keys.write().unwrap();
        if let Some(info) = keys.get_mut(api_key) {
            info.is_active = false;
            true
        } else {
            false
        }
    }
}

impl Default for ApiKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract and validate API key from Authorization header
pub fn extract_api_key(headers: &HeaderMap) -> Result<String, StatusCode> {
    let auth_header = headers
        .get("Authorization")
        .ok_or(StatusCode::UNAUTHORIZED)?
        .to_str()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    
    let api_key = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    Ok(api_key.to_string())
}

/// Authentication middleware
pub async fn auth_middleware(
    State(key_store): State<ApiKeyStore>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    // Extract API key
    let api_key = match extract_api_key(&headers) {
        Ok(key) => key,
        Err(status) => {
            warn!("Missing or invalid Authorization header");
            return (
                status,
                axum::Json(serde_json::json!({
                    "type": "invalid_request",
                    "code": "unauthorized",
                    "message": "Invalid or missing Authorization header"
                }))
            ).into_response();
        }
    };
    
    // Validate API key
    match key_store.validate(&api_key) {
        Some(info) => {
            debug!("Authenticated: {} ({})", info.merchant_id, info.name);
            
            // Store merchant info in request extensions
            request.extensions_mut().insert(info);
            
            next.run(request).await
        }
        None => {
            warn!("Invalid API key: {}", api_key);
            (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({
                    "type": "invalid_request",
                    "code": "invalid_api_key",
                    "message": "Invalid or inactive API key"
                }))
            ).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_api_key_store() {
        let store = ApiKeyStore::new();
        
        // Valid key
        assert!(store.validate("api_key_demo_123").is_some());
        
        // Invalid key
        assert!(store.validate("invalid_key").is_none());
    }

    #[test]
    fn test_extract_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer test_key_123"),
        );
        
        let key = extract_api_key(&headers).unwrap();
        assert_eq!(key, "test_key_123");
    }

    #[test]
    fn test_revoke_key() {
        let store = ApiKeyStore::new();
        
        // Key should be valid initially
        assert!(store.validate("api_key_demo_123").is_some());
        
        // Revoke it
        assert!(store.revoke_key("api_key_demo_123"));
        
        // Should now be invalid
        assert!(store.validate("api_key_demo_123").is_none());
    }
} 