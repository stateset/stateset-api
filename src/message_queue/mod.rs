use async_trait::async_trait;
use lapin::{
    Channel, BasicProperties, options::*, types::FieldTable, Error as LapinError, Consumer, message::Delivery,
};
use serde::{Serialize, Deserialize};
use futures_util::StreamExt;
use std::sync::Arc;
use tracing::{error, info};
use redis::{Commands, ConnectionLike};
use std::time::Duration;
use tokio::time::interval;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub trait MessageQueue {
    async fn publish<T: Serialize>(&self, queue: &str, message: &T) -> Result<(), LapinError>;
    async fn consume<T: for<'de> Deserialize<'de>>(&self, queue: &str, callback: impl Fn(T) + Send + Sync + 'static) -> Result<(), LapinError>;
}

pub struct RabbitMQ {
    channel: Arc<Channel>,
}

impl RabbitMQ {
    pub fn new(channel: Channel) -> Self {
        Self { channel: Arc::new(channel) }
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

    async fn handle_message<T: for<'de> Deserialize<'de>>(
        delivery: Delivery,
        callback: &impl Fn(T),
    ) -> Result<(), LapinError> {
        match serde_json::from_slice::<T>(&delivery.data) {
            Ok(message) => {
                callback(message);
                delivery.ack(BasicAckOptions::default()).await?;
            }
            Err(e) => {
                error!("Failed to deserialize message: {}", e);
                delivery.nack(BasicNackOptions::default()).await?;
            }
        }
        Ok(())
    }

    async fn consume_internal<T: for<'de> Deserialize<'de>>(
        channel: Arc<Channel>,
        queue: String,
        callback: Arc<impl Fn(T) + Send + Sync + 'static>,
    ) -> Result<(), LapinError> {
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
                if let Err(e) = Self::handle_message(delivery, &*callback).await {
                    error!("Error handling message: {}", e);
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl MessageQueue for RabbitMQ {
    async fn publish<T: Serialize>(&self, queue: &str, message: &T) -> Result<(), LapinError> {
        self.declare_queue(queue).await?;
        let payload = serde_json::to_vec(message)
            .map_err(|e| LapinError::InvalidChannelState(e.to_string()))?;
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

    async fn consume<T: for<'de> Deserialize<'de>>(
        &self,
        queue: &str,
        callback: impl Fn(T) + Send + Sync + 'static,
    ) -> Result<(), LapinError> {
        self.declare_queue(queue).await?;
        let callback = Arc::new(callback);
        let queue = queue.to_string();
        let channel = Arc::clone(&self.channel);
        tokio::spawn(Self::consume_internal(channel, queue, callback));
        Ok(())
    }
}
