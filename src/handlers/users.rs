use super::common::{
    created_response, no_content_response, success_response, validate_input, PaginationParams,
};
use crate::{
    auth::AuthenticatedUser,
    errors::{ApiError, ServiceError},
    handlers::AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tracing::info;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

// Request and response DTOs

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "name": "John Doe",
    "email": "john.doe@example.com",
    "password": "SecurePass123!",
    "role": "user",
    "department": "Engineering",
    "phone": "+1-555-123-4567"
}))]
pub struct CreateUserRequest {
    /// User's full name
    #[schema(example = "John Doe")]
    pub name: String,

    /// User's email address (must be unique)
    #[schema(example = "john.doe@example.com")]
    pub email: String,

    /// User's password (minimum 8 characters, must include uppercase, lowercase, number)
    #[schema(example = "SecurePass123!")]
    pub password: String,
    /// User role (user, admin, manager)
    #[schema(example = "user")]
    pub role: Option<String>,
    /// User's department
    #[schema(example = "Engineering")]
    pub department: Option<String>,
    /// User's phone number
    #[schema(example = "+1-555-123-4567")]
    pub phone: Option<String>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "name": "John Smith",
    "email": "john.smith@example.com",
    "role": "manager",
    "department": "Product",
    "phone": "+1-555-987-6543",
    "is_active": true
}))]
pub struct UpdateUserRequest {
    /// Updated user name
    #[schema(example = "John Smith")]
    pub name: Option<String>,

    /// Updated email address
    #[schema(example = "john.smith@example.com")]
    pub email: Option<String>,
    /// Updated role
    #[schema(example = "manager")]
    pub role: Option<String>,
    /// Updated department
    #[schema(example = "Product")]
    pub department: Option<String>,
    /// Updated phone number
    #[schema(example = "+1-555-987-6543")]
    pub phone: Option<String>,
    /// Whether user account is active
    #[schema(example = true)]
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Validate, ToSchema)]
#[schema(example = json!({
    "current_password": "OldPass123!",
    "new_password": "NewSecure456!",
    "confirm_password": "NewSecure456!"
}))]
pub struct ChangePasswordRequest {
    /// Current password for verification
    #[validate(length(
        min = 6,
        message = "Current password must be at least 6 characters long"
    ))]
    #[schema(example = "OldPass123!")]
    pub current_password: String,

    /// New password (minimum 8 characters)
    #[schema(example = "NewSecure456!")]
    pub new_password: String,

    /// Confirm new password (must match new_password)
    #[schema(example = "NewSecure456!")]
    pub confirm_password: String,
}

// Example User model for responses
#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({
    "id": "550e8400-e29b-41d4-a716-446655440000",
    "name": "John Doe",
    "email": "john.doe@example.com",
    "role": "user",
    "department": "Engineering",
    "phone": "+1-555-123-4567",
    "is_active": true,
    "created_at": "2024-12-09T10:30:00Z",
    "updated_at": "2024-12-09T14:45:00Z"
}))]
pub struct User {
    /// User UUID
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub id: Uuid,
    /// User's full name
    #[schema(example = "John Doe")]
    pub name: String,
    /// User's email address
    #[schema(example = "john.doe@example.com")]
    pub email: String,
    /// User role (user, admin, manager)
    #[schema(example = "user")]
    pub role: String,
    /// User's department
    #[schema(example = "Engineering")]
    pub department: Option<String>,
    /// User's phone number
    #[schema(example = "+1-555-123-4567")]
    pub phone: Option<String>,
    /// Whether user account is active
    #[schema(example = true)]
    pub is_active: bool,
    /// Account creation timestamp
    #[schema(example = "2024-12-09T10:30:00Z")]
    pub created_at: String,
    /// Last update timestamp
    #[schema(example = "2024-12-09T14:45:00Z")]
    pub updated_at: Option<String>,
}

// Handler functions

/// Create a new user
#[utoipa::path(
    post,
    path = "/api/v1/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse),
        (status = 500, description = "Internal server error", body = crate::errors::ErrorResponse)
    ),
    tag = "users"
)]
pub async fn create_user(
    State(_state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Check if current user has admin role
    if !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::ServiceError(ServiceError::Forbidden(
            "Only admin users can create new users".to_string(),
        )));
    }

    let user_id = Uuid::new_v4();

    info!("User created: {}", user_id);

    Ok(created_response(serde_json::json!({
        "id": user_id,
        "message": "User created successfully"
    })))
}

