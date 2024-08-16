use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use async_trait::async_trait;;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: i32,
    pub message: String,
    pub notification_type: NotificationType,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NotificationType {
    OrderStatus,
    ShipmentUpdate,
    InventoryAlert,
    SystemMessage,
}

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Notification not found")]
    NotificationNotFound,
}

#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn send_notification(&self, notification: Notification) -> Result<(), NotificationError>;
    async fn get_user_notifications(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError>;
    async fn mark_notification_as_read(&self, notification_id: Uuid) -> Result<(), NotificationError>;
    async fn delete_notification(&self, notification_id: Uuid) -> Result<(), NotificationError>;
}

pub struct RedisNotificationService {
    redis_client: redis::Client,
}

impl RedisNotificationService {
    pub fn new(redis_client: redis::Client) -> Self {
        Self { redis_client }
    }

    fn get_user_notifications_key(user_id: i32) -> String {
        format!("user:{}:notifications", user_id)
    }
}

#[async_trait]
impl NotificationService for RedisNotificationService {
    async fn send_notification(&self, notification: Notification) -> Result<(), NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let notification_json = serde_json::to_string(&notification)?;
        redis::cmd("LPUSH")
            .arg(Self::get_user_notifications_key(notification.user_id))
            .arg(notification_json)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    async fn get_user_notifications(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let notifications_json: Vec<String> = redis::cmd("LRANGE")
            .arg(Self::get_user_notifications_key(user_id))
            .arg(0)
            .arg(limit - 1)
            .query_async(&mut conn)
            .await?;

        notifications_json
            .into_iter()
            .map(|json| serde_json::from_str(&json).map_err(NotificationError::from))
            .collect()
    }

    async fn mark_notification_as_read(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        // This implementation assumes notifications are stored individually.
        // You might need to adjust this based on your actual data structure.
        let key = format!("notification:{}", notification_id);
        let mut notification: Notification = redis::cmd("GET")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        
        if notification.read {
            return Ok(());
        }

        notification.read = true;
        let updated_json = serde_json::to_string(&notification)?;
        redis::cmd("SET")
            .arg(&key)
            .arg(updated_json)
            .query_async(&mut conn)
            .await?;
        
        Ok(())
    }

    async fn delete_notification(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let key = format!("notification:{}", notification_id);
        let deleted: i32 = redis::cmd("DEL")
            .arg(&key)
            .query_async(&mut conn)
            .await?;
        
        if deleted == 0 {
            Err(NotificationError::NotificationNotFound)
        } else {
            Ok(())
        }
    }
}

// Utility functions

pub fn create_order_status_notification(user_id: i32, order_id: String, status: String) -> Notification {
    Notification {
        id: Uuid::new_v4(),
        user_id,
        message: format!("Your order {} status has been updated to: {}", order_id, status),
        notification_type: NotificationType::OrderStatus,
        read: false,
        created_at: Utc::now(),
    }
}

pub fn create_shipment_update_notification(user_id: i32, shipment_id: String, update: String) -> Notification {
    Notification {
        id: Uuid::new_v4(),
        user_id,
        message: format!("Shipment {} update: {}", shipment_id, update),
        notification_type: NotificationType::ShipmentUpdate,
        read: false,
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;

    mock! {
        pub NotificationServiceMock {}
        #[async_trait]
        impl NotificationService for NotificationServiceMock {
            async fn send_notification(&self, notification: Notification) -> Result<(), NotificationError>;
            async fn get_user_notifications(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError>;
            async fn mark_notification_as_read(&self, notification_id: Uuid) -> Result<(), NotificationError>;
            async fn delete_notification(&self, notification_id: Uuid) -> Result<(), NotificationError>;
        }
    }

    #[tokio::test]
    async fn test_send_notification() {
        let mut mock = MockNotificationServiceMock::new();
        mock.expect_send_notification()
            .with(function(|n: &Notification| n.user_id == 1))
            .times(1)
            .returning(|_| Ok(()));

        let notification = create_order_status_notification(1, "ORDER123".to_string(), "Shipped".to_string());
        assert!(mock.send_notification(notification).await.is_ok());
    }
}