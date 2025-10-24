use crate::metrics::{CACHE_HITS, CACHE_MISSES, CACHE_OPERATIONS};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Cache miss")]
    Miss,
    #[error("Cache operation failed: {0}")]
    OperationFailed(String),
}

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

impl InMemoryCache {
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, CacheError> {
        CACHE_OPERATIONS.inc();
        let mut store = self.store.write().await;
        if let Some(entry) = store.get(key) {
            if entry.is_expired() {
                store.remove(key);
                CACHE_MISSES.inc();
                Ok(None)
            } else {
                CACHE_HITS.inc();
                Ok(Some(entry.value.clone()))
            }
        } else {
            CACHE_MISSES.inc();
            Ok(None)
        }
    }

    pub async fn set(
        &self,
        key: &str,
        value: &str,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        CACHE_OPERATIONS.inc();
        let mut store = self.store.write().await;
        store.insert(key.to_string(), CacheEntry::new(value.to_string(), ttl));
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), CacheError> {
        CACHE_OPERATIONS.inc();
        let mut store = self.store.write().await;
        store.remove(key);
        Ok(())
    }
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new()
    }
}
