use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use thiserror::Error;
use uuid::Uuid;
use redis::{AsyncCommands, Client, RedisResult};
use std::sync::Arc;
use tokio::sync::RwLock;
use slog::{info, warn, Logger};
use tracing::{instrument, error};
use crate::circuit_breaker::{CircuitBreakerRegistry, CircuitBreakerError};

/// Represents a notification
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: i32,
    pub message: String,
    #[serde(rename = "type")]
    pub notification_type: NotificationType,
    pub read: bool,
    pub created_at: DateTime<Utc>,
}

/// Types of notifications
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum NotificationType {
    OrderStatus,
    ShipmentUpdate,
    InventoryAlert,
    SystemMessage,
}

/// Notification service errors
#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Notification not found: {0}")]
    NotFound(Uuid),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Circuit breaker open: {0}")]
    CircuitBreakerOpen(String),
}

/// Trait for notification service operations
#[async_trait]
pub trait NotificationService: Send + Sync {
    async fn send(&self, notification: Notification) -> Result<(), NotificationError>;
    async fn get_user_notifications(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError>;
    async fn mark_as_read(&self, notification_id: Uuid) -> Result<(), NotificationError>;
    async fn delete(&self, notification_id: Uuid) -> Result<(), NotificationError>;
}

/// Redis-based notification service implementation
#[derive(Clone)]
pub struct RedisNotificationService {
    redis: Arc<Client>,
    logger: Logger,
    locks: Arc<RwLock<Vec<Uuid>>>, // Simpler lock structure
    circuit_breaker: Option<Arc<CircuitBreakerRegistry>>,
}

impl RedisNotificationService {
    pub async fn new(redis_url: &str, logger: Logger) -> Result<Self, NotificationError> {
        let redis = Client::open(redis_url)
            .map_err(NotificationError::Redis)?;
        Ok(Self {
            redis: Arc::new(redis),
            logger,
            locks: Arc::new(RwLock::new(Vec::new())),
            circuit_breaker: None,
        })
    }
    
    /// Create a new RedisNotificationService with circuit breaker protection
    pub async fn new_with_circuit_breaker(
        redis_url: &str, 
        logger: Logger,
        circuit_breaker: Arc<CircuitBreakerRegistry>
    ) -> Result<Self, NotificationError> {
        let redis = Client::open(redis_url)
            .map_err(NotificationError::Redis)?;
        Ok(Self {
            redis: Arc::new(redis),
            logger,
            locks: Arc::new(RwLock::new(Vec::new())),
            circuit_breaker: Some(circuit_breaker),
        })
    }

    fn user_key(user_id: i32) -> String {
        format!("notifications:user:{}", user_id)
    }

    fn notification_key(id: Uuid) -> String {
        format!("notification:{}", id)
    }

    /// Efficiently updates notification in user's list
    async fn update_user_list(
        &self,
        conn: &mut redis::aio::Connection,
        user_id: i32,
        notification: &Notification,
    ) -> Result<(), NotificationError> {
        let user_key = Self::user_key(user_id);
        let json = serde_json::to_string(notification)?;
        
        // Use sorted set with timestamp as score for better ordering
        conn.zadd(user_key, json, notification.created_at.timestamp())
            .await?;
        
        // Trim to keep only recent notifications (e.g., last 1000)
        conn.zremrangebyrank(&user_key, 0, -1001).await?;
        
        Ok(())
    }
}

/// Conversion from CircuitBreakerError to NotificationError
impl From<CircuitBreakerError> for NotificationError {
    fn from(err: CircuitBreakerError) -> Self {
        Self::CircuitBreakerOpen(err.to_string())
    }
}

#[async_trait]
impl NotificationService for RedisNotificationService {
    #[instrument(skip(self, notification), fields(id = %notification.id, user_id = notification.user_id))]
    async fn send(&self, notification: Notification) -> Result<(), NotificationError> {
        // Execute with circuit breaker if available
        if let Some(cb) = &self.circuit_breaker {
            return cb.execute("redis-notification-service", || async {
                self.send_with_connection(notification).await
            }).await;
        }
        
        // Otherwise execute directly
        self.send_with_connection(notification).await
    }
    
    // Internal helper to send notification with Redis connection
    async fn send_with_connection(&self, notification: Notification) -> Result<(), NotificationError> {
        let mut conn = self.redis.get_async_connection().await?;
        let json = serde_json::to_string(&notification)?;

        let notification_key = Self::notification_key(notification.id);
        
        redis::pipe()
            .atomic()
            .set(&notification_key, &json)
            .zadd(
                Self::user_key(notification.user_id),
                &json,
                notification.created_at.timestamp()
            )
            .query_async(&mut conn)
            .await?;

        info!(self.logger, "Notification sent"; 
            "type" => format!("{:?}", notification.notification_type)
        );
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_user_notifications(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError> {
        // Execute with circuit breaker if available
        if let Some(cb) = &self.circuit_breaker {
            return cb.execute("redis-notification-service", || async {
                self.get_user_notifications_with_connection(user_id, limit).await
            }).await;
        }
        
        // Otherwise execute directly
        self.get_user_notifications_with_connection(user_id, limit).await
    }
    
    // Internal helper to get user notifications with Redis connection
    async fn get_user_notifications_with_connection(&self, user_id: i32, limit: usize) -> Result<Vec<Notification>, NotificationError> {
        let mut conn = self.redis.get_async_connection().await?;
        let user_key = Self::user_key(user_id);

        // Get latest notifications using ZREVRANGE
        let notifications_json: Vec<String> = conn
            .zrevrange(user_key, 0, limit as isize - 1)
            .await?;

        let notifications: Vec<Notification> = notifications_json
            .into_iter()
            .map(|json| serde_json::from_str(&json))
            .collect::<Result<Vec<_>, _>>()?;

        info!(self.logger, "Retrieved user notifications"; "count" => notifications.len());
        Ok(notifications)
    }

    #[instrument(skip(self))]
    async fn mark_as_read(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        // Execute with circuit breaker if available
        if let Some(cb) = &self.circuit_breaker {
            return cb.execute("redis-notification-service", || async {
                self.mark_as_read_with_connection(notification_id).await
            }).await;
        }
        
        // Otherwise execute directly
        self.mark_as_read_with_connection(notification_id).await
    }
    
    // Internal helper to mark a notification as read
    async fn mark_as_read_with_connection(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        let mut locks = self.locks.write().await;
        if locks.contains(&notification_id) {
            return Err(NotificationError::Internal("Concurrent modification".to_string()));
        }
        locks.push(notification_id);

        let mut conn = self.redis.get_async_connection().await?;
        let notification_key = Self::notification_key(notification_id);
        
        let json: Option<String> = conn.get(&notification_key).await?;
        let mut notification = json
            .map(|j| serde_json::from_str(&j))
            .transpose()?
            .ok_or(NotificationError::NotFound(notification_id))?;

        if !notification.read {
            notification.read = true;
            let updated_json = serde_json::to_string(&notification)?;
            
            // Create a pipe
            let mut pipe = redis::pipe();
            pipe.atomic()
                .set(&notification_key, &updated_json)
                .ignore();
            
            // Update the user sorted set
            let user_key = Self::user_key(notification.user_id);
            let json = serde_json::to_string(&notification)?;
            pipe.zadd(&user_key, json, notification.created_at.timestamp()).ignore();
            
            // Execute pipe
            pipe.query_async(&mut conn).await?;
            
            info!(self.logger, "Notification marked as read");
        }

        locks.retain(|&id| id != notification_id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        // Execute with circuit breaker if available
        if let Some(cb) = &self.circuit_breaker {
            return cb.execute("redis-notification-service", || async {
                self.delete_with_connection(notification_id).await
            }).await;
        }
        
        // Otherwise execute directly
        self.delete_with_connection(notification_id).await
    }
    
    // Internal helper to delete a notification
    async fn delete_with_connection(&self, notification_id: Uuid) -> Result<(), NotificationError> {
        let mut locks = self.locks.write().await;
        if locks.contains(&notification_id) {
            return Err(NotificationError::Internal("Concurrent modification".to_string()));
        }
        locks.push(notification_id);

        let mut conn = self.redis.get_async_connection().await?;
        let notification_key = Self::notification_key(notification_id);
        
        let json: Option<String> = conn.get(&notification_key).await?;
        let notification = json
            .map(|j| serde_json::from_str(&j))
            .transpose()?
            .ok_or(NotificationError::NotFound(notification_id))?;

        let user_key = Self::user_key(notification.user_id);
        redis::pipe()
            .atomic()
            .del(&notification_key)
            .zrem(&user_key, serde_json::to_string(&notification)?)
            .query_async(&mut conn)
            .await?;

        locks.retain(|&id| id != notification_id);
        info!(self.logger, "Notification deleted");
        Ok(())
    }
}

/// Notification creation helpers
pub struct NotificationBuilder;

impl NotificationBuilder {
    pub fn order_status(user_id: i32, order_id: &str, status: &str) -> Notification {
        Notification {
            id: Uuid::new_v4(),
            user_id,
            message: format!("Order {} status updated to: {}", order_id, status),
            notification_type: NotificationType::OrderStatus,
            read: false,
            created_at: Utc::now(),
        }
    }

    pub fn shipment_update(user_id: i32, shipment_id: &str, update: &str) -> Notification {
        Notification {
            id: Uuid::new_v4(),
            user_id,
            message: format!("Shipment {} update: {}", shipment_id, update),
            notification_type: NotificationType::ShipmentUpdate,
            read: false,
            created_at: Utc::now(),
        }
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use redis::aio::Connection;
    use slog::{o, Drain, Logger};
    use slog_term::TermDecorator;
    use tokio;

    fn setup_logger() -> Logger {
        let decorator = TermDecorator::new().build();
        let drain = slog_term::CompactFormat::new(decorator).build().fuse();
        Logger::root(drain, o!())
    }

    async fn setup_service() -> (RedisNotificationService, Connection) {
        let logger = setup_logger();
        let client = Client::open("redis://localhost:6379").unwrap();
        let conn = client.get_async_connection().await.unwrap();
        let service = RedisNotificationService::new("redis://localhost:6379", logger).await.unwrap();
        (service, conn)
    }

    #[tokio::test]
    async fn test_notification_lifecycle() {
        let (service, mut conn) = setup_service().await;
        let notification = NotificationBuilder::order_status(1, "ORDER123", "Shipped");

        // Send notification
        service.send(notification.clone()).await.unwrap();
        
        // Get notifications
        let notifications = service.get_user_notifications(1, 10).await.unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, notification.message);
        assert!(!notifications[0].read);

        // Mark as read
        service.mark_as_read(notification.id).await.unwrap();
        let notifications = service.get_user_notifications(1, 10).await.unwrap();
        assert!(notifications[0].read);

        // Delete
        service.delete(notification.id).await.unwrap();
        let notifications = service.get_user_notifications(1, 10).await.unwrap();
        assert_eq!(notifications.len(), 0);

        // Cleanup
        conn.del(Self::user_key(1)).await.unwrap();
        conn.del(Self::notification_key(notification.id)).await.unwrap();
    }

    #[tokio::test]
    async fn test_not_found() {
        let (service, mut conn) = setup_service().await;
        let fake_id = Uuid::new_v4();

        let result = service.mark_as_read(fake_id).await;
        assert!(matches!(result, Err(NotificationError::NotFound(_))));

        let result = service.delete(fake_id).await;
        assert!(matches!(result, Err(NotificationError::NotFound(_))));
    }
}