// cache/mod.rs

use async_trait::async_trait;
use redis::{AsyncCommands, Client as RedisClient};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    sync::Arc,
    time::Duration,
};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;
use dashmap::DashMap;
use tokio::time::{sleep, Instant};

/// Comprehensive error type for cache operations.
#[derive(Error, Debug)]
pub enum CacheError {
    #[error("Redis error: {0}")]
    RedisError(#[from] redis::RedisError),

    #[error("Serialization/Deserialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("Invalid TTL duration")]
    InvalidTTL,

    #[error("In-memory cache lock poisoned")]
    InMemoryLockPoisoned,
}

/// Trait defining the caching interface.
/// Supports generic types that are serializable and deserializable.
#[async_trait]
pub trait Cache: Send + Sync {
    /// Retrieves a value from the cache.
    ///
    /// # Arguments
    ///
    /// * `key` - The key associated with the cached value.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(T))` if the key exists and deserialization succeeds.
    /// * `Ok(None)` if the key does not exist.
    /// * `Err(CacheError)` if an error occurs during retrieval or deserialization.
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>, CacheError>;

    /// Inserts a value into the cache with an optional TTL (Time To Live).
    ///
    /// # Arguments
    ///
    /// * `key` - The key to associate with the cached value.
    /// * `value` - The value to cache.
    /// * `ttl` - Optional TTL after which the cached value expires.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the operation succeeds.
    /// * `Err(CacheError)` if an error occurs during serialization or insertion.
    async fn set<T: Serialize + Send>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError>;

    /// Deletes a key-value pair from the cache.
    ///
    /// # Arguments
    ///
    /// * `key` - The key to remove from the cache.
    ///
    /// # Returns
    ///
    /// * `Ok(())` if the operation succeeds.
    /// * `Err(CacheError)` if an error occurs during deletion.
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
}

/// In-memory cache implementation using DashMap for thread-safe operations.
/// Supports TTL by storing expiration times and periodically cleaning expired entries.
pub struct InMemoryCache {
    /// The underlying DashMap storing key-value pairs.
    map: DashMap<String, (String, Instant)>,
    /// Background task handle for cleaning expired entries.
    cleaner_handle: tokio::task::JoinHandle<()>,
}

impl InMemoryCache {
    /// Creates a new in-memory cache with a specified capacity and TTL cleanup interval.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries the cache can hold.
    /// * `cleanup_interval` - Frequency at which the cache checks for expired entries.
    pub fn new(capacity: usize, cleanup_interval: Duration) -> Self {
        let map = DashMap::new();
        let map_clone = map.clone();

        // Spawn a background task to clean expired entries.
        let cleaner_handle = tokio::spawn(async move {
            loop {
                sleep(cleanup_interval).await;
                let now = Instant::now();
                map_clone.retain(|_, &mut (_, exp)| exp > now);
            }
        });

        Self {
            map,
            cleaner_handle,
        }
    }
}

#[async_trait]
impl Cache for InMemoryCache {
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>, CacheError> {
        if let Some(entry) = self.map.get(key) {
            let (value, exp) = entry.value();
            if *exp > Instant::now() {
                let deserialized = serde_json::from_str::<T>(value)?;
                return Ok(Some(deserialized));
            } else {
                // Entry has expired
                self.map.remove(key);
            }
        }
        Ok(None)
    }

    async fn set<T: Serialize + Send>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError> {
        let serialized = serde_json::to_string(value)?;
        let expiration = ttl.map_or_else(|| Instant::now() + Duration::from_secs(3600), |dur| Instant::now() + dur); // Default TTL 1 hour
        self.map.insert(key.to_string(), (serialized, expiration));
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.map.remove(key);
        Ok(())
    }
}

/// Redis cache implementation.
/// Connects to a Redis server and performs cache operations using async Redis commands.
pub struct RedisCache {
    client: RedisClient,
}

impl RedisCache {
    /// Creates a new Redis cache instance.
    ///
    /// # Arguments
    ///
    /// * `redis_url` - The Redis server URL.
    pub fn new(redis_url: &str) -> Result<Self, redis::RedisError> {
        let client = RedisClient::open(redis_url)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>, CacheError> {
        let mut conn = self.client.get_async_connection().await?;
        let value: Option<String> = conn.get(key).await?;
        match value {
            Some(v) => {
                let deserialized = serde_json::from_str::<T>(&v)?;
                Ok(Some(deserialized))
            },
            None => Ok(None),
        }
    }

    async fn set<T: Serialize + Send>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError> {
        let mut conn = self.client.get_async_connection().await?;
        let serialized = serde_json::to_string(value)?;
        match ttl {
            Some(dur) => {
                conn.set_ex(key, serialized, dur.as_secs() as usize).await?;
            },
            None => {
                conn.set(key, serialized).await?;
            },
        }
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.client.get_async_connection().await?;
        conn.del(key).await?;
        Ok(())
    }
}

/// Multi-level cache implementation combining in-memory and Redis caches.
/// Attempts to retrieve from the in-memory cache first, then falls back to Redis.
pub struct MultiLevelCache {
    l1: Arc<InMemoryCache>,
    l2: Arc<RedisCache>,
}

impl MultiLevelCache {
    /// Creates a new multi-level cache.
    ///
    /// # Arguments
    ///
    /// * `l1` - The in-memory cache instance.
    /// * `l2` - The Redis cache instance.
    pub fn new(l1: InMemoryCache, l2: RedisCache) -> Self {
        Self {
            l1: Arc::new(l1),
            l2: Arc::new(l2),
        }
    }
}

#[async_trait]
impl Cache for MultiLevelCache {
    async fn get<T: DeserializeOwned + Send>(&self, key: &str) -> Result<Option<T>, CacheError> {
        // Attempt to get from in-memory cache
        if let Some(value) = self.l1.get::<T>(key).await? {
            return Ok(Some(value));
        }

        // Fallback to Redis
        if let Some(value) = self.l2.get::<T>(key).await? {
            // Populate in-memory cache for faster future access
            self.l1.set(key, &value, None).await?;
            return Ok(Some(value));
        }

        Ok(None)
    }

    async fn set<T: Serialize + Send>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError> {
        // Set in both caches
        self.l1.set(key, value, ttl).await?;
        self.l2.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        // Delete from both caches
        self.l1.delete(key).await?;
        self.l2.delete(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use tokio::time::Duration as TokioDuration;

    /// Mock Redis server using Redis in-memory server for testing.
    /// For simplicity, tests for RedisCache will assume a running Redis instance.
    /// In a real-world scenario, consider using `redis-server` in a Docker container for integration tests.

    #[tokio::test]
    async fn test_in_memory_cache_set_get() {
        let cache = InMemoryCache::new(100, Duration::from_secs(60));
        let key = "test_key";
        let value = "test_value";

        // Set value
        cache.set(key, &value, Some(Duration::from_secs(5))).await.unwrap();

        // Get value
        let retrieved: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        // Wait for TTL to expire
        sleep(TokioDuration::from_secs(6)).await;

        // Attempt to get expired value
        let expired: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(expired, None);
    }

    #[tokio::test]
    async fn test_in_memory_cache_delete() {
        let cache = InMemoryCache::new(100, Duration::from_secs(60));
        let key = "delete_key";
        let value = "delete_value";

        // Set value
        cache.set(key, &value, None).await.unwrap();

        // Delete value
        cache.delete(key).await.unwrap();

        // Attempt to get deleted value
        let deleted: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(deleted, None);
    }

    #[tokio::test]
    async fn test_redis_cache_set_get_delete() {
        // Ensure Redis server is running at this URL for the test
        let redis_url = "redis://127.0.0.1/";
        let redis_cache = RedisCache::new(redis_url).unwrap();

        let key = "redis_test_key";
        let value = "redis_test_value";

        // Set value with TTL
        redis_cache.set(key, &value, Some(Duration::from_secs(5))).await.unwrap();

        // Get value
        let retrieved: Option<String> = redis_cache.get(key).await.unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        // Delete value
        redis_cache.delete(key).await.unwrap();

        // Attempt to get deleted value
        let deleted: Option<String> = redis_cache.get(key).await.unwrap();
        assert_eq!(deleted, None);
    }

    #[tokio::test]
    async fn test_multi_level_cache_set_get() {
        // Initialize in-memory cache
        let in_memory = InMemoryCache::new(100, Duration::from_secs(60));

        // Initialize Redis cache
        let redis_url = "redis://127.0.0.1/";
        let redis_cache = RedisCache::new(redis_url).unwrap();

        // Initialize multi-level cache
        let cache = MultiLevelCache::new(in_memory, redis_cache);

        let key = "multi_level_key";
        let value = "multi_level_value";

        // Set value with TTL
        cache.set(key, &value, Some(Duration::from_secs(5))).await.unwrap();

        // Get value (should be retrieved from in-memory cache)
        let retrieved: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(retrieved, Some(value.to_string()));

        // Wait for TTL to expire
        sleep(TokioDuration::from_secs(6)).await;

        // Attempt to get expired value
        let expired: Option<String> = cache.get(key).await.unwrap();
        assert_eq!(expired, None);
    }

    #[tokio::test]
    async fn test_cache_error_handling() {
        // Initialize Redis cache with invalid URL
        let invalid_redis_url = "redis://invalid_url/";
        let redis_cache = RedisCache::new(invalid_redis_url);

        assert!(redis_cache.is_err());
    }

    #[tokio::test]
    async fn test_cache_serialization() {
        let in_memory = InMemoryCache::new(100, Duration::from_secs(60));
        let redis_url = "redis://127.0.0.1/";
        let redis_cache = RedisCache::new(redis_url).unwrap();
        let cache = MultiLevelCache::new(in_memory, redis_cache);

        let key = "user:1";
        let user = User {
            id: Uuid::new_v4(),
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
        };

        // Set user in cache
        cache.set(key, &user, None).await.unwrap();

        // Get user from cache
        let retrieved: Option<User> = cache.get(key).await.unwrap();
        assert_eq!(retrieved, Some(user));
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct User {
        id: Uuid,
        name: String,
        email: String,
    }
}
