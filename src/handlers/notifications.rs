use actix_web::{get, post, web, HttpResponse};
use crate::db::DbPool;
use crate::errors::ServiceError;
use crate::services::notifications::{get_user_notifications, mark_notification_as_read};
use crate::auth::AuthenticatedUser;

#[get("")]
async fn get_user_notifications(
    pool: web::Data<DbPool>,
    redis_client: web::Data<redis::Client>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    let notifications = get_user_notifications(&pool, &redis_client, user.user_id).await?;
    Ok(HttpResponse::Ok().json(notifications))
}

#[post("/{id}/read")]
async fn mark_notification_as_read(
    pool: web::Data<DbPool>,
    redis_client: web::Data<redis::Client>,
    id: web::Path<String>,
    user: AuthenticatedUser,
) -> Result<HttpResponse, ServiceError> {
    mark_notification_as_read(&pool, &redis_client, user.user_id, id.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}