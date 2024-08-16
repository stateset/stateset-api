use async_trait::async_trait;;
use lapin::{
    Channel, BasicProperties, options::*, types::FieldTable, Error as LapinError, Consumer, message::Delivery,
};
use serde::{Serialize, Deserialize};
use futures_util::StreamExt;
use std::sync::Arc;
use tracing::{error, info, warn};
use tokio::time::{sleep, Duration};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessageQueueError {
    #[error("RabbitMQ error: {0}")]
    LapinError(#[from] LapinError),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Deserialization error: {0}")]
    DeserializationError(String),
}

#[async_trait]
pub trait MessageQueue: Send + Sync {
    async fn publish<T: Serialize + Send + Sync>(&self, queue: &str, message: &T) -> Result<(), MessageQueueError>;
    async fn consume<T, F>(&self, queue: &str, callback: F) -> Result<(), MessageQueueError>
    where
        T: for<'de> Deserialize<'de> + Send + 'static,
        F: Fn(T) -> Result<(), MessageQueueError> + Send + Sync + 'static;
}

pub struct RabbitMQ {
    channel: Arc<Channel>,
    retry_delay: Duration,
    max_retries: u32,
}

impl RabbitMQ {
    pub fn new(channel: Channel, retry_delay: Duration, max_retries: u32) -> Self {
        Self {
            channel: Arc::new(channel),
            retry_delay,
            max_retries,
        }
    }

    async fn declare_queue(&self, queue: &str) -> Result<(), LapinError> {
        self.channel
            .queue_declare(
                queue,
                QueueDeclareOptions::default(),
                FieldTable::default(),
            )
            .await
            .map(|_| ())
    }

    async fn handle_message<T, F>(
        delivery: Delivery,
        callback: &F,
        retry_delay: Duration,
        max_retries: u32,
    ) -> Result<(), MessageQueueError>
    where
        T: for<'de> Deserialize<'de> + Send + 'static,
        F: Fn(T) -> Result<(), MessageQueueError> + Send + Sync + 'static,
    {
        let message = serde_json::from_slice::<T>(&delivery.data)
            .map_err(|e| MessageQueueError::DeserializationError(e.to_string()))?;

        let mut retries = 0;
        while retries < max_retries {
            match callback(message.clone()) {
                Ok(_) => {
                    delivery.ack(BasicAckOptions::default()).await?;
                    return Ok(());
                }
                Err(e) => {
                    warn!("Error processing message, retrying: {}", e);
                    retries += 1;
                    sleep(retry_delay).await;
                }
            }
        }

        error!("Max retries reached, nacking message");
        delivery.nack(BasicNackOptions::default()).await?;
        Ok(())
    }

    async fn consume_internal<T, F>(
        channel: Arc<Channel>,
        queue: String,
        callback: Arc<F>,
        retry_delay: Duration,
        max_retries: u32,
    ) -> Result<(), MessageQueueError>
    where
        T: for<'de> Deserialize<'de> + Send + 'static,
        F: Fn(T) -> Result<(), MessageQueueError> + Send + Sync + 'static,
    {
        let mut consumer = channel
            .basic_consume(
                &queue,
                "consumer",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;

        while let Some(delivery) = consumer.next().await {
            if let Ok((_, delivery)) = delivery {
                if let Err(e) = Self::handle_message(delivery, &*callback, retry_delay, max_retries).await {
                    error!("Error handling message: {}", e);
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl MessageQueue for RabbitMQ {
    async fn publish<T: Serialize + Send + Sync>(&self, queue: &str, message: &T) -> Result<(), MessageQueueError> {
        self.declare_queue(queue).await?;
        let payload = serde_json::to_vec(message)?;
        self.channel
            .basic_publish(
                "",
                queue,
                BasicPublishOptions::default(),
                &payload,
                BasicProperties::default(),
            )
            .await?;
        info!("Message published to queue: {}", queue);
        Ok(())
    }

    async fn consume<T, F>(&self, queue: &str, callback: F) -> Result<(), MessageQueueError>
    where
        T: for<'de> Deserialize<'de> + Send + 'static,
        F: Fn(T) -> Result<(), MessageQueueError> + Send + Sync + 'static,
    {
        self.declare_queue(queue).await?;
        let callback = Arc::new(callback);
        let queue = queue.to_string();
        let channel = Arc::clone(&self.channel);
        let retry_delay = self.retry_delay;
        let max_retries = self.max_retries;
        tokio::spawn(async move {
            if let Err(e) = Self::consume_internal(channel, queue.clone(), callback, retry_delay, max_retries).await {
                error!("Error consuming from queue {}: {}", queue, e);
            }
        });
        Ok(())
    }
}

// Utility functions

pub async fn create_rabbitmq_connection(url: &str) -> Result<lapin::Connection, LapinError> {
    lapin::Connection::connect(url, lapin::ConnectionProperties::default()).await
}

pub async fn create_rabbitmq_channel(connection: &lapin::Connection) -> Result<lapin::Channel, LapinError> {
    connection.create_channel().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
    struct TestMessage {
        content: String,
    }

    #[tokio::test]
    async fn test_publish_and_consume() {
        let connection = create_rabbitmq_connection("amqp://localhost").await.unwrap();
        let channel = create_rabbitmq_channel(&connection).await.unwrap();
        let mq = RabbitMQ::new(channel, Duration::from_secs(1), 3);

        let queue = "test_queue";
        let test_message = TestMessage {
            content: "Hello, World!".to_string(),
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        mq.consume::<TestMessage, _>(queue, move |msg| {
            let tx = tx.clone();
            tokio::spawn(async move {
                tx.send(msg).await.unwrap();
            });
            Ok(())
        })
        .await
        .unwrap();

        mq.publish(queue, &test_message).await.unwrap();

        let received_message = rx.recv().await.unwrap();
        assert_eq!(received_message, test_message);
    }
}