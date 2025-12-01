// Cache module with fallback when Redis is not available

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use thiserror::Error;

pub mod middleware;
pub mod query;
pub mod strategy;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Redis error: {0}")]
    RedisError(String),
    #[error("Cache miss")]
    Miss,
    #[error("Cache operation failed: {0}")]
    OperationFailed(String),
    #[error("Invalid TTL")]
    InvalidTTL,
}

// In-memory cache implementation as fallback
#[derive(Debug, Clone)]
pub struct InMemoryCache {
    store: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    value: String,
    expires_at: Option<Instant>,
}

impl CacheEntry {
    fn new(value: String, ttl: Option<Duration>) -> Self {
        Self {
            value,
            expires_at: ttl.map(|d| Instant::now() + d),
        }
    }

    fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Instant::now() > expires_at
        } else {
            false
        }
    }
}

#[async_trait::async_trait]
pub trait CacheBackend: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError>;
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
    async fn exists(&self, key: &str) -> Result<bool, CacheError>;
    async fn clear(&self) -> Result<(), CacheError>;
}

impl InMemoryCache {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let store = self.store.read()
            .map_err(|e| CacheError::OperationFailed(format!("Lock poisoned: {}", e)))?;
        if let Some(entry) = store.get(key) {
            if entry.is_expired() {
                drop(store);
                let mut store = self.store.write()
                    .map_err(|e| CacheError::OperationFailed(format!("Lock poisoned: {}", e)))?;
                store.remove(key);
                Ok(None)
            } else {
                Ok(Some(entry.value.clone()))
            }
        } else {
            Ok(None)
        }
    }

    pub async fn set(
        &self,
        key: &str,
        value: &str,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        let mut store = self.store.write()
            .map_err(|e| CacheError::OperationFailed(format!("Lock poisoned: {}", e)))?;
        store.insert(key.to_string(), CacheEntry::new(value.to_string(), ttl));
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut store = self.store.write()
            .map_err(|e| CacheError::OperationFailed(format!("Lock poisoned: {}", e)))?;
        store.remove(key);
        Ok(())
    }

    pub async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let store = self.store.read()
            .map_err(|e| CacheError::OperationFailed(format!("Lock poisoned: {}", e)))?;
        if let Some(entry) = store.get(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    pub async fn clear(&self) -> Result<(), CacheError> {
        let mut store = self.store.write()
            .map_err(|e| CacheError::OperationFailed(format!("Lock poisoned: {}", e)))?;
        store.clear();
        Ok(())
    }
}

#[async_trait::async_trait]
impl CacheBackend for InMemoryCache {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        self.get(key).await
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError> {
        self.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        self.exists(key).await
    }

    async fn clear(&self) -> Result<(), CacheError> {
        self.clear().await
    }
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

pub type Cache = InMemoryCache;

// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub enabled: bool,
    pub redis_url: Option<String>,
    pub default_ttl_secs: Option<u64>,
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            redis_url: None,
            default_ttl_secs: Some(300), // 5 minutes default
            max_entries: 1000,
        }
    }
}

// Cache factory
pub struct CacheFactory;

impl CacheFactory {
    pub async fn create_cache(config: &CacheConfig) -> Result<Arc<dyn CacheBackend>, CacheError> {
        if !config.enabled {
            return Ok(Arc::new(InMemoryCache::new())); // Disabled cache
        }

        // Try Redis first if configured
        if let Some(redis_url) = &config.redis_url {
            match RedisCache::new(redis_url).await {
                Ok(redis_cache) => {
                    tracing::info!("Using Redis cache backend");
                    return Ok(Arc::new(redis_cache));
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect to Redis: {}, falling back to in-memory cache",
                        e
                    );
                }
            }
        }

        tracing::info!("Using in-memory cache backend");
        Ok(Arc::new(InMemoryCache::new()))
    }
}

// Redis cache implementation
#[derive(Clone)]
pub struct RedisCache {
    client: redis::Client,
}

impl RedisCache {
    pub async fn new(redis_url: &str) -> Result<Self, CacheError> {
        let client =
            redis::Client::open(redis_url).map_err(|e| CacheError::RedisError(e.to_string()))?;

        // Test the connection
        let mut conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CacheError::RedisError(format!("Failed to connect: {}", e)))?;

        // Ping to verify connection
        redis::cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| CacheError::RedisError(format!("Connection test failed: {}", e)))?;

        Ok(Self { client })
    }

    async fn get_connection(&self) -> Result<redis::aio::MultiplexedConnection, CacheError> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CacheError::RedisError(e.to_string()))
    }
}

#[async_trait::async_trait]
impl CacheBackend for RedisCache {
    async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        let mut conn = self.get_connection().await?;
        let result: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::RedisError(e.to_string()))?;
        Ok(result)
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError> {
        let mut conn = self.get_connection().await?;
        if let Some(ttl) = ttl {
            let ttl_secs = ttl.as_secs();
            if ttl_secs == 0 {
                return Err(CacheError::InvalidTTL);
            }
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl_secs)
                .arg(value)
                .query_async::<_, ()>(&mut conn)
                .await
                .map_err(|e| CacheError::RedisError(e.to_string()))?;
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .query_async::<_, ()>(&mut conn)
                .await
                .map_err(|e| CacheError::RedisError(e.to_string()))?;
        }
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.get_connection().await?;
        redis::cmd("DEL")
            .arg(key)
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| CacheError::RedisError(e.to_string()))?;
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool, CacheError> {
        let mut conn = self.get_connection().await?;
        let result: i32 = redis::cmd("EXISTS")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::RedisError(e.to_string()))?;
        Ok(result > 0)
    }

    async fn clear(&self) -> Result<(), CacheError> {
        let mut conn = self.get_connection().await?;
        redis::cmd("FLUSHDB")
            .query_async::<_, ()>(&mut conn)
            .await
            .map_err(|e| CacheError::RedisError(e.to_string()))?;
        Ok(())
    }
}
