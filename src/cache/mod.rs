use async_trait::async_trait;;
use redis::{Client as RedisClient, AsyncCommands};
use serde::{Serialize, Deserialize};
use std::time::Duration;
use tokio::sync::Mutex;

#[async_trait]
pub trait Cache: Send + Sync {
    async fn get(&self, key: &str) -> Option<String>;
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
}

pub struct MultiLevelCache {
    l1: Arc<InMemoryCache>,
    l2: Arc<RedisCache>,
}

impl MultiLevelCache {
    pub fn new(l1: InMemoryCache, l2: RedisCache) -> Self {
        Self {
            l1: Arc::new(l1),
            l2: Arc::new(l2),
        }
    }
}

#[async_trait]
impl Cache for MultiLevelCache {
    async fn get(&self, key: &str) -> Option<String> {
        if let Some(value) = self.l1.get(key).await {
            return Some(value);
        }
        if let Some(value) = self.l2.get(key).await {
            self.l1.set(key, &value, None).await.ok();
            return Some(value);
        }
        None
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), CacheError> {
        self.l1.set(key, value, ttl).await?;
        self.l2.set(key, value, ttl).await
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.l1.delete(key).await?;
        self.l2.delete(key).await
    }
}

pub struct InMemoryCache {
    cache: Mutex<LruCache<String, String>>,
}

impl InMemoryCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            cache: Mutex::new(LruCache::new(capacity)),
        }
    }
}

#[async_trait]
impl Cache for InMemoryCache {
    async fn get(&self, key: &str) -> Option<String> {
        self.cache.lock().await.get(key).cloned()
    }

    async fn set(&self, key: &str, value: &str, _ttl: Option<Duration>) -> Result<(), CacheError> {
        self.cache.lock().await.put(key.to_string(), value.to_string());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.cache.lock().await.pop(key);
        Ok(())
    }
}

pub struct RedisCache {
    client: Arc<RedisClient>,
}

impl RedisCache {
    pub fn new(client: Arc<RedisClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>, errors::ServiceError> {
        let mut conn = self.client.get_async_connection().await.map_err(errors::ServiceError::RedisError)?;
        let value: Option<String> = conn.get(key).await.map_err(errors::ServiceError::RedisError)?;
        match value {
            Some(v) => serde_json::from_str(&v).map_err(errors::ServiceError::JsonError).map(Some),
            None => Ok(None),
        }
    }

    async fn set<T: Serialize>(&self, key: &str, value: &T, expiration: Option<Duration>) -> Result<(), errors::ServiceError> {
        let mut conn = self.client.get_async_connection().await.map_err(errors::ServiceError::RedisError)?;
        let serialized = serde_json::to_string(value).map_err(errors::ServiceError::JsonError)?;
        match expiration {
            Some(exp) => conn.set_ex(key, serialized, exp.as_secs() as usize).await.map_err(errors::ServiceError::RedisError),
            None => conn.set(key, serialized).await.map_err(errors::ServiceError::RedisError),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), errors::ServiceError> {
        let mut conn = self.client.get_async_connection().await.map_err(errors::ServiceError::RedisError)?;
        conn.del(key).await.map_err(errors::ServiceError::RedisError)
    }
}
