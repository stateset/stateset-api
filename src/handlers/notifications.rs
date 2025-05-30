use axum::{
    extract::{Path, State},
    routing::{get, post, delete},
    Json, Router,
};
use std::sync::Arc;
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::services::notifications::{get_user_notifications, mark_notification_as_read, mark_notification_as_unread, delete_user_notification};
use crate::auth::AuthenticatedUser;

async fn get_user_notifications_handler(
    State(pool): State<Arc<DbPool>>,
    State(redis_client): State<Arc<redis::Client>>,
    user: AuthenticatedUser,
) -> Result<Json<Vec<Notification>>, ServiceError> {
    let notifications = get_user_notifications(&pool, &redis_client, user.user_id).await?;
    Ok(Json(notifications))
}

async fn mark_notification_as_read_handler(
    State(pool): State<Arc<DbPool>>,
    State(redis_client): State<Arc<redis::Client>>,
    Path(id): Path<String>,
    user: AuthenticatedUser,
) -> Result<(), ServiceError> {
    mark_notification_as_read(&pool, &redis_client, user.user_id, id).await?;
    Ok(())
}

async fn mark_notification_as_unread_handler(
    State(pool): State<Arc<DbPool>>,
    State(redis_client): State<Arc<redis::Client>>,
    Path(id): Path<String>,
    user: AuthenticatedUser,
) -> Result<(), ServiceError> {
    mark_notification_as_unread(&pool, &redis_client, user.user_id, id).await?;
    Ok(())
}

async fn delete_user_notification_handler(
    State(pool): State<Arc<DbPool>>,
    State(redis_client): State<Arc<redis::Client>>,
    Path(id): Path<String>,
    user: AuthenticatedUser,
) -> Result<(), ServiceError> {
    delete_user_notification(&pool, &redis_client, user.user_id, id).await?;
    Ok(())
}

pub fn notification_routes() -> Router<(Arc<DbPool>, Arc<redis::Client>)> {
    Router::new()
        .route("/", get(get_user_notifications_handler))
        .route("/:id/read", post(mark_notification_as_read_handler))
        .route("/:id/unread", post(mark_notification_as_unread_handler))
        .route("/:id/delete", delete(delete_user_notification_handler))
}