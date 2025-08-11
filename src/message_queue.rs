/*!
 * # Message Queue Implementation
 *
 * This module provides message queue functionality for asynchronous
 * processing and event-driven architecture.
 */

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
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

/// Mock message queue for testing
#[cfg(test)]
pub struct MockMessageQueue {
    published_messages: Arc<Mutex<Vec<Message>>>,
}

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
