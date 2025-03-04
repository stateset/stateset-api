use axum::{
    routing::{get, post, put, delete},
    extract::{State, Path, Query, Json},
    Router,
};
use std::sync::Arc;
use uuid::Uuid;
use crate::{
    auth::AuthenticatedUser,
    errors::{ApiError, ServiceError},
    AppState,
};
use serde::{Serialize, Deserialize};
use validator::Validate;
use tracing::{info, error};
use super::common::{validate_input, map_service_error, success_response, created_response, no_content_response, PaginationParams};

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
    #[validate(length(min = 6, message = "Current password must be at least 6 characters long"))]
    pub current_password: String,
    #[validate(length(min = 6, message = "New password must be at least 6 characters long"))]
    pub new_password: String,
    #[validate(must_match = "new_password", message = "Passwords do not match")]
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
    AuthenticatedUser(current_user): AuthenticatedUser,
    Json(payload): Json<CreateUserRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Check if current user has admin role
    if current_user.role \!= "admin" {
        return Err(ApiError::Forbidden("Only admin users can create new users".to_string()));
    }
    
    // This is a mock implementation
    // In a real application, you would call a user service to create the user
    
    let user_id = Uuid::new_v4();
    
    info\!("User created: {}", user_id);
    
    created_response(serde_json::json\!({
        "id": user_id,
        "message": "User created successfully"
    }))
}

/// Get a user by ID
async fn get_user(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    // Check if current user has access to this user
    // Either the user is requesting their own profile or they are an admin
    let user_id_str = user_id.to_string();
    if current_user.user_id \!= user_id_str && current_user.role \!= "admin" {
        return Err(ApiError::Forbidden("Access denied".to_string()));
    }
    
    // This is a mock implementation
    // In a real application, you would call a user service to get the user
    
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
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Check if current user has access to update this user
    // Either the user is updating their own profile or they are an admin
    let user_id_str = user_id.to_string();
    if current_user.user_id \!= user_id_str && current_user.role \!= "admin" {
        return Err(ApiError::Forbidden("Access denied".to_string()));
    }
    
    // This is a mock implementation
    // In a real application, you would call a user service to update the user
    
    info\!("User updated: {}", user_id);
    
    success_response(serde_json::json\!({
        "message": "User updated successfully"
    }))
}

/// Delete a user
async fn delete_user(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(user_id): Path<Uuid>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    // Only admins can delete users
    if current_user.role \!= "admin" {
        return Err(ApiError::Forbidden("Only admin users can delete users".to_string()));
    }
    
    // This is a mock implementation
    // In a real application, you would call a user service to delete the user
    
    info\!("User deleted: {}", user_id);
    
    no_content_response()
}

/// List all users with pagination
async fn list_users(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(current_user): AuthenticatedUser,
    Query(pagination): Query<PaginationParams>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    // Only admins can list all users
    if current_user.role \!= "admin" {
        return Err(ApiError::Forbidden("Only admin users can list all users".to_string()));
    }
    
    // This is a mock implementation
    // In a real application, you would call a user service to list users
    
    let users = vec\![
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
    
    success_response(serde_json::json\!({
        "users": users,
        "total": 2,
        "page": pagination.page,
        "per_page": pagination.per_page
    }))
}

/// Change user password
async fn change_password(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(current_user): AuthenticatedUser,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    validate_input(&payload)?;
    
    // Check if current user has access to change this user's password
    // Either the user is changing their own password or they are an admin
    let user_id_str = user_id.to_string();
    if current_user.user_id \!= user_id_str && current_user.role \!= "admin" {
        return Err(ApiError::Forbidden("Access denied".to_string()));
    }
    
    // This is a mock implementation
    // In a real application, you would:
    // 1. Verify the current password
    // 2. Hash the new password
    // 3. Update the user's password in the database
    
    info\!("Password changed for user: {}", user_id);
    
    success_response(serde_json::json\!({
        "message": "Password changed successfully"
    }))
}

/// Get current user profile
async fn get_current_user(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(current_user): AuthenticatedUser,
) -> Result<impl axum::response::IntoResponse, ApiError> {
    // This is a mock implementation
    // In a real application, you would get the user from the database using current_user.user_id
    
    let user = User {
        id: Uuid::parse_str(&current_user.user_id).unwrap_or_else(|_| Uuid::new_v4()),
        name: current_user.name,
        email: current_user.email,
        role: current_user.role,
        department: Some("Engineering".to_string()),
        phone: Some("1234567890".to_string()),
        is_active: true,
        created_at: "2023-01-01T00:00:00Z".to_string(),
        updated_at: Some("2023-01-02T00:00:00Z".to_string()),
    };
    
    success_response(user)
}

/// Creates the router for user endpoints
pub fn user_routes() -> Router {
    Router::new()
        .route("/", post(create_user))
        .route("/", get(list_users))
        .route("/profile", get(get_current_user))
        .route("/:id", get(get_user))
        .route("/:id", put(update_user))
        .route("/:id", delete(delete_user))
        .route("/:id/change-password", post(change_password))
}
