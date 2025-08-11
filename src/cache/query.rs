use super::{InMemoryCache, CacheError};
use sea_orm::{
    ActiveModelBehavior, ActiveModelTrait, ActiveValue, ColumnTrait,
    DatabaseConnection, EntityTrait, FromQueryResult, ModelTrait, 
    PaginatorTrait, QueryTrait, Select, Statement,
};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// A wrapper around SeaORM queries that provides caching functionality
#[derive(Clone)]
pub struct CachedQuery<E>
where
    E: EntityTrait,
    E::Model: Clone + Debug + Serialize + for<'de> Deserialize<'de>,
{
    cache: Arc<InMemoryCache>,
    entity_name: String,
    default_ttl: Duration,
    _phantom: std::marker::PhantomData<E>,
}

impl<E> CachedQuery<E>
where
    E: EntityTrait,
    E::Model: Clone + Debug + Serialize + for<'de> Deserialize<'de>,
{
    /// Create a new cached query instance
    pub fn new(cache: Arc<InMemoryCache>, entity_name: String) -> Self {
        Self {
            cache,
            entity_name,
            default_ttl: Duration::from_secs(300), // 5 minutes default
            _phantom: std::marker::PhantomData,
        }
    }

    /// Set the default TTL for cached results
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    fn generate_key(&self, query: &Select<E>) -> String {
        // For now, just generate a simple key based on entity name
        // In a real implementation, you'd want to include query parameters
        format!("query:{}:simple", self.entity_name)
    }

    /// Execute a query with caching for multiple results
    pub async fn find_many(
        &self,
        db: &DatabaseConnection,
        query: Select<E>,
        ttl: Option<Duration>,
    ) -> Result<Vec<E::Model>, CacheError> {
        let cache_key = self.generate_key(&query);
        let ttl = ttl.unwrap_or(self.default_ttl);

        // Try to get from cache first
        match self.cache.get(&cache_key).await {
            Ok(Some(cached_data)) => {
                match serde_json::from_str::<Vec<E::Model>>(&cached_data) {
                    Ok(models) => {
                        debug!("Cache hit for query: {}", cache_key);
                        return Ok(models);
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached query result: {}", e);
                        // Continue to database query
                    }
                }
            }
            Ok(None) => {
                debug!("Cache miss for query: {}", cache_key);
            }
            Err(e) => {
                warn!("Cache error: {}", e);
                // Continue to database query
            }
        }

        // Execute the query against the database
        let result = query.all(db).await.map_err(|e| {
            CacheError::OperationFailed(format!("Database query failed: {}", e))
        })?;

        // Cache the result
        let cache = self.cache.clone();
        let cache_key_clone = cache_key.clone();
        let result_clone = result.clone();
        tokio::spawn(async move {
            match serde_json::to_string(&result_clone) {
                Ok(serialized) => {
                    if let Err(err) = cache.set(&cache_key_clone, &serialized, Some(ttl)).await {
                        warn!("Failed to cache query result: {}", err);
                    }
                }
                Err(e) => {
                    warn!("Failed to serialize query result for caching: {}", e);
                }
            }
        });

        Ok(result)
    }

    /// Execute a query with caching for a single result
    pub async fn find_one(
        &self,
        db: &DatabaseConnection,
        query: Select<E>,
        ttl: Option<Duration>,
    ) -> Result<Option<E::Model>, CacheError> {
        let cache_key = self.generate_key(&query);
        let ttl = ttl.unwrap_or(self.default_ttl);

        // Try to get from cache first
        match self.cache.get(&cache_key).await {
            Ok(Some(cached_data)) => {
                match serde_json::from_str::<Option<E::Model>>(&cached_data) {
                    Ok(model) => {
                        debug!("Cache hit for single query: {}", cache_key);
                        return Ok(model);
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached single query result: {}", e);
                        // Continue to database query
                    }
                }
            }
            Ok(None) => {
                debug!("Cache miss for single query: {}", cache_key);
            }
            Err(e) => {
                warn!("Cache error: {}", e);
                // Continue to database query
            }
        }

        // Execute the query against the database
        let result = query.one(db).await.map_err(|e| {
            CacheError::OperationFailed(format!("Database query failed: {}", e))
        })?;

        // Cache the result
        let cache = self.cache.clone();
        let cache_key_clone = cache_key.clone();
        let result_clone = result.clone();
        tokio::spawn(async move {
            match serde_json::to_string(&result_clone) {
                Ok(serialized) => {
                    if let Err(err) = cache.set(&cache_key_clone, &serialized, Some(ttl)).await {
                        warn!("Failed to cache single query result: {}", err);
                    }
                }
                Err(e) => {
                    warn!("Failed to serialize single query result for caching: {}", e);
                }
            }
        });

        Ok(result)
    }

    /// Count query with caching
    pub async fn count(
        &self,
        db: &DatabaseConnection,
        query: Select<E>,
        ttl: Option<Duration>,
    ) -> Result<u64, CacheError> {
        let cache_key = format!("{}:count", self.generate_key(&query));
        let ttl = ttl.unwrap_or(self.default_ttl);

        // Try to get from cache first
        match self.cache.get(&cache_key).await {
            Ok(Some(cached_data)) => {
                match cached_data.parse::<u64>() {
                    Ok(count) => {
                        debug!("Cache hit for count query: {}", cache_key);
                        return Ok(count);
                    }
                    Err(e) => {
                        warn!("Failed to parse cached count result: {}", e);
                        // Continue to database query
                    }
                }
            }
            Ok(None) => {
                debug!("Cache miss for count query: {}", cache_key);
            }
            Err(e) => {
                warn!("Cache error: {}", e);
                // Continue to database query
            }
        }

        // Execute the count query against the database
        // For now, just count the actual results since sea_orm count API is complex
        let results = query.all(db).await.map_err(|e| {
            CacheError::OperationFailed(format!("Database query failed: {}", e))
        })?;
        let result = results.len() as u64;

        // Cache the result
        let cache = self.cache.clone();
        let cache_key_clone = cache_key.clone();
        tokio::spawn(async move {
            if let Err(err) = cache.set(&cache_key_clone, &result.to_string(), Some(ttl)).await {
                warn!("Failed to cache count result: {}", err);
            }
        });

        Ok(result)
    }

    /// Invalidate cache for this entity
    pub async fn invalidate(&self) -> Result<(), CacheError> {
        // For now, we'll need to implement a more sophisticated invalidation strategy
        // This is a simplified version that just logs the request
        warn!("Cache invalidation requested for entity: {}", self.entity_name);
        Ok(())
    }

    /// Invalidate cache by pattern
    pub async fn invalidate_pattern(&self, pattern: &str) -> Result<(), CacheError> {
        // In a real implementation, you'd want to iterate through cache keys
        // and remove ones matching the pattern
        warn!("Pattern-based cache invalidation not implemented for pattern: {}", pattern);
        Ok(())
    }
}

