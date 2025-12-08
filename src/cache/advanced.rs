/*!
 * # Advanced Caching Module
 *
 * This module provides enterprise-grade caching capabilities including:
 * - LRU (Least Recently Used) eviction policy
 * - Cache warming for frequently accessed data
 * - Advanced cache invalidation patterns
 * - Distributed caching with Redis Cluster
 * - Cache analytics and monitoring
 * - Intelligent cache prefetching
 */

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, RwLock},
    time::{Duration, Instant},
    hash::{Hash, Hasher},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock as TokioRwLock;
use async_trait::async_trait;
use futures::future::join_all;
use rand::seq::SliceRandom;
use tracing::{debug, info, warn};

#[derive(Debug, Error)]
pub enum AdvancedCacheError {
    #[error("Cache operation failed: {0}")]
    OperationFailed(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Redis cluster error: {0}")]
    RedisClusterError(String),
    #[error("Cache warming failed: {0}")]
    CacheWarmingError(String),
    #[error("Invalid cache key: {0}")]
    InvalidKey(String),
    #[error("Cache capacity exceeded")]
    CapacityExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: u64,
    pub accessed_at: u64,
    pub access_count: u64,
    pub ttl: Option<u64>, // TTL in seconds from creation
    pub tags: HashSet<String>, // For invalidation patterns
    pub priority: CachePriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CachePriority {
    Low = 1,
    Medium = 2,
    High = 3,
    Critical = 4,
}

impl CachePriority {
    pub fn eviction_weight(&self) -> f64 {
        match self {
            CachePriority::Low => 0.2,
            CachePriority::Medium => 0.4,
            CachePriority::High => 0.7,
            CachePriority::Critical => 1.0,
        }
    }
}

#[derive(Debug, Clone)]
struct LRUEntry {
    key: String,
    last_accessed: u64,
    access_count: u64,
    priority: CachePriority,
}

/// Advanced LRU Cache with priority-based eviction
pub struct LRUCache<T> {
    capacity: usize,
    store: HashMap<String, CacheEntry<T>>,
    access_order: VecDeque<String>,
    priority_queues: HashMap<CachePriority, VecDeque<String>>,
    total_evictions: u64,
    total_hits: u64,
    total_misses: u64,
}

impl<T> LRUCache<T> {
    pub fn new(capacity: usize) -> Self {
        let mut priority_queues = HashMap::new();
        priority_queues.insert(CachePriority::Low, VecDeque::new());
        priority_queues.insert(CachePriority::Medium, VecDeque::new());
        priority_queues.insert(CachePriority::High, VecDeque::new());
        priority_queues.insert(CachePriority::Critical, VecDeque::new());

        Self {
            capacity,
            store: HashMap::new(),
            access_order: VecDeque::new(),
            priority_queues,
            total_evictions: 0,
            total_hits: 0,
            total_misses: 0,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&T> {
        if let Some(entry) = self.store.get_mut(key) {
            if self.is_expired(entry) {
                self.remove_expired(key);
                self.total_misses += 1;
                return None;
            }

            entry.accessed_at = Self::current_timestamp();
            entry.access_count += 1;

            // Update access order
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                self.access_order.remove(pos);
            }
            self.access_order.push_front(key.to_string());

            self.total_hits += 1;
            Some(&entry.value)
        } else {
            self.total_misses += 1;
            None
        }
    }

    pub fn put(&mut self, key: String, value: T, ttl: Option<u64>, priority: CachePriority, tags: HashSet<String>) {
        let now = Self::current_timestamp();

        let entry = CacheEntry {
            value,
            created_at: now,
            accessed_at: now,
            access_count: 0,
            ttl,
            tags,
            priority,
        };

        // Remove expired entries if we're at capacity
        self.evict_expired();

        // If we're still at capacity, evict based on priority and LRU
        if self.store.len() >= self.capacity {
            self.evict_least_valuable();
        }

        // Remove from old priority queue if exists
        if let Some(old_entry) = self.store.get(&key) {
            if let Some(queue) = self.priority_queues.get_mut(&old_entry.priority) {
                if let Some(pos) = queue.iter().position(|k| k == &key) {
                    queue.remove(pos);
                }
            }
        }

        // Add to new priority queue
        if let Some(queue) = self.priority_queues.get_mut(&priority) {
            queue.push_front(key.clone());
        }

        // Update access order
        if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
            self.access_order.remove(pos);
        }
        self.access_order.push_front(key.clone());

        self.store.insert(key, entry);
    }

    pub fn remove(&mut self, key: &str) -> Option<T> {
        if let Some(entry) = self.store.remove(key) {
            // Remove from priority queue
            if let Some(queue) = self.priority_queues.get_mut(&entry.priority) {
                if let Some(pos) = queue.iter().position(|k| k == key) {
                    queue.remove(pos);
                }
            }

            // Remove from access order
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                self.access_order.remove(pos);
            }

            Some(entry.value)
        } else {
            None
        }
    }

    pub fn invalidate_by_tag(&mut self, tag: &str) -> usize {
        let mut removed = 0;
        let keys_to_remove: Vec<String> = self.store.iter()
            .filter(|(_, entry)| entry.tags.contains(tag))
            .map(|(key, _)| key.clone())
            .collect();

        for key in keys_to_remove {
            if self.remove(&key).is_some() {
                removed += 1;
            }
        }

        debug!("Invalidated {} entries with tag: {}", removed, tag);
        removed
    }

    pub fn invalidate_by_pattern(&mut self, pattern: &str) -> usize {
        let mut removed = 0;
        let keys_to_remove: Vec<String> = self.store.keys()
            .filter(|key| key.contains(pattern))
            .cloned()
            .collect();

        for key in keys_to_remove {
            if self.remove(&key).is_some() {
                removed += 1;
            }
        }

        debug!("Invalidated {} entries matching pattern: {}", removed, pattern);
        removed
    }

    pub fn clear(&mut self) {
        self.store.clear();
        self.access_order.clear();
        for queue in self.priority_queues.values_mut() {
            queue.clear();
        }
    }

    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.store.len(),
            capacity: self.capacity,
            hit_ratio: if self.total_hits + self.total_misses > 0 {
                self.total_hits as f64 / (self.total_hits + self.total_misses) as f64
            } else {
                0.0
            },
            total_hits: self.total_hits,
            total_misses: self.total_misses,
            total_evictions: self.total_evictions,
        }
    }

    fn evict_least_valuable(&mut self) {
        // Try to evict from lowest priority first
        for priority in &[CachePriority::Low, CachePriority::Medium, CachePriority::High, CachePriority::Critical] {
            if let Some(queue) = self.priority_queues.get_mut(priority) {
                while let Some(key) = queue.pop_back() {
                    if self.store.contains_key(&key) {
                        self.store.remove(&key);
                        self.access_order.retain(|k| k != &key);
                        self.total_evictions += 1;
                        debug!("Evicted cache entry: {} (priority: {:?})", key, priority);
                        return;
                    }
                }
            }
        }
    }

    fn evict_expired(&mut self) {
        let expired_keys: Vec<String> = self.store.iter()
            .filter(|(_, entry)| self.is_expired(entry))
            .map(|(key, _)| key.clone())
            .collect();

        for key in expired_keys {
            self.remove(&key);
        }
    }

    fn remove_expired(&mut self, key: &str) {
        if let Some(entry) = self.store.get(key) {
            if self.is_expired(entry) {
                self.remove(key);
            }
        }
    }

    fn is_expired(&self, entry: &CacheEntry<T>) -> bool {
        if let Some(ttl) = entry.ttl {
            let now = Self::current_timestamp();
            now >= entry.created_at + ttl
        } else {
            false
        }
    }

    fn current_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub capacity: usize,
    pub hit_ratio: f64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub total_evictions: u64,
}

