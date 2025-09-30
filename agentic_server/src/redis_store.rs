use crate::errors::ServiceError;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, instrument};

/// Redis-backed session store
#[derive(Clone)]
pub struct RedisStore {
    client: Arc<redis::Client>,
}

impl RedisStore {
    pub async fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = redis::Client::open(redis_url)?;
        
        // Test connection
        let mut conn = client.get_multiplexed_async_connection().await?;
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        
        debug!("Redis connection established");
        
        Ok(Self {
            client: Arc::new(client),
        })
    }
    
    /// Save a value with TTL
    #[instrument(skip(self, value))]
    pub async fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<Duration>,
    ) -> Result<(), ServiceError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| ServiceError::CacheError(format!("Redis connection failed: {}", e)))?;
        
        let data = serde_json::to_string(value)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;
        
        if let Some(ttl) = ttl {
            conn.set_ex(key, data, ttl.as_secs() as u64).await
                .map_err(|e| ServiceError::CacheError(format!("Redis SET failed: {}", e)))?;
        } else {
            conn.set(key, data).await
                .map_err(|e| ServiceError::CacheError(format!("Redis SET failed: {}", e)))?;
        }
        
        debug!("Stored key: {} with TTL: {:?}", key, ttl);
        Ok(())
    }
    
    /// Get a value
    #[instrument(skip(self))]
    pub async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        key: &str,
    ) -> Result<Option<T>, ServiceError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| ServiceError::CacheError(format!("Redis connection failed: {}", e)))?;
        
        let data: Option<String> = conn.get(key).await
            .map_err(|e| ServiceError::CacheError(format!("Redis GET failed: {}", e)))?;
        
        match data {
            Some(json) => {
                let value = serde_json::from_str(&json)
                    .map_err(|e| ServiceError::ParseError(e.to_string()))?;
                debug!("Retrieved key: {}", key);
                Ok(Some(value))
            }
            None => {
                debug!("Key not found: {}", key);
                Ok(None)
            }
        }
    }
    
    /// Delete a value
    #[instrument(skip(self))]
    pub async fn delete(&self, key: &str) -> Result<(), ServiceError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| ServiceError::CacheError(format!("Redis connection failed: {}", e)))?;
        
        conn.del(key).await
            .map_err(|e| ServiceError::CacheError(format!("Redis DEL failed: {}", e)))?;
        
        debug!("Deleted key: {}", key);
        Ok(())
    }
    
    /// Check if key exists
    #[instrument(skip(self))]
    pub async fn exists(&self, key: &str) -> Result<bool, ServiceError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| ServiceError::CacheError(format!("Redis connection failed: {}", e)))?;
        
        let exists: bool = conn.exists(key).await
            .map_err(|e| ServiceError::CacheError(format!("Redis EXISTS failed: {}", e)))?;
        
        Ok(exists)
    }
    
    /// Set a value only if it doesn't exist (for idempotency)
    #[instrument(skip(self, value))]
    pub async fn set_nx<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<bool, ServiceError> {
        let mut conn = self.client.get_multiplexed_async_connection().await
            .map_err(|e| ServiceError::CacheError(format!("Redis connection failed: {}", e)))?;
        
        let data = serde_json::to_string(value)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;
        
        // SET NX EX (set if not exists with expiry)
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(data)
            .arg("NX")
            .arg("EX")
            .arg(ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| ServiceError::CacheError(format!("Redis SETNX failed: {}", e)))?;
        
        Ok(result.is_some())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Missing signature header")]
    MissingSignature,
    
    #[error("Missing timestamp header")]
    MissingTimestamp,
    
    #[error("Invalid format")]
    InvalidFormat,
    
    #[error("Invalid timestamp")]
    InvalidTimestamp,
    
    #[error("Timestamp too old")]
    TimestampTooOld,
    
    #[error("Invalid signature")]
    InvalidSignature,
}

// Tests require running Redis instance, so they're disabled by default
// To test Redis functionality, run integration tests with a Redis container 