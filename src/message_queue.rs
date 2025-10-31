/*!
 * # Message Queue Implementation
 *
 * This module provides message queue functionality for asynchronous
 * processing and event-driven architecture.
 */

use async_trait::async_trait;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use uuid::Uuid;

/// Message queue errors
#[derive(Error, Debug)]
pub enum MessageQueueError {
    #[error("Queue is full")]
    QueueFull,
    #[error("Queue is empty")]
    QueueEmpty,
    #[error("Serialization error: {0}")]
    SerializationError(String),
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

/// Message envelope for queue items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub topic: String,
    pub payload: serde_json::Value,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl Message {
    pub fn new(topic: String, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            topic,
            payload,
            timestamp: chrono::Utc::now(),
            retry_count: 0,
            max_retries: 3,
        }
    }
}

/// Message queue trait for different implementations
#[async_trait]
pub trait MessageQueue: Send + Sync {
    async fn publish(&self, message: Message) -> Result<(), MessageQueueError>;
    async fn subscribe(&self, topic: &str) -> Result<Option<Message>, MessageQueueError>;
    async fn ack(&self, message_id: &Uuid) -> Result<(), MessageQueueError>;
    async fn nack(&self, message_id: &Uuid) -> Result<(), MessageQueueError>;
}

/// In-memory message queue implementation
#[derive(Debug)]
pub struct InMemoryMessageQueue {
    queues: Arc<Mutex<std::collections::HashMap<String, VecDeque<Message>>>>,
    max_size: usize,
}

impl InMemoryMessageQueue {
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(std::collections::HashMap::new())),
            max_size: 1000,
        }
    }

    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            queues: Arc::new(Mutex::new(std::collections::HashMap::new())),
            max_size,
        }
    }
}

#[async_trait]
impl MessageQueue for InMemoryMessageQueue {
    async fn publish(&self, message: Message) -> Result<(), MessageQueueError> {
        let mut queues = self.queues.lock().unwrap();
        let queue = queues
            .entry(message.topic.clone())
            .or_insert_with(VecDeque::new);

        if queue.len() >= self.max_size {
            return Err(MessageQueueError::QueueFull);
        }

        queue.push_back(message);
        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<Option<Message>, MessageQueueError> {
        let mut queues = self.queues.lock().unwrap();
        if let Some(queue) = queues.get_mut(topic) {
            Ok(queue.pop_front())
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, _message_id: &Uuid) -> Result<(), MessageQueueError> {
        // In-memory implementation doesn't need explicit acking
        Ok(())
    }

    async fn nack(&self, _message_id: &Uuid) -> Result<(), MessageQueueError> {
        // In-memory implementation doesn't support nacking
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct InFlightRecord {
    topic: String,
    payload: String,
}

/// Redis-backed message queue implementation for cross-instance durability.
#[derive(Debug)]
pub struct RedisMessageQueue {
    client: Arc<redis::Client>,
    namespace: String,
    block_timeout: Duration,
    inflight: Arc<Mutex<HashMap<Uuid, InFlightRecord>>>,
}

impl RedisMessageQueue {
    const DEFAULT_NAMESPACE: &'static str = "stateset:mq";

    pub async fn new(
        client: Arc<redis::Client>,
        namespace: impl Into<String>,
        block_timeout: Duration,
    ) -> Result<Self, MessageQueueError> {
        let namespace = namespace.into();
        let namespace = if namespace.trim().is_empty() {
            Self::DEFAULT_NAMESPACE.to_string()
        } else {
            namespace
        };

        let queue = Self {
            client,
            namespace,
            block_timeout,
            inflight: Arc::new(Mutex::new(HashMap::new())),
        };

        queue.recover_stalled_messages().await?;

        Ok(queue)
    }

    fn queue_key(&self, topic: &str) -> String {
        format!("{}:queue:{}", self.namespace, topic)
    }

    fn inflight_key(&self) -> String {
        format!("{}:topics", self.namespace)
    }

    async fn recover_stalled_messages(&self) -> Result<(), MessageQueueError> {
        let mut conn = self
            .client
            .get_async_connection()
            .await
            .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

        let topics: Vec<String> = redis::cmd("SMEMBERS")
            .arg(self.inflight_key())
            .query_async(&mut conn)
            .await
            .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

        for topic in topics {
            let processing_key = self.processing_key(&topic);
            let queue_key = self.queue_key(&topic);

            loop {
                let payload: Option<String> = redis::cmd("RPOPLPUSH")
                    .arg(&processing_key)
                    .arg(&queue_key)
                    .query_async(&mut conn)
                    .await
                    .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

                if payload.is_none() {
                    break;
                }
            }
        }

        Ok(())
    }

    fn processing_key(&self, topic: &str) -> String {
        format!("{}:processing:{}", self.namespace, topic)
    }

    fn block_timeout_secs(&self) -> usize {
        let secs = self.block_timeout.as_secs();
        if secs == 0 {
            1
        } else {
            secs as usize
        }
    }
}

#[async_trait]
impl MessageQueue for RedisMessageQueue {
    async fn publish(&self, message: Message) -> Result<(), MessageQueueError> {
        let mut conn = self
            .client
            .get_async_connection()
            .await
            .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

        let payload = serde_json::to_string(&message)
            .map_err(|e| MessageQueueError::SerializationError(e.to_string()))?;
        let queue_key = self.queue_key(&message.topic);

        redis::pipe()
            .atomic()
            .cmd("LPUSH")
            .arg(&queue_key)
            .arg(&payload)
            .cmd("SADD")
            .arg(self.inflight_key())
            .arg(&message.topic)
            .query_async(&mut conn)
            .await
            .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

        Ok(())
    }

    async fn subscribe(&self, topic: &str) -> Result<Option<Message>, MessageQueueError> {
        let mut conn = self
            .client
            .get_async_connection()
            .await
            .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

        let queue_key = self.queue_key(topic);
        let processing_key = self.processing_key(topic);

        let result: Option<String> = redis::cmd("BRPOPLPUSH")
            .arg(&queue_key)
            .arg(&processing_key)
            .arg(self.block_timeout_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

        if let Some(payload) = result {
            let message: Message = serde_json::from_str(&payload)
                .map_err(|e| MessageQueueError::SerializationError(e.to_string()))?;

            self.inflight.lock().unwrap().insert(
                message.id,
                InFlightRecord {
                    topic: topic.to_string(),
                    payload,
                },
            );

            Ok(Some(message))
        } else {
            Ok(None)
        }
    }

    async fn ack(&self, message_id: &Uuid) -> Result<(), MessageQueueError> {
        let record = {
            let mut inflight = self.inflight.lock().unwrap();
            inflight.remove(message_id)
        };

        if let Some(record) = record {
            let mut conn = self
                .client
                .get_async_connection()
                .await
                .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

            redis::cmd("LREM")
                .arg(self.processing_key(&record.topic))
                .arg(1)
                .arg(&record.payload)
                .query_async(&mut conn)
                .await
                .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;
        }

        Ok(())
    }

    async fn nack(&self, message_id: &Uuid) -> Result<(), MessageQueueError> {
        let record = {
            let mut inflight = self.inflight.lock().unwrap();
            inflight.remove(message_id)
        };

        if let Some(record) = record {
            let mut conn = self
                .client
                .get_async_connection()
                .await
                .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;

            redis::pipe()
                .atomic()
                .cmd("LREM")
                .arg(self.processing_key(&record.topic))
                .arg(1)
                .arg(&record.payload)
                .cmd("RPUSH")
                .arg(self.queue_key(&record.topic))
                .arg(&record.payload)
                .query_async(&mut conn)
                .await
                .map_err(|e| MessageQueueError::ConnectionError(e.to_string()))?;
        }

        Ok(())
    }
}

/// Mock message queue for testing
#[cfg(all(test, feature = "mock-tests"))]
pub struct MockMessageQueue {
    published_messages: Arc<Mutex<Vec<Message>>>,
}

#[cfg(all(test, feature = "mock-tests"))]
impl MockMessageQueue {
    pub fn new() -> Self {
        Self {
            published_messages: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn get_published_messages(&self) -> Vec<Message> {
        self.published_messages.lock().unwrap().clone()
    }
}

#[cfg(all(test, feature = "mock-tests"))]
#[async_trait]
impl MessageQueue for MockMessageQueue {
    async fn publish(&self, message: Message) -> Result<(), MessageQueueError> {
        self.published_messages.lock().unwrap().push(message);
        Ok(())
    }

    async fn subscribe(&self, _topic: &str) -> Result<Option<Message>, MessageQueueError> {
        Ok(None)
    }

    async fn ack(&self, _message_id: &Uuid) -> Result<(), MessageQueueError> {
        Ok(())
    }

    async fn nack(&self, _message_id: &Uuid) -> Result<(), MessageQueueError> {
        Ok(())
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_queue() {
        let queue = InMemoryMessageQueue::new();
        let message = Message::new(
            "test_topic".to_string(),
            serde_json::json!({"test": "data"}),
        );

        // Publish message
        assert!(queue.publish(message.clone()).await.is_ok());

        // Subscribe and receive message
        let received = queue.subscribe("test_topic").await.unwrap();
        assert!(received.is_some());
        assert_eq!(received.unwrap().topic, "test_topic");

        // Queue should be empty now
        let empty = queue.subscribe("test_topic").await.unwrap();
        assert!(empty.is_none());
    }
}
