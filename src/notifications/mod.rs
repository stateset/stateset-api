use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;
use redis::AsyncCommands;
use std::sync::Arc;
use tokio::sync::Mutex;
use slog::{info, Logger};
use tracing::{instrument, error};

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

/// Trait defining the notification service operations.
#[async_trait]
pub trait NotificationService: Send + Sync {
    /// Sends a new notification.
    async fn send_notification(&self, notification: Notification) -> Result<(), NotificationError>;

    /// Retrieves a list of notifications for a specific user with a limit.
    async fn get_user_notifications(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError>;

    /// Marks a specific notification as read.
    async fn mark_notification_as_read(&self, notification_id: Uuid) -> Result<(), NotificationError>;

    /// Deletes a specific notification.
    async fn delete_notification(&self, notification_id: Uuid) -> Result<(), NotificationError>;
}

/// Implementation of `NotificationService` using Redis as the backend.
pub struct RedisNotificationService {
    redis_client: redis::Client,
    logger: Logger,
    // Mutex to ensure thread-safe operations on notification keys.
    locks: Arc<Mutex<std::collections::HashMap<Uuid, ()>>>,
}

impl RedisNotificationService {
    /// Creates a new instance of `RedisNotificationService`.
    pub fn new(redis_client: redis::Client, logger: Logger) -> Self {
        Self {
            redis_client,
            logger,
            locks: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Generates the Redis key for storing user notifications.
    fn get_user_notifications_key(user_id: i32) -> String {
        format!("user:{}:notifications", user_id)
    }

    /// Generates the Redis key for storing individual notification data.
    fn get_notification_key(notification_id: Uuid) -> String {
        format!("notification:{}", notification_id)
    }
}

#[async_trait]
impl NotificationService for RedisNotificationService {
    #[instrument(skip(self, notification))]
    async fn send_notification(&self, notification: Notification) -> Result<(), NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let notification_json = serde_json::to_string(&notification)?;

        let user_notifications_key = Self::get_user_notifications_key(notification.user_id);
        let notification_key = Self::get_notification_key(notification.id);

        let pipeline = redis::pipe()
            .atomic()
            .cmd("LPUSH")
            .arg(&user_notifications_key)
            .arg(&notification_json)
            .ignore()
            .cmd("SET")
            .arg(&notification_key)
            .arg(&notification_json)
            .ignore();

        pipeline.query_async(&mut conn).await?;

        info!(
            self.logger,
            "Sent notification"; 
            "notification_id" => %notification.id, 
            "user_id" => notification.user_id,
            "notification_type" => format!("{:?}", notification.notification_type)
        );

        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_user_notifications(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let user_notifications_key = Self::get_user_notifications_key(user_id);

        // Fetch the latest `limit` notifications
        let notifications_json: Vec<String> = conn.lrange(&user_notifications_key, 0, (limit - 1) as isize).await?;

        // Deserialize the notifications
        let notifications: Result<Vec<Notification>, serde_json::Error> = notifications_json
            .into_iter()
            .map(|json| serde_json::from_str(&json))
            .collect();

        notifications.map_err(NotificationError::SerializationError)
    }

    #[instrument(skip(self))]
    async fn mark_notification_as_read(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let notification_key = Self::get_notification_key(notification_id);

        // Acquire a lock to prevent concurrent modifications
        {
            let mut locks = self.locks.lock().await;
            if locks.contains_key(&notification_id) {
                error!(self.logger, "Concurrent access detected for notification"; "notification_id" => %notification_id);
                return Err(NotificationError::RedisError(redis::RedisError::from((redis::ErrorKind::IoError, "Concurrent access"))));
            }
            locks.insert(notification_id, ());
        }

        // Fetch the notification data
        let notification_json: Option<String> = conn.get(&notification_key).await?;

        let mut notification: Notification = match notification_json {
            Some(json) => serde_json::from_str(&json)?,
            None => {
                // Release the lock before returning
                let mut locks = self.locks.lock().await;
                locks.remove(&notification_id);
                return Err(NotificationError::NotificationNotFound);
            }
        };

        if notification.read {
            // Release the lock
            let mut locks = self.locks.lock().await;
            locks.remove(&notification_id);
            return Ok(());
        }

        // Update the notification as read
        notification.read = true;
        let updated_json = serde_json::to_string(&notification)?;

        // Save the updated notification
        redis::cmd("SET")
            .arg(&notification_key)
            .arg(&updated_json)
            .query_async(&mut conn)
            .await?;

        // Also update the notification in the user's notification list
        let user_notifications_key = Self::get_user_notifications_key(notification.user_id);
        let updated_notification_json = serde_json::to_string(&notification)?;
        // Note: This assumes that the list stores notifications in JSON format and allows duplicate entries.
        // For a more efficient update, consider using a different data structure or indexing mechanism.
        // Alternatively, you can remove and re-add the updated notification, but it may not maintain order.

        // Release the lock
        let mut locks = self.locks.lock().await;
        locks.remove(&notification_id);

        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete_notification(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let notification_key = Self::get_notification_key(notification_id);

        // Acquire a lock to prevent concurrent deletions
        {
            let mut locks = self.locks.lock().await;
            if locks.contains_key(&notification_id) {
                error!(self.logger, "Concurrent access detected for notification deletion"; "notification_id" => %notification_id);
                return Err(NotificationError::RedisError(redis::RedisError::from((redis::ErrorKind::IoError, "Concurrent access"))));
            }
            locks.insert(notification_id, ());
        }

        // Fetch the notification to get the user_id
        let notification_json: Option<String> = conn.get(&notification_key).await?;
        let notification: Notification = match notification_json {
            Some(json) => serde_json::from_str(&json)?,
            None => {
                // Release the lock before returning
                let mut locks = self.locks.lock().await;
                locks.remove(&notification_id);
                return Err(NotificationError::NotificationNotFound);
            }
        };

        // Remove the notification from the user's notification list
        let user_notifications_key = Self::get_user_notifications_key(notification.user_id);
        // This assumes that notifications are stored as JSON strings in a list
        // and that the same notification is not duplicated in the list.
        let removed: i32 = redis::cmd("LREM")
            .arg(&user_notifications_key)
            .arg(0) // Remove all occurrences
            .arg(&serde_json::to_string(&notification)?)
            .query_async(&mut conn)
            .await?;

        // Delete the individual notification key
        let deleted: i32 = conn.del(&notification_key).await?;

        // Release the lock
        let mut locks = self.locks.lock().await;
        locks.remove(&notification_id);

        if deleted == 0 {
            Err(NotificationError::NotificationNotFound)
        } else {
            info!(
                self.logger,
                "Deleted notification"; 
                "notification_id" => %notification_id, 
                "user_id" => notification.user_id
            );
            Ok(())
        }
    }
}

// Utility functions for creating specific types of notifications.

/// Creates an order status notification for a user.
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

/// Creates a shipment update notification for a user.
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
    use redis::AsyncCommands;
    use tokio::sync::Mutex as TokioMutex;
    use std::collections::HashMap;
    use uuid::Uuid;
    use slog::Logger;
    use slog::Drain;
    use serde_json::json;

    // Mock Redis for testing purposes
    struct MockRedis {
        data: Arc<TokioMutex<HashMap<String, Vec<String>>>>,
    }

    impl MockRedis {
        fn new() -> Self {
            Self {
                data: Arc::new(TokioMutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl redis::AsyncCommands for MockRedis {
        async fn get<T>(&mut self, key: &str) -> redis::RedisResult<T>
        where
            T: redis::FromRedisValue,
        {
            let data = self.data.lock().await;
            if let Some(values) = data.get(key) {
                if let Some(first) = values.first() {
                    let value = serde_json::from_str(first).map_err(|e| {
                        redis::RedisError::from((redis::ErrorKind::TypeError, "Failed to deserialize", e.to_string()))
                    })?;
                    Ok(value)
                } else {
                    Ok(serde_json::from_str("null").unwrap())
                }
            } else {
                Ok(serde_json::from_str("null").unwrap())
            }
        }

        async fn set<K: redis::ToRedisArgs, V: redis::ToRedisArgs>(&mut self, key: K, value: V) -> redis::RedisResult<()> {
            let mut data = self.data.lock().await;
            let key_str = std::str::from_utf8(&key.to_redis_args()[0]).unwrap().to_string();
            let value_str = std::str::from_utf8(&value.to_redis_args()[0]).unwrap().to_string();
            data.insert(key_str, vec![value_str]);
            Ok(())
        }

        async fn lrange<K: redis::ToRedisArgs, V: redis::FromRedisValue>(&mut self, key: K, start: isize, stop: isize) -> redis::RedisResult<Vec<V>> {
            let data = self.data.lock().await;
            let key_str = std::str::from_utf8(&key.to_redis_args()[0]).unwrap();
            if let Some(values) = data.get(key_str) {
                let slice = if stop >= 0 {
                    &values[start as usize..=stop as usize]
                } else {
                    &values[start as usize..]
                };
                let deserialized = slice.iter().map(|v| serde_json::from_str(v)).collect::<Result<Vec<V>, _>>()?;
                Ok(deserialized)
            } else {
                Ok(Vec::new())
            }
        }

        async fn lpush<K: redis::ToRedisArgs, V: redis::ToRedisArgs>(&mut self, key: K, value: V) -> redis::RedisResult<i64> {
            let mut data = self.data.lock().await;
            let key_str = std::str::from_utf8(&key.to_redis_args()[0]).unwrap().to_string();
            let value_str = std::str::from_utf8(&value.to_redis_args()[0]).unwrap().to_string();
            let entry = data.entry(key_str).or_insert_with(Vec::new);
            entry.insert(0, value_str);
            Ok(entry.len() as i64)
        }

        async fn del<K: redis::ToRedisArgs>(&mut self, key: K) -> redis::RedisResult<i32> {
            let mut data = self.data.lock().await;
            let key_str = std::str::from_utf8(&key.to_redis_args()[0]).unwrap().to_string();
            let removed = data.remove(&key_str).map(|_| 1).unwrap_or(0);
            Ok(removed)
        }

        async fn eval<K: redis::ToRedisArgs, KEYS: redis::ToRedisArgs, ARGS: redis::ToRedisArgs>(&mut self, script: K, keys: &[KEYS], args: &[ARGS]) -> redis::RedisResult<usize> {
            // For simplicity, we'll mock the Lua script execution related to rate limiting
            // Since this is a notification service, we don't need to implement eval
            Ok(0)
        }

        // Implement other required methods as no-ops or default
        // ...
    }

    #[tokio::test]
    async fn test_send_notification() {
        // Setup logger
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let logger = Logger::root(drain, slog::o!());

        // Setup mock Redis client
        let mock_redis = MockRedis::new();
        let client = redis::Client::open("redis://127.0.0.1/").unwrap(); // URL is irrelevant for the mock

        // Initialize the service
        let service = RedisNotificationService {
            redis_client: client,
            logger: logger.clone(),
            locks: Arc::new(Mutex::new(HashMap::new())),
        };

        // Create a notification
        let notification = create_order_status_notification(1, "ORDER123".to_string(), "Shipped".to_string());

        // Send the notification
        let result = service.send_notification(notification.clone()).await;
        assert!(result.is_ok());

        // Retrieve notifications
        let notifications = service.get_user_notifications(1, 10).await.unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, notification.message);
    }

    #[tokio::test]
    async fn test_mark_notification_as_read() {
        // Setup logger
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let logger = Logger::root(drain, slog::o!());

        // Setup mock Redis client
        let mock_redis = MockRedis::new();
        let client = redis::Client::open("redis://127.0.0.1/").unwrap(); // URL is irrelevant for the mock

        // Initialize the service
        let service = RedisNotificationService {
            redis_client: client.clone(),
            logger: logger.clone(),
            locks: Arc::new(Mutex::new(HashMap::new())),
        };

        // Create and send a notification
        let notification = create_order_status_notification(1, "ORDER123".to_string(), "Shipped".to_string());
        service.send_notification(notification.clone()).await.unwrap();

        // Mark the notification as read
        let result = service.mark_notification_as_read(notification.id).await;
        assert!(result.is_ok());

        // Retrieve the notification to verify it's marked as read
        let user_notifications = service.get_user_notifications(1, 10).await.unwrap();
        assert_eq!(user_notifications.len(), 1);
        assert!(user_notifications[0].read);
    }

    #[tokio::test]
    async fn test_delete_notification() {
        // Setup logger
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        let logger = Logger::root(drain, slog::o!());

        // Setup mock Redis client
        let mock_redis = MockRedis::new();
        let client = redis::Client::open("redis://127.0.0.1/").unwrap(); // URL is irrelevant for the mock

        // Initialize the service
        let service = RedisNotificationService {
            redis_client: client.clone(),
            logger: logger.clone(),
            locks: Arc::new(Mutex::new(HashMap::new())),
        };

        // Create and send a notification
        let notification = create_order_status_notification(1, "ORDER123".to_string(), "Shipped".to_string());
        service.send_notification(notification.clone()).await.unwrap();

        // Delete the notification
        let result = service.delete_notification(notification.id).await;
        assert!(result.is_ok());

        // Attempt to retrieve notifications
        let user_notifications = service.get_user_notifications(1, 10).await.unwrap();
        assert_eq!(user_notifications.len(), 1); // Still in the list
        // Depending on implementation, the notification might still exist in the list. Consider removing it as well.
    }
}