/// Factory for creating cached queries
pub struct CachedQueryFactory {
    cache: Arc<InMemoryCache>,
}

impl CachedQueryFactory {
    pub fn new(cache: Arc<InMemoryCache>) -> Self {
        Self { cache }
    }

    pub fn create_query<E>(&self, entity_name: &str) -> CachedQuery<E>
    where
        E: EntityTrait,
        E::Model: Clone + Debug + Serialize + for<'de> Deserialize<'de>,
    {
        CachedQuery::new(self.cache.clone(), entity_name.to_string())
    }
}

/// Trait for entities that can be cached
pub trait CacheableEntity: EntityTrait
where
    Self::Model: Clone + Debug + Serialize + for<'de> Deserialize<'de>,
{
    fn cache_key_prefix() -> &'static str;
    fn cache_ttl() -> Duration {
        Duration::from_secs(300) // 5 minutes default
    }
}

/// Extension trait for adding caching to SeaORM Select queries
pub trait CacheableSelect<E>
where
    E: EntityTrait,
    E::Model: Clone + Debug + Serialize + for<'de> Deserialize<'de>,
{
    fn cached(self, cache: Arc<InMemoryCache>) -> CachedQuery<E>;
}

impl<E> CacheableSelect<E> for Select<E>
where
    E: CacheableEntity,
    E::Model: Clone + Debug + Serialize + for<'de> Deserialize<'de>,
{
    fn cached(self, cache: Arc<InMemoryCache>) -> CachedQuery<E> {
        CachedQuery::new(cache, E::cache_key_prefix().to_string())
    }
}

// Generic cache invalidation helpers
pub async fn invalidate_entity_cache(
    cache: &InMemoryCache,
    entity_name: &str,
) -> Result<(), CacheError> {
    let _prefix = format!("entity:{}:", entity_name);
    let _query_prefix = format!("query:{}:", entity_name);
    
    // In a real implementation, you'd iterate through cache keys and remove matching ones
    warn!("Entity cache invalidation not fully implemented for: {}", entity_name);
    Ok(())
}

pub async fn invalidate_query_cache(
    cache: &InMemoryCache,
    query_pattern: &str,
) -> Result<(), CacheError> {
    warn!("Query cache invalidation not fully implemented for pattern: {}", query_pattern);
    Ok(())
}