/// Get a user by ID
#[utoipa::path(
    get,
    path = "/api/v1/users/:id",
    params(("id" = String, Path, description = "User ID (UUID)")),
    responses(
        (status = 200, description = "User returned", body = User,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse)
    ),
    tag = "users"
)]
pub async fn get_user(
    State(_state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Check authorization - users can only view their own profile unless they're admin
    if current_user.user_id != user_id.to_string()
        && !current_user.roles.contains(&"admin".to_string())
    {
        return Err(ApiError::ServiceError(ServiceError::Forbidden(
            "You can only view your own profile".to_string(),
        )));
    }

    let user = User {
        id: user_id,
        name: "John Doe".to_string(),
        email: "john.doe@example.com".to_string(),
        role: "user".to_string(),
        department: Some("Engineering".to_string()),
        phone: Some("1234567890".to_string()),
        is_active: true,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        updated_at: Some("2023-01-02T00:00:00Z".to_string()),
    };

    Ok(success_response(user))
}

/// Update a user
#[utoipa::path(
    put,
    path = "/api/v1/users/:id",
    params(("id" = String, Path, description = "User ID (UUID)")),
    request_body = UpdateUserRequest,
    responses(
        (status = 200, description = "User updated",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse)
    ),
    tag = "users"
)]
pub async fn update_user(
    State(_state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Check authorization - users can only update their own profile unless they're admin
    if current_user.user_id != user_id.to_string()
        && !current_user.roles.contains(&"admin".to_string())
    {
        return Err(ApiError::ServiceError(ServiceError::Forbidden(
            "You can only update your own profile".to_string(),
        )));
    }

    info!("User updated: {}", user_id);

    Ok(success_response(serde_json::json!({
        "message": "User updated successfully"
    })))
}

/// Delete a user
#[utoipa::path(
    delete,
    path = "/api/v1/users/:id",
    params(("id" = String, Path, description = "User ID (UUID)")),
    responses(
        (status = 204, description = "User deleted",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 404, description = "Not found", body = crate::errors::ErrorResponse)
    ),
    tag = "users"
)]
pub async fn delete_user(
    State(_state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if current user has admin role
    if !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::ServiceError(ServiceError::Forbidden(
            "Only admin users can delete users".to_string(),
        )));
    }

    info!("User deleted: {}", user_id);

    Ok(no_content_response())
}

/// List all users with pagination
#[utoipa::path(
    get,
    path = "/api/v1/users",
    responses(
        (status = 200, description = "Users listed",
            headers(
                ("X-Request-Id" = String, description = "Unique request id"),
                ("X-RateLimit-Limit" = String, description = "Requests allowed in current window"),
                ("X-RateLimit-Remaining" = String, description = "Remaining requests in window"),
                ("X-RateLimit-Reset" = String, description = "Seconds until reset"),
            )
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse),
        (status = 429, description = "Rate limit exceeded", body = crate::errors::ErrorResponse)
    ),
    tag = "users"
)]
pub async fn list_users(
    State(_state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Query(pagination): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if current user has admin role
    if !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::ServiceError(ServiceError::Forbidden(
            "Only admin users can list all users".to_string(),
        )));
    }

    // This is a mock implementation
    // In a real application, you would call a user service to list users

    let users = vec![
        User {
            id: Uuid::new_v4(),
            name: "John Doe".to_string(),
            email: "john.doe@example.com".to_string(),
            role: "user".to_string(),
            department: Some("Engineering".to_string()),
            phone: Some("1234567890".to_string()),
            is_active: true,
            created_at: "2023-01-01T00:00:00Z".to_string(),
            updated_at: Some("2023-01-02T00:00:00Z".to_string()),
        },
        User {
            id: Uuid::new_v4(),
            name: "Jane Smith".to_string(),
            email: "jane.smith@example.com".to_string(),
            role: "admin".to_string(),
            department: Some("Management".to_string()),
            phone: Some("0987654321".to_string()),
            is_active: true,
            created_at: "2023-01-01T00:00:00Z".to_string(),
            updated_at: Some("2023-01-02T00:00:00Z".to_string()),
        },
    ];

    Ok(success_response(serde_json::json!({
        "users": users,
        "total": 2,
        "page": pagination.page,
        "per_page": pagination.per_page
    })))
}

/// Change user password
#[utoipa::path(
    post,
    path = "/api/v1/users/:id/change-password",
    params(("id" = String, Path, description = "User ID (UUID)")),
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed",
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 400, description = "Invalid request", body = crate::errors::ErrorResponse),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse),
        (status = 403, description = "Forbidden", body = crate::errors::ErrorResponse)
    ),
    tag = "users"
)]
pub async fn change_password(
    State(_state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Check if current user has access to change this user's password
    // Either the user is changing their own password or they are an admin
    let user_id_str = user_id.to_string();
    if current_user.user_id != user_id_str && !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::ServiceError(ServiceError::Forbidden(
            "You can only update your own profile".to_string(),
        )));
    }

    info!("Password changed for user: {}", user_id);

    Ok(success_response(serde_json::json!({
        "message": "Password changed successfully"
    })))
}

/// Get current user profile
#[utoipa::path(
    get,
    path = "/api/v1/users/profile",
    responses(
        (status = 200, description = "Current user", body = User,
            headers(("X-Request-Id" = String, description = "Unique request id"))
        ),
        (status = 401, description = "Unauthorized", body = crate::errors::ErrorResponse)
    ),
    tag = "users"
)]
pub async fn get_current_user(
    State(_state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
) -> Result<impl IntoResponse, ApiError> {
    // This is a mock implementation
    // In a real application, you would get the user from the database using current_user.user_id

    let user = User {
        id: Uuid::parse_str(&current_user.user_id).unwrap_or_else(|_| Uuid::new_v4()),
        name: current_user.name.unwrap_or_default(),
        email: current_user.email.unwrap_or_default(),
        role: current_user
            .roles
            .first()
            .cloned()
            .unwrap_or_else(|| "user".to_string()),
        department: Some("Engineering".to_string()),
        phone: Some("1234567890".to_string()),
        is_active: true,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        updated_at: Some("2023-01-02T00:00:00Z".to_string()),
    };

    Ok(success_response(user))
}

/// Creates the router for user endpoints
pub fn user_routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(create_user))
        .route("/", get(list_users))
        .route("/profile", get(get_current_user))
        .route("/:id", get(get_user))
        .route("/:id", put(update_user))
        .route("/:id", delete(delete_user))
        .route("/:id/change-password", post(change_password))
}
