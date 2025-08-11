use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Authentication request objects
#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
    pub tenant_id: Option<String>,
}

/// Reset password request
#[derive(Debug, Serialize, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub password: String,
}

/// Change password request
#[derive(Debug, Serialize, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

/// Forgot password request
#[derive(Debug, Serialize, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// Update user request
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// User response
#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub tenant_id: Option<String>,
    pub active: bool,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API key response
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyResponse {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Create API key request
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    pub expires_in_days: Option<i64>,
    pub permissions: Option<Vec<String>>,
}

/// API key creation response
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyCreationResponse {
    pub key: String,
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Revoke API key request
#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeApiKeyRequest {
    pub api_key_id: Uuid,
}

/// User role request
#[derive(Debug, Serialize, Deserialize)]
pub struct UserRoleRequest {
    pub user_id: Uuid,
    pub role_name: String,
}
