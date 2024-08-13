use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub user_id: i32,
    pub message: String,
    pub read: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn send_notification(redis_client: &redis::Client, notification: Notification) -> Result<(), redis::RedisError> {
    let mut conn = redis_client.get_async_connection().await?;
    let notification_json = serde_json::to_string(&notification).unwrap();
    redis::cmd("LPUSH")
        .arg(format!("user:{}:notifications", notification.user_id))
        .arg(notification_json)
        .query_async(&mut conn)
        .await?;
    Ok(())
}