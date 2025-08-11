use super::common::{
    created_response, map_service_error, no_content_response, success_response, validate_input,
    PaginationParams,
};
use crate::{
    auth::AuthenticatedUser,
    errors::{ApiError, ServiceError},
    handlers::AppState,
};
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;
use validator::Validate;

// Request and response DTOs

#[derive(Debug, Deserialize, Validate)]
pub struct CreateUserRequest {
    #[validate(length(min = 2, message = "Name must be at least 2 characters long"))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 6, message = "Password must be at least 6 characters long"))]
    pub password: String,
    pub role: Option<String>,
    pub department: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateUserRequest {
    pub name: Option<String>,
    #[validate(email)]
    pub email: Option<String>,
    pub role: Option<String>,
    pub department: Option<String>,
    pub phone: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ChangePasswordRequest {
    #[validate(length(
        min = 6,
        message = "Current password must be at least 6 characters long"
    ))]
    pub current_password: String,
    #[validate(length(min = 6, message = "New password must be at least 6 characters long"))]
    pub new_password: String,
    #[validate(must_match(other = "new_password", message = "Passwords do not match"))]
    pub confirm_password: String,
}

// Example User model for responses
#[derive(Debug, Serialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
    pub role: String,
    pub department: Option<String>,
    pub phone: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: Option<String>,
}

// Handler functions

/// Create a new user
async fn create_user(
    State(state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Check if current user has admin role
    if !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::Forbidden {
            message: "Only admin users can create new users".to_string(),
        });
    }

    let user_id = Uuid::new_v4();

    info!("User created: {}", user_id);

    created_response(serde_json::json!({
        "id": user_id,
        "message": "User created successfully"
    }))
}

/// Get a user by ID
async fn get_user(
    State(state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Check authorization - users can only view their own profile unless they're admin
    if current_user.user_id != user_id.to_string() && !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::Forbidden {
            message: "You can only view your own profile".to_string(),
        });
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

    success_response(user)
}

/// Update a user
async fn update_user(
    State(state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Check authorization - users can only update their own profile unless they're admin
    if current_user.user_id != user_id.to_string() && !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::Forbidden {
            message: "You can only update your own profile".to_string(),
        });
    }

    info!("User updated: {}", user_id);

    success_response(serde_json::json!({
        "message": "User updated successfully"
    }))
}

/// Delete a user
async fn delete_user(
    State(state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if current user has admin role
    if !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::Forbidden {
            message: "Only admin users can delete users".to_string(),
        });
    }

    info!("User deleted: {}", user_id);

    no_content_response()
}

/// List all users with pagination
async fn list_users(
    State(state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Query(pagination): Query<PaginationParams>,
) -> Result<impl IntoResponse, ApiError> {
    // Check if current user has admin role
    if !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::Forbidden {
            message: "Only admin users can list all users".to_string(),
        });
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

    success_response(serde_json::json!({
        "users": users,
        "total": 2,
        "page": pagination.page,
        "per_page": pagination.per_page
    }))
}

/// Change user password
async fn change_password(
    State(state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, ApiError> {
    validate_input(&payload)?;

    // Check if current user has access to change this user's password
    // Either the user is changing their own password or they are an admin
    let user_id_str = user_id.to_string();
    if current_user.user_id != user_id_str && !current_user.roles.contains(&"admin".to_string()) {
        return Err(ApiError::Forbidden {
            message: "You can only update your own profile".to_string(),
        });
    }

    info!("Password changed for user: {}", user_id);

    success_response(serde_json::json!({
        "message": "Password changed successfully"
    }))
}

/// Get current user profile
async fn get_current_user(
    State(state): State<Arc<AppState>>,
    current_user: AuthenticatedUser,
) -> Result<impl IntoResponse, ApiError> {
    // This is a mock implementation
    // In a real application, you would get the user from the database using current_user.user_id

    let user = User {
        id: Uuid::parse_str(&current_user.user_id).unwrap_or_else(|_| Uuid::new_v4()),
        name: current_user.name.unwrap_or_default(),
        email: current_user.email.unwrap_or_default(),
        role: current_user.roles.first().cloned().unwrap_or_else(|| "user".to_string()),
        department: Some("Engineering".to_string()),
        phone: Some("1234567890".to_string()),
        is_active: true,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        updated_at: Some("2023-01-02T00:00:00Z".to_string()),
    };

    success_response(user)
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
