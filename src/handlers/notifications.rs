use crate::auth::AuthenticatedUser;
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::services::notifications::{
    delete_user_notification, delete_user_notifications, get_user_notifications,
    mark_notification_as_read, mark_notification_as_unread, Notification,
};
use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;

async fn get_user_notifications_handler(
    State((pool, redis_client)): State<(Arc<DbPool>, Arc<redis::Client>)>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<Notification>>, ServiceError> {
    let user_id = uuid::Uuid::parse_str(&user.user_id)
        .map_err(|_| ServiceError::ValidationError("Invalid user ID".to_string()))?;
    let notifications = get_user_notifications(&pool, &redis_client, user_id).await?;
    Ok(Json(notifications))
}

async fn mark_notification_as_read_handler(
    State((pool, redis_client)): State<(Arc<DbPool>, Arc<redis::Client>)>,
    Path(id): Path<String>,
    user: AuthenticatedUser,
) -> Result<(), ServiceError> {
    let user_id = uuid::Uuid::parse_str(&user.user_id)
        .map_err(|_| ServiceError::ValidationError("Invalid user ID".to_string()))?;
    mark_notification_as_read(&pool, &redis_client, user_id, id).await?;
    Ok(())
}

async fn mark_notification_as_unread_handler(
    State((pool, redis_client)): State<(Arc<DbPool>, Arc<redis::Client>)>,
    Path(id): Path<String>,
    user: AuthenticatedUser,
) -> Result<(), ServiceError> {
    let user_id = uuid::Uuid::parse_str(&user.user_id)
        .map_err(|_| ServiceError::ValidationError("Invalid user ID".to_string()))?;
    mark_notification_as_unread(&pool, &redis_client, user_id, id).await?;
    Ok(())
}

async fn delete_user_notification_handler(
    State((pool, redis_client)): State<(Arc<DbPool>, Arc<redis::Client>)>,
    Path(id): Path<String>,
    user: AuthenticatedUser,
) -> Result<(), ServiceError> {
    let user_id = uuid::Uuid::parse_str(&user.user_id)
        .map_err(|_| ServiceError::ValidationError("Invalid user ID".to_string()))?;
    delete_user_notification(&pool, &redis_client, user_id, id).await?;
    Ok(())
}

pub fn notification_routes() -> Router<(Arc<DbPool>, Arc<redis::Client>)> {
    Router::new()
        .route("/", get(get_user_notifications_handler))
        .route("/{id}/read", post(mark_notification_as_read_handler))
        .route("/{id}/unread", post(mark_notification_as_unread_handler))
        .route("/{id}/delete", delete(delete_user_notification_handler))
}