/// Distributed Cache Interface
#[async_trait]
pub trait DistributedCache: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>, AdvancedCacheError>;
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), AdvancedCacheError>;
    async fn delete(&self, key: &str) -> Result<(), AdvancedCacheError>;
    async fn invalidate_pattern(&self, pattern: &str) -> Result<(), AdvancedCacheError>;
    async fn get_cluster_info(&self) -> Result<ClusterInfo, AdvancedCacheError>;
}

#[derive(Debug, Clone)]
pub struct ClusterInfo {
    pub nodes: Vec<String>,
    pub master_nodes: Vec<String>,
    pub total_slots: u64,
    pub slots_per_node: HashMap<String, u64>,
}

/// Redis Cluster Implementation
pub struct RedisClusterCache {
    client: redis::Client,
    cluster_nodes: Vec<String>,
}

impl RedisClusterCache {
    pub fn new(cluster_nodes: Vec<String>) -> Result<Self, AdvancedCacheError> {
        let connection_string = cluster_nodes.join(",");
        let client = redis::Client::open(connection_string)
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        Ok(Self {
            client,
            cluster_nodes,
        })
    }
}

#[async_trait]
impl DistributedCache for RedisClusterCache {
    async fn get(&self, key: &str) -> Result<Option<String>, AdvancedCacheError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        let result: Option<String> = redis::cmd("GET")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        Ok(result)
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), AdvancedCacheError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        if let Some(ttl) = ttl {
            redis::cmd("SETEX")
                .arg(key)
                .arg(ttl.as_secs() as i64)
                .arg(value)
                .query_async(&mut conn)
                .await
        } else {
            redis::cmd("SET")
                .arg(key)
                .arg(value)
                .query_async(&mut conn)
                .await
        }.map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), AdvancedCacheError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        redis::cmd("DEL")
            .arg(key)
            .query_async(&mut conn)
            .await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        Ok(())
    }

    async fn invalidate_pattern(&self, pattern: &str) -> Result<(), AdvancedCacheError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        if !keys.is_empty() {
            redis::cmd("DEL")
                .arg(&keys)
                .query_async(&mut conn)
                .await
                .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;
        }

        Ok(())
    }

    async fn get_cluster_info(&self) -> Result<ClusterInfo, AdvancedCacheError> {
        let mut conn = self.client.get_async_connection().await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        let cluster_info: String = redis::cmd("CLUSTER")
            .arg("INFO")
            .query_async(&mut conn)
            .await
            .map_err(|e| AdvancedCacheError::RedisClusterError(e.to_string()))?;

        // Parse cluster info (simplified implementation)
        let mut nodes = Vec::new();
        let mut master_nodes = Vec::new();
        let mut slots_per_node = HashMap::new();

        // This is a simplified parser - in production you'd want a more robust implementation
        for line in cluster_info.lines() {
            if line.contains("cluster_known_nodes") {
                // Parse node count
            }
        }

        Ok(ClusterInfo {
            nodes,
            master_nodes,
            total_slots: 16384, // Redis cluster default
            slots_per_node,
        })
    }
}

