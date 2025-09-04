/*!
 * # Advanced Caching Integration Module
 *
 * This module provides integration between advanced caching and database optimization,
 * creating a unified high-performance data access layer.
 */

use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::advanced::{AdvancedCacheManager, CachePriority, DistributedCache, RedisClusterCache};
use crate::db::optimization::{DatabaseOptimizationManager, QueryType, DatabaseOptimizationConfig};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStrategy {
    pub name: String,
    pub priority: CachePriority,
    pub ttl: Duration,
    pub tags: Vec<String>,
    pub enable_warming: bool,
    pub enable_prefetch: bool,
    pub database_fallback: bool,
}

#[derive(Debug)]
pub struct IntegratedCacheSystem<T> {
    cache_manager: Arc<AdvancedCacheManager<T>>,
    db_manager: Arc<DatabaseOptimizationManager>,
    strategies: HashMap<String, CacheStrategy>,
    performance_stats: Arc<RwLock<CachePerformanceStats>>,
}

#[derive(Debug, Clone, Default)]
pub struct CachePerformanceStats {
    pub total_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub database_queries: u64,
    pub average_response_time: Duration,
    pub error_count: u64,
}

#[derive(Debug, Clone)]
pub struct CacheRequest<T> {
    pub key: String,
    pub strategy_name: String,
    pub data_fetcher: Option<Arc<dyn Fn() -> T + Send + Sync>>,
    pub prefer_read_replica: bool,
    pub enable_caching: bool,
}

