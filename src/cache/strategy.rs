use async_trait::async_trait;
use sha2::Sha256;
use std::collections::HashMap;
use std::time::Duration;

/// Cache strategy that determines how and when caching should be applied
#[async_trait]
pub trait CacheStrategy: Send + Sync {
    /// Generates a cache key for the given parameters
    fn generate_key(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> String;

    /// Determines the TTL (Time To Live) for the cache entry
    fn get_ttl(&self) -> Option<Duration>;

    /// Whether this strategy should be used for the given request
    fn should_cache(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> bool;
}

/// Simple time-based caching strategy
/// Caches all resources of specified types for a fixed TTL
pub struct TimeBasedStrategy {
    /// TTL for cache entries
    ttl: Duration,
    /// Resource types that should be cached
    cacheable_resources: Vec<String>,
}

impl TimeBasedStrategy {
    pub fn new(ttl: Duration, cacheable_resources: Vec<String>) -> Self {
        Self {
            ttl,
            cacheable_resources,
        }
    }
}

#[async_trait]
impl CacheStrategy for TimeBasedStrategy {
    fn generate_key(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> String {
        let mut key = format!("{}:", resource_type);

        if let Some(id) = resource_id {
            key.push_str(id);
        }

        if let Some(params) = params {
            key.push(':');
            let params_str = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            key.push_str(&params_str);
        }

        key
    }

    fn get_ttl(&self) -> Option<Duration> {
        Some(self.ttl)
    }

    fn should_cache(
        &self,
        resource_type: &str,
        _resource_id: Option<&str>,
        _params: Option<&[(&str, &str)]>,
    ) -> bool {
        self.cacheable_resources.iter().any(|r| r == resource_type)
    }
}

/// User-aware caching strategy
/// Generates different cache keys for different users to prevent data leakage
pub struct UserAwareStrategy {
    /// TTL for cache entries
    ttl: Duration,
    /// Resource types that should be cached
    cacheable_resources: Vec<String>,
}

impl UserAwareStrategy {
    pub fn new(ttl: Duration, cacheable_resources: Vec<String>) -> Self {
        Self {
            ttl,
            cacheable_resources,
        }
    }
}

#[async_trait]
impl CacheStrategy for UserAwareStrategy {
    fn generate_key(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> String {
        let mut key = String::new();

        // Extract user_id from params
        let user_id = params
            .and_then(|p| p.iter().find(|(k, _)| *k == "user_id").map(|(_, v)| *v))
            .unwrap_or("anonymous");

        key.push_str(&format!("user:{}:", user_id));
        key.push_str(resource_type);

        if let Some(id) = resource_id {
            key.push(':');
            key.push_str(id);
        }

        if let Some(params) = params {
            // Filter out user_id as it's already part of the key prefix
            let filtered_params: Vec<(&str, &str)> = params
                .iter()
                .filter(|(k, _)| *k != "user_id")
                .map(|&(k, v)| (k, v))
                .collect();

            if !filtered_params.is_empty() {
                key.push(':');
                let params_str = filtered_params
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                key.push_str(&params_str);
            }
        }

        key
    }

    fn get_ttl(&self) -> Option<Duration> {
        Some(self.ttl)
    }

    fn should_cache(
        &self,
        resource_type: &str,
        _resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> bool {
        // Only cache if resource type is cacheable and user_id is present
        self.cacheable_resources.iter().any(|r| r == resource_type)
            && params.is_some_and(|p| p.iter().any(|(k, _)| *k == "user_id"))
    }
}

/// Volatile data caching strategy
/// For frequently changing data that needs short TTLs
pub struct VolatileStrategy {
    /// TTL for cache entries
    ttl: Duration,
    /// Resource types that should be cached
    cacheable_resources: Vec<String>,
    /// Invalidation patterns - maps resource types to patterns that should invalidate this cache
    invalidation_patterns: HashMap<String, Vec<String>>,
}

impl VolatileStrategy {
    pub fn new(
        ttl: Duration,
        cacheable_resources: Vec<String>,
        invalidation_patterns: HashMap<String, Vec<String>>,
    ) -> Self {
        Self {
            ttl,
            cacheable_resources,
            invalidation_patterns,
        }
    }

    /// Checks if a given event should invalidate a cache entry
    pub fn should_invalidate(&self, resource_type: &str, event_type: &str) -> bool {
        self.invalidation_patterns
            .get(resource_type)
            .map(|patterns| patterns.iter().any(|p| p == event_type))
            .unwrap_or(false)
    }
}

#[async_trait]
impl CacheStrategy for VolatileStrategy {
    fn generate_key(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> String {
        let mut key = format!("volatile:{}:", resource_type);

        if let Some(id) = resource_id {
            key.push_str(id);
        }

        if let Some(params) = params {
            key.push(':');
            let params_str = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            key.push_str(&params_str);
        }

        key
    }

    fn get_ttl(&self) -> Option<Duration> {
        Some(self.ttl)
    }

    fn should_cache(
        &self,
        resource_type: &str,
        _resource_id: Option<&str>,
        _params: Option<&[(&str, &str)]>,
    ) -> bool {
        self.cacheable_resources.iter().any(|r| r == resource_type)
    }
}

/// No caching strategy - explicitly avoids caching for sensitive data
pub struct NoCacheStrategy {
    /// Resource types that should never be cached
    uncacheable_resources: Vec<String>,
}

impl NoCacheStrategy {
    pub fn new(uncacheable_resources: Vec<String>) -> Self {
        Self {
            uncacheable_resources,
        }
    }
}

#[async_trait]
impl CacheStrategy for NoCacheStrategy {
    fn generate_key(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> String {
        // This key will never be used, but we need to implement it
        let mut key = format!("nocache:{}:", resource_type);

        if let Some(id) = resource_id {
            key.push_str(id);
        }

        if let Some(params) = params {
            key.push(':');
            let params_str = params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&");
            key.push_str(&params_str);
        }

        key
    }

    fn get_ttl(&self) -> Option<Duration> {
        None
    }

    fn should_cache(
        &self,
        resource_type: &str,
        _resource_id: Option<&str>,
        _params: Option<&[(&str, &str)]>,
    ) -> bool {
        !self
            .uncacheable_resources
            .iter()
            .any(|r| r == resource_type)
    }
}

/// Hierarchical caching strategy
/// For resources with parent-child relationships
pub struct HierarchicalStrategy {
    /// TTL for cache entries
    ttl: Duration,
    /// Resource types that should be cached with their hierarchy info
    /// Format: (resource_type, parent_resource_type, parent_id_param)
    hierarchical_resources: Vec<(String, String, String)>,
}

impl HierarchicalStrategy {
    pub fn new(ttl: Duration, hierarchical_resources: Vec<(String, String, String)>) -> Self {
        Self {
            ttl,
            hierarchical_resources,
        }
    }

    /// Get parent info for a resource type
    fn get_parent_info(&self, resource_type: &str) -> Option<(&str, &str)> {
        self.hierarchical_resources
            .iter()
            .find(|(rt, _, _)| rt == resource_type)
            .map(|(_, parent_type, parent_param)| (parent_type.as_str(), parent_param.as_str()))
    }
}

#[async_trait]
impl CacheStrategy for HierarchicalStrategy {
    fn generate_key(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> String {
        let mut key = String::new();

        // Check if this is a hierarchical resource
        if let Some((parent_type, parent_param)) = self.get_parent_info(resource_type) {
            // Extract parent ID from params
            if let Some(parent_id) =
                params.and_then(|p| p.iter().find(|(k, _)| *k == parent_param).map(|(_, v)| *v))
            {
                key.push_str(&format!("{}:{}:", parent_type, parent_id));
            }
        }

        key.push_str(resource_type);

        if let Some(id) = resource_id {
            key.push(':');
            key.push_str(id);
        }

        if let Some(params) = params {
            // Filter out parent ID param as it's already part of the key prefix
            let filtered_params: Vec<(&str, &str)> =
                if let Some((_, parent_param)) = self.get_parent_info(resource_type) {
                    params
                        .iter()
                        .filter(|(k, _)| *k != parent_param)
                        .map(|&(k, v)| (k, v))
                        .collect()
                } else {
                    params.to_vec()
                };

            if !filtered_params.is_empty() {
                key.push(':');
                let params_str = filtered_params
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("&");
                key.push_str(&params_str);
            }
        }

        key
    }

    fn get_ttl(&self) -> Option<Duration> {
        Some(self.ttl)
    }

    fn should_cache(
        &self,
        resource_type: &str,
        _resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> bool {
        if let Some((_parent_type, parent_param)) = self.get_parent_info(resource_type) {
            // Check if parent ID is present in params
            params.is_some_and(|p| p.iter().any(|(k, _)| *k == parent_param))
        } else {
            false
        }
    }
}

/// A factory for creating caching strategies
/// Helps select the appropriate caching strategy based on the resource type
pub struct CacheStrategyFactory {
    strategies: Vec<Box<dyn CacheStrategy>>,
}

impl CacheStrategyFactory {
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    pub fn add_strategy(&mut self, strategy: Box<dyn CacheStrategy>) {
        self.strategies.push(strategy);
    }

    /// Find the first strategy that should be used for the given request
    pub fn find_strategy(
        &self,
        resource_type: &str,
        resource_id: Option<&str>,
        params: Option<&[(&str, &str)]>,
    ) -> Option<&dyn CacheStrategy> {
        self.strategies
            .iter()
            .find(|s| s.should_cache(resource_type, resource_id, params))
            .map(|s| s.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_time_based_strategy() {
        let strategy = TimeBasedStrategy::new(
            Duration::from_secs(60),
            vec!["orders".to_string(), "products".to_string()],
        );

        // Test key generation
        let key = strategy.generate_key("orders", Some("123"), Some(&[("status", "pending")]));
        assert_eq!(key, "orders:123:status=pending");

        // Test should_cache
        assert!(strategy.should_cache("orders", None, None));
        assert!(strategy.should_cache("products", None, None));
        assert!(!strategy.should_cache("customers", None, None));

        // Test TTL
        assert_eq!(strategy.get_ttl(), Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_user_aware_strategy() {
        let strategy = UserAwareStrategy::new(
            Duration::from_secs(300),
            vec!["orders".to_string(), "cart".to_string()],
        );

        // Test key generation with user_id
        let key = strategy.generate_key(
            "orders",
            Some("123"),
            Some(&[("user_id", "user456"), ("status", "completed")]),
        );
        assert_eq!(key, "user:user456:orders:123:status=completed");

        // Test should_cache
        assert!(strategy.should_cache("orders", None, Some(&[("user_id", "user456")])));
        assert!(!strategy.should_cache("orders", None, Some(&[("customer_id", "456")])));
        assert!(!strategy.should_cache("customers", None, Some(&[("user_id", "user456")])));
    }

    #[test]
    fn test_volatile_strategy() {
        let mut invalidation_patterns = HashMap::new();
        invalidation_patterns.insert(
            "inventory".to_string(),
            vec!["inventory.updated".to_string(), "order.shipped".to_string()],
        );

        let strategy = VolatileStrategy::new(
            Duration::from_secs(30),
            vec!["inventory".to_string()],
            invalidation_patterns,
        );

        // Test key generation
        let key =
            strategy.generate_key("inventory", Some("SKU123"), Some(&[("warehouse", "main")]));
        assert_eq!(key, "volatile:inventory:SKU123:warehouse=main");

        // Test should_cache
        assert!(strategy.should_cache("inventory", None, None));
        assert!(!strategy.should_cache("products", None, None));

        // Test invalidation
        assert!(strategy.should_invalidate("inventory", "inventory.updated"));
        assert!(strategy.should_invalidate("inventory", "order.shipped"));
        assert!(!strategy.should_invalidate("inventory", "product.updated"));
        assert!(!strategy.should_invalidate("products", "inventory.updated"));
    }

    #[test]
    fn test_no_cache_strategy() {
        let strategy = NoCacheStrategy::new(vec!["payments".to_string(), "users".to_string()]);

        // Test should_cache
        assert!(!strategy.should_cache("payments", None, None));
        assert!(!strategy.should_cache("users", None, None));
        assert!(strategy.should_cache("products", None, None));
    }

    #[test]
    fn test_hierarchical_strategy() {
        let hierarchical_resources = vec![
            (
                "order_items".to_string(),
                "orders".to_string(),
                "order_id".to_string(),
            ),
            (
                "line_items".to_string(),
                "invoices".to_string(),
                "invoice_id".to_string(),
            ),
        ];

        let strategy = HierarchicalStrategy::new(Duration::from_secs(120), hierarchical_resources);

        // Test key generation for order items
        let key = strategy.generate_key(
            "order_items",
            Some("item123"),
            Some(&[("order_id", "order456"), ("product_id", "prod789")]),
        );
        assert_eq!(
            key,
            "orders:order456:order_items:item123:product_id=prod789"
        );

        // Test should_cache
        assert!(strategy.should_cache("order_items", None, Some(&[("order_id", "order456")])));
        assert!(!strategy.should_cache("order_items", None, None));
        assert!(!strategy.should_cache("products", None, Some(&[("order_id", "order456")])));
    }

    #[test]
    fn test_cache_strategy_factory() {
        let mut factory = CacheStrategyFactory::new();

        // Add NoCache strategy
        factory.add_strategy(Box::new(NoCacheStrategy::new(vec![
            "payments".to_string(),
            "users".to_string(),
        ])));

        // Add TimeBasedStrategy
        factory.add_strategy(Box::new(TimeBasedStrategy::new(
            Duration::from_secs(60),
            vec!["products".to_string(), "categories".to_string()],
        )));

        // Test finding strategies
        assert!(factory.find_strategy("payments", None, None).is_none());
        assert!(factory.find_strategy("products", None, None).is_some());
    }
}