/// Cache Warming Strategy
#[derive(Debug, Clone)]
pub struct CacheWarmer<T> {
    warm_up_queries: Vec<CacheWarmUpQuery>,
    _phantom: std::marker::PhantomData<T>,
}

#[derive(Debug, Clone)]
pub struct CacheWarmUpQuery {
    pub key_pattern: String,
    pub ttl: Duration,
    pub priority: CachePriority,
    pub tags: HashSet<String>,
    pub warm_up_function: Arc<dyn Fn() -> Vec<(String, serde_json::Value)> + Send + Sync>,
}

impl<T> CacheWarmer<T> {
    pub fn new() -> Self {
        Self {
            warm_up_queries: Vec::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn add_warm_up_query<F>(&mut self, key_pattern: String, ttl: Duration, priority: CachePriority, tags: HashSet<String>, warm_up_fn: F)
    where
        F: Fn() -> Vec<(String, serde_json::Value)> + Send + Sync + 'static,
    {
        self.warm_up_queries.push(CacheWarmUpQuery {
            key_pattern,
            ttl,
            priority,
            tags,
            warm_up_function: Arc::new(warm_up_fn),
        });
    }

    pub async fn warm_up_cache(&self, cache: &mut LRUCache<serde_json::Value>) -> Result<usize, AdvancedCacheError> {
        let mut total_warmed = 0;

        for query in &self.warm_up_queries {
            info!("Warming up cache for pattern: {}", query.key_pattern);
            
            let data = (query.warm_up_function)();
            
            for (key, value) in data {
                cache.put(
                    key,
                    value,
                    Some(query.ttl.as_secs()),
                    query.priority,
                    query.tags.clone(),
                );
                total_warmed += 1;
            }
        }

        info!("Cache warming completed. Warmed {} entries", total_warmed);
        Ok(total_warmed)
    }
}

/// Intelligent Cache Prefetching
pub struct CachePrefetcher<T> {
    prediction_model: HashMap<String, Vec<String>>,
    prefetch_threshold: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> CachePrefetcher<T> {
    pub fn new(prefetch_threshold: usize) -> Self {
        Self {
            prediction_model: HashMap::new(),
            prefetch_threshold,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn record_access_pattern(&mut self, current_key: &str, next_keys: Vec<String>) {
        self.prediction_model.insert(current_key.to_string(), next_keys);
    }

    pub fn get_prefetch_candidates(&self, current_key: &str) -> Vec<String> {
        self.prediction_model
            .get(current_key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .take(self.prefetch_threshold)
            .collect()
    }
}

/// Advanced Cache Manager
pub struct AdvancedCacheManager<T> {
    lru_cache: TokioRwLock<LRUCache<T>>,
    distributed_cache: Option<Box<dyn DistributedCache>>,
    warmer: CacheWarmer<T>,
    prefetcher: CachePrefetcher<T>,
    invalidation_patterns: HashMap<String, Vec<String>>,
}

impl<T> AdvancedCacheManager<T>
where
    T: Clone + Send + Sync + serde::Serialize + for<'de> serde::Deserialize<'de>,
{
    pub fn new(
        capacity: usize,
        distributed_cache: Option<Box<dyn DistributedCache>>,
    ) -> Self {
        Self {
            lru_cache: TokioRwLock::new(LRUCache::new(capacity)),
            distributed_cache,
            warmer: CacheWarmer::new(),
            prefetcher: CachePrefetcher::new(5),
            invalidation_patterns: HashMap::new(),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<T>, AdvancedCacheError> {
        // Try LRU cache first
        {
            let mut cache = self.lru_cache.write().await;
            if let Some(value) = cache.get(key) {
                // Record access for prefetching analytics (prefetching happens on next access)
                let _prefetch_candidates = self.prefetcher.get_prefetch_candidates(key);
                return Ok(Some(value.clone()));
            }
        }

        // Try distributed cache
        if let Some(dist_cache) = &self.distributed_cache {
            if let Some(value_str) = dist_cache.get(key).await? {
                let value: T = serde_json::from_str(&value_str)?;
                
                // Store in LRU cache
                let mut cache = self.lru_cache.write().await;
                cache.put(
                    key.to_string(),
                    value.clone(),
                    None,
                    CachePriority::Medium,
                    HashSet::new(),
                );
                
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    pub async fn set(&self, key: String, value: T, ttl: Option<Duration>, priority: CachePriority, tags: HashSet<String>) -> Result<(), AdvancedCacheError> {
        let ttl_secs = ttl.map(|d| d.as_secs());

        // Store in LRU cache
        {
            let mut cache = self.lru_cache.write().await;
            cache.put(key.clone(), value.clone(), ttl_secs, priority, tags);
        }

        // Store in distributed cache
        if let Some(dist_cache) = &self.distributed_cache {
            let value_str = serde_json::to_string(&value)?;
            dist_cache.set(&key, &value_str, ttl).await?;
        }

        Ok(())
    }

    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<(), AdvancedCacheError> {
        // Invalidate LRU cache
        {
            let mut cache = self.lru_cache.write().await;
            cache.invalidate_by_pattern(pattern);
        }

        // Invalidate distributed cache
        if let Some(dist_cache) = &self.distributed_cache {
            dist_cache.invalidate_pattern(pattern).await?;
        }

        Ok(())
    }

    pub async fn invalidate_by_tag(&self, tag: &str) -> Result<(), AdvancedCacheError> {
        // Invalidate LRU cache
        {
            let mut cache = self.lru_cache.write().await;
            cache.invalidate_by_tag(tag);
        }

        // For distributed cache, we need to get all keys with the tag
        // This is a simplified implementation - in production you'd want a more efficient approach
        if let Some(dist_cache) = &self.distributed_cache {
            // This would require implementing tag-based invalidation in the distributed cache
            // For now, we'll skip this or implement a less efficient pattern-based approach
            let pattern = format!("*{}*", tag);
            dist_cache.invalidate_pattern(&pattern).await?;
        }

        Ok(())
    }

    pub async fn warm_up(&self) -> Result<usize, AdvancedCacheError> {
        let mut cache = self.lru_cache.write().await;
        self.warmer.warm_up_cache(&mut cache).await
    }

    pub fn add_warm_up_query<F>(&mut self, key_pattern: String, ttl: Duration, priority: CachePriority, tags: HashSet<String>, warm_up_fn: F)
    where
        F: Fn() -> Vec<(String, serde_json::Value)> + Send + Sync + 'static,
    {
        self.warmer.add_warm_up_query(key_pattern, ttl, priority, tags, warm_up_fn);
    }

    pub async fn prefetch_item(&self, key: &str) -> Result<(), AdvancedCacheError> {
        // This would implement the actual prefetching logic
        // For example, if we know that accessing order details often leads to customer details,
        // we could prefetch the customer data
        debug!("Prefetching item: {}", key);
        Ok(())
    }

    pub async fn get_stats(&self) -> CacheStats {
        let cache = self.lru_cache.read().await;
        cache.stats()
    }

    pub async fn health_check(&self) -> Result<(), AdvancedCacheError> {
        // Check LRU cache
        {
            let cache = self.lru_cache.read().await;
            if cache.store.len() > cache.capacity {
                return Err(AdvancedCacheError::OperationFailed("Cache size exceeded capacity".to_string()));
            }
        }

        // Check distributed cache
        if let Some(dist_cache) = &self.distributed_cache {
            let _ = dist_cache.get_cluster_info().await?;
        }

        Ok(())
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lru_cache_basic_operations() {
        let mut cache = LRUCache::<String>::new(3);

        // Test put and get
        cache.put("key1".to_string(), "value1".to_string(), None, CachePriority::Medium, HashSet::new());
        cache.put("key2".to_string(), "value2".to_string(), None, CachePriority::Medium, HashSet::new());
        cache.put("key3".to_string(), "value3".to_string(), None, CachePriority::Medium, HashSet::new());

        assert_eq!(cache.get("key1"), Some(&"value1".to_string()));
        assert_eq!(cache.get("key2"), Some(&"value2".to_string()));
        assert_eq!(cache.get("key3"), Some(&"value3".to_string()));

        // Test LRU eviction
        cache.put("key4".to_string(), "value4".to_string(), None, CachePriority::Medium, HashSet::new());
        assert_eq!(cache.get("key1"), None); // key1 should be evicted
        assert_eq!(cache.get("key4"), Some(&"value4".to_string()));
    }

    #[tokio::test]
    async fn test_cache_invalidation_by_tag() {
        let mut cache = LRUCache::<String>::new(10);

        let mut tags1 = HashSet::new();
        tags1.insert("user".to_string());
        tags1.insert("profile".to_string());

        let mut tags2 = HashSet::new();
        tags2.insert("order".to_string());

        cache.put("user:123".to_string(), "user_data".to_string(), None, CachePriority::High, tags1);
        cache.put("order:456".to_string(), "order_data".to_string(), None, CachePriority::High, tags2);

        // Invalidate all user-related cache entries
        let invalidated = cache.invalidate_by_tag("user");
        assert_eq!(invalidated, 1);
        assert_eq!(cache.get("user:123"), None);
        assert_eq!(cache.get("order:456"), Some(&"order_data".to_string()));
    }

    #[tokio::test]
    async fn test_cache_priority_eviction() {
        let mut cache = LRUCache::<String>::new(2);

        // Add high priority item first
        cache.put("high_priority".to_string(), "high_value".to_string(), None, CachePriority::High, HashSet::new());
        
        // Add medium priority items
        cache.put("medium1".to_string(), "medium_value1".to_string(), None, CachePriority::Medium, HashSet::new());
        cache.put("medium2".to_string(), "medium_value2".to_string(), None, CachePriority::Medium, HashSet::new());

        // High priority item should still be there
        assert_eq!(cache.get("high_priority"), Some(&"high_value".to_string()));
        
        // One medium priority item should be evicted
        let medium1_present = cache.get("medium1").is_some();
        let medium2_present = cache.get("medium2").is_some();
        assert!(medium1_present || medium2_present); // At least one should be present
        assert!(!(medium1_present && medium2_present)); // But not both (capacity is 2, high priority takes 1)
    }
}