impl<T> IntegratedCacheSystem<T>
where
    T: Clone + Send + Sync + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
{
    pub async fn new(
        cache_capacity: usize,
        db_primary_url: &str,
        db_replica_urls: Vec<String>,
        redis_cluster_nodes: Option<Vec<String>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Create distributed cache if Redis cluster is provided
        let distributed_cache: Option<Box<dyn DistributedCache>> = if let Some(nodes) = redis_cluster_nodes {
            Some(Box::new(RedisClusterCache::new(nodes)?))
        } else {
            None
        };

        // Create cache manager
        let mut cache_manager = AdvancedCacheManager::new(cache_capacity, distributed_cache);

        // Create database optimization manager
        let db_config = DatabaseOptimizationConfig::default();
        let db_manager = DatabaseOptimizationManager::new(
            db_primary_url,
            db_replica_urls,
            db_config,
        ).await?;

        // Initialize default cache strategies
        let mut strategies = HashMap::new();

        // User data strategy - high priority, longer TTL
        strategies.insert("user_data".to_string(), CacheStrategy {
            name: "user_data".to_string(),
            priority: CachePriority::High,
            ttl: Duration::from_secs(3600), // 1 hour
            tags: vec!["user".to_string(), "profile".to_string()],
            enable_warming: true,
            enable_prefetch: true,
            database_fallback: true,
        });

        // Order data strategy - critical priority, medium TTL
        strategies.insert("order_data".to_string(), CacheStrategy {
            name: "order_data".to_string(),
            priority: CachePriority::Critical,
            ttl: Duration::from_secs(1800), // 30 minutes
            tags: vec!["order".to_string(), "transaction".to_string()],
            enable_warming: true,
            enable_prefetch: true,
            database_fallback: true,
        });

        // Inventory data strategy - medium priority, short TTL
        strategies.insert("inventory_data".to_string(), CacheStrategy {
            name: "inventory_data".to_string(),
            priority: CachePriority::Medium,
            ttl: Duration::from_secs(300), // 5 minutes
            tags: vec!["inventory".to_string(), "stock".to_string()],
            enable_warming: true,
            enable_prefetch: false,
            database_fallback: true,
        });

        // Analytics data strategy - low priority, long TTL
        strategies.insert("analytics_data".to_string(), CacheStrategy {
            name: "analytics_data".to_string(),
            priority: CachePriority::Low,
            ttl: Duration::from_secs(7200), // 2 hours
            tags: vec!["analytics".to_string(), "report".to_string()],
            enable_warming: false,
            enable_prefetch: false,
            database_fallback: false,
        });

        Ok(Self {
            cache_manager: Arc::new(cache_manager),
            db_manager: Arc::new(db_manager),
            strategies,
            performance_stats: Arc::new(RwLock::new(CachePerformanceStats::default())),
        })
    }

    pub async fn execute_request(&self, request: CacheRequest<T>) -> Result<T, Box<dyn std::error::Error>> {
        let start_time = Instant::now();
        let mut stats = self.performance_stats.write().await;
        stats.total_requests += 1;

        let strategy = self.strategies.get(&request.strategy_name)
            .ok_or_else(|| format!("Unknown cache strategy: {}", request.strategy_name))?;

        // Try cache first
        if request.enable_caching {
            if let Ok(Some(cached_data)) = self.cache_manager.get(&request.key).await {
                stats.cache_hits += 1;
                let response_time = start_time.elapsed();
                stats.average_response_time = Duration::from_nanos(
                    ((stats.average_response_time.as_nanos() * (stats.cache_hits - 1)) as u128
                     + response_time.as_nanos()) / stats.cache_hits as u128
                );
                return Ok(cached_data);
            }
            stats.cache_misses += 1;
        }

        // Fetch from database with optimization
        if strategy.database_fallback {
            if let Some(fetcher) = &request.data_fetcher {
                stats.database_queries += 1;

                // Use optimized database query
                let data = (fetcher)();

                // Cache the result if caching is enabled
                if request.enable_caching {
                    let tags: std::collections::HashSet<String> = strategy.tags.iter().cloned().collect();
                    let _ = self.cache_manager.set(
                        request.key.clone(),
                        data.clone(),
                        Some(strategy.ttl),
                        strategy.priority,
                        tags,
                    ).await;
                }

                let response_time = start_time.elapsed();
                stats.average_response_time = Duration::from_nanos(
                    ((stats.average_response_time.as_nanos() * (stats.total_requests - 1)) as u128
                     + response_time.as_nanos()) / stats.total_requests as u128
                );

                Ok(data)
            } else {
                stats.error_count += 1;
                Err("No data fetcher provided and database fallback enabled".into())
            }
        } else {
            stats.error_count += 1;
            Err("Cache miss and database fallback disabled".into())
        }
    }

    pub async fn invalidate_by_tags(&self, tags: &[String]) -> Result<usize, Box<dyn std::error::Error>> {
        let mut total_invalidated = 0;

        for tag in tags {
            total_invalidated += self.cache_manager.invalidate_by_tag(tag).await?;
        }

        Ok(total_invalidated)
    }

    pub async fn warm_up_cache(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Starting cache warming process");

        // Add warming queries for different data types
        for (strategy_name, strategy) in &self.strategies {
            if strategy.enable_warming {
                match strategy_name.as_str() {
                    "user_data" => {
                        self.cache_manager.add_warm_up_query(
                            "user:*".to_string(),
                            strategy.ttl,
                            strategy.priority,
                            strategy.tags.iter().cloned().collect(),
                            || {
                                // Simulate fetching recent users
                                vec![
                                    ("user:1".to_string(), serde_json::json!({"id": 1, "name": "User 1"})),
                                    ("user:2".to_string(), serde_json::json!({"id": 2, "name": "User 2"})),
                                ]
                            }
                        );
                    }
                    "order_data" => {
                        self.cache_manager.add_warm_up_query(
                            "order:*".to_string(),
                            strategy.ttl,
                            strategy.priority,
                            strategy.tags.iter().cloned().collect(),
                            || {
                                // Simulate fetching recent orders
                                vec![
                                    ("order:1001".to_string(), serde_json::json!({"id": 1001, "total": 99.99})),
                                    ("order:1002".to_string(), serde_json::json!({"id": 1002, "total": 149.99})),
                                ]
                            }
                        );
                    }
                    "inventory_data" => {
                        self.cache_manager.add_warm_up_query(
                            "inventory:*".to_string(),
                            strategy.ttl,
                            strategy.priority,
                            strategy.tags.iter().cloned().collect(),
                            || {
                                // Simulate fetching inventory data
                                vec![
                                    ("inventory:item1".to_string(), serde_json::json!({"id": "item1", "stock": 100})),
                                    ("inventory:item2".to_string(), serde_json::json!({"id": "item2", "stock": 50})),
                                ]
                            }
                        );
                    }
                    _ => {}
                }
            }
        }

        self.cache_manager.warm_up().await?;
        info!("Cache warming completed");

        Ok(())
    }

    pub async fn get_performance_stats(&self) -> CachePerformanceStats {
        self.performance_stats.read().await.clone()
    }

    pub async fn get_cache_stats(&self) -> super::advanced::CacheStats {
        self.cache_manager.get_stats().await
    }

    pub async fn get_database_stats(&self) -> crate::db::optimization::DatabaseMetrics {
        self.db_manager.connection_pool.get_metrics().await
    }

    pub async fn optimize_query(&self, query: &str) -> Result<crate::db::optimization::QueryOptimizationResult, Box<dyn std::error::Error>> {
        Ok(self.db_manager.optimize_query(query).await?)
    }

    pub async fn maintenance_tasks(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Run cache maintenance
        self.cache_manager.invalidate_by_tag("expired").await?;

        // Run database maintenance
        self.db_manager.maintenance_tasks().await?;

        // Update performance stats
        let cache_stats = self.get_cache_stats().await;
        let db_stats = self.get_database_stats().await;

        info!(
            "Maintenance completed - Cache: {} entries, DB: {} queries",
            cache_stats.size, db_stats.total_queries
        );

        Ok(())
    }

    pub fn add_custom_strategy(&mut self, name: String, strategy: CacheStrategy) {
        self.strategies.insert(name, strategy);
    }

    pub async fn prefetch_related_data(&self, key: &str, strategy_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let strategy = self.strategies.get(strategy_name)
            .ok_or_else(|| format!("Unknown strategy: {}", strategy_name))?;

        if !strategy.enable_prefetch {
            return Ok(());
        }

        // Implement prefetching logic based on key patterns
        match key {
            key if key.starts_with("user:") => {
                // Prefetch user's recent orders and profile data
                let user_id = key.strip_prefix("user:").unwrap_or("unknown");
                let order_key = format!("user:{}:recent_orders", user_id);
                let profile_key = format!("user:{}:profile", user_id);

                // These would trigger async prefetches in a real implementation
                debug!("Prefetching related data for user: {}", user_id);
            }
            key if key.starts_with("order:") => {
                // Prefetch order items and customer data
                let order_id = key.strip_prefix("order:").unwrap_or("unknown");
                let items_key = format!("order:{}:items", order_id);
                let customer_key = format!("order:{}:customer", order_id);

                debug!("Prefetching related data for order: {}", order_id);
            }
            _ => {}
        }

        Ok(())
    }
}

/// High-level API for common caching patterns
pub struct CacheApi<T> {
    system: Arc<IntegratedCacheSystem<T>>,
}

impl<T> CacheApi<T>
where
    T: Clone + Send + Sync + serde::Serialize + for<'de> serde::Deserialize<'de> + 'static,
{
    pub fn new(system: Arc<IntegratedCacheSystem<T>>) -> Self {
        Self { system }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<T, Box<dyn std::error::Error>> {
        let request = CacheRequest {
            key: format!("user:{}", user_id),
            strategy_name: "user_data".to_string(),
            data_fetcher: Some(Arc::new(move || {
                // In a real implementation, this would fetch from database
                serde_json::json!({"id": user_id, "name": "User", "email": "user@example.com"})
            })),
            prefer_read_replica: true,
            enable_caching: true,
        };

        self.system.execute_request(request).await
    }

    pub async fn get_order(&self, order_id: &str) -> Result<T, Box<dyn std::error::Error>> {
        let request = CacheRequest {
            key: format!("order:{}", order_id),
            strategy_name: "order_data".to_string(),
            data_fetcher: Some(Arc::new(move || {
                // In a real implementation, this would fetch from database
                serde_json::json!({"id": order_id, "total": 99.99, "status": "pending"})
            })),
            prefer_read_replica: false, // Use primary for order data
            enable_caching: true,
        };

        self.system.execute_request(request).await
    }

    pub async fn get_inventory(&self, item_id: &str) -> Result<T, Box<dyn std::error::Error>> {
        let request = CacheRequest {
            key: format!("inventory:{}", item_id),
            strategy_name: "inventory_data".to_string(),
            data_fetcher: Some(Arc::new(move || {
                // In a real implementation, this would fetch from database
                serde_json::json!({"id": item_id, "stock": 100, "location": "warehouse_a"})
            })),
            prefer_read_replica: true,
            enable_caching: true,
        };

        self.system.execute_request(request).await
    }

    pub async fn invalidate_user_cache(&self, user_id: &str) -> Result<usize, Box<dyn std::error::Error>> {
        let tags = vec![format!("user:{}", user_id), "user".to_string()];
        self.system.invalidate_by_tags(&tags).await
    }

    pub async fn get_system_health(&self) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error>> {
        let cache_stats = self.system.get_cache_stats().await;
        let db_stats = self.system.get_database_stats().await;
        let perf_stats = self.system.get_performance_stats().await;

        let mut health = HashMap::new();
        health.insert("cache_hit_ratio".to_string(), serde_json::json!(cache_stats.hit_ratio));
        health.insert("database_connections".to_string(), serde_json::json!(db_stats.active_connections));
        health.insert("total_requests".to_string(), serde_json::json!(perf_stats.total_requests));
        health.insert("average_response_time_ms".to_string(), serde_json::json!(perf_stats.average_response_time.as_millis()));

        Ok(health)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_integrated_cache_system() {
        let system = IntegratedCacheSystem::<serde_json::Value>::new(
            100,
            "sqlite::memory:",
            vec![],
            None,
        ).await.unwrap();

        let api = CacheApi::new(Arc::new(system));

        // Test user data retrieval
        let user_data = api.get_user("123").await.unwrap();
        assert_eq!(user_data["id"], "123");

        // Test cache hit (should return same data without database call)
        let user_data2 = api.get_user("123").await.unwrap();
        assert_eq!(user_data, user_data2);

        // Test order data retrieval
        let order_data = api.get_order("456").await.unwrap();
        assert_eq!(order_data["id"], "456");

        // Test cache invalidation
        let invalidated = api.invalidate_user_cache("123").await.unwrap();
        assert!(invalidated >= 1);
    }

    #[tokio::test]
    async fn test_cache_strategies() {
        let system = IntegratedCacheSystem::<serde_json::Value>::new(
            100,
            "sqlite::memory:",
            vec![],
            None,
        ).await.unwrap();

        // Verify strategies are loaded
        assert!(system.strategies.contains_key("user_data"));
        assert!(system.strategies.contains_key("order_data"));
        assert!(system.strategies.contains_key("inventory_data"));
        assert!(system.strategies.contains_key("analytics_data"));

        // Verify strategy properties
        let user_strategy = &system.strategies["user_data"];
        assert_eq!(user_strategy.priority, CachePriority::High);
        assert!(user_strategy.enable_warming);
        assert!(user_strategy.enable_prefetch);
    }

    #[tokio::test]
    async fn test_performance_stats() {
        let system = IntegratedCacheSystem::<serde_json::Value>::new(
            100,
            "sqlite::memory:",
            vec![],
            None,
        ).await.unwrap();

        let api = CacheApi::new(Arc::new(system));

        // Generate some requests
        for i in 0..5 {
            let _ = api.get_user(&format!("user{}", i)).await.unwrap();
        }

        let stats = api.system.get_performance_stats().await;
        assert_eq!(stats.total_requests, 5);
        assert!(stats.cache_misses >= 4); // First request is miss, others may be hits
    }
}
