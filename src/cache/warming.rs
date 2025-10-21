/*!
 * # Cache Warming Module
 *
 * This module provides intelligent cache warming capabilities:
 * - Analysis of access patterns to identify frequently used data
 * - Proactive cache warming based on usage patterns
 * - Scheduled cache warming jobs
 * - Cache warming for predicted access patterns
 */

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use chrono::{DateTime, Utc, Timelike};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPattern {
    pub key: String,
    pub access_count: u64,
    pub last_accessed: DateTime<Utc>,
    pub average_access_interval: Option<Duration>,
    pub access_times: Vec<DateTime<Utc>>,
    pub related_keys: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct CacheWarmingConfig {
    pub enable_pattern_analysis: bool,
    pub enable_predictive_warming: bool,
    pub enable_scheduled_warming: bool,
    pub analysis_window_hours: u64,
    pub min_access_count_threshold: u64,
    pub max_patterns_to_track: usize,
    pub warming_batch_size: usize,
    pub scheduled_warming_times: Vec<(u32, u32)>, // (hour, minute) pairs
}

impl Default for CacheWarmingConfig {
    fn default() -> Self {
        Self {
            enable_pattern_analysis: true,
            enable_predictive_warming: true,
            enable_scheduled_warming: true,
            analysis_window_hours: 24,
            min_access_count_threshold: 10,
            max_patterns_to_track: 1000,
            warming_batch_size: 100,
            scheduled_warming_times: vec![(2, 0), (14, 0)], // 2 AM and 2 PM
        }
    }
}

pub struct CacheWarmingEngine<T> {
    config: CacheWarmingConfig,
    access_patterns: Arc<RwLock<HashMap<String, AccessPattern>>>,
    warming_candidates: Arc<RwLock<VecDeque<String>>>,
    data_providers: HashMap<String, Arc<dyn CacheDataProvider<T>>>,
    warming_jobs: Vec<WarmingJob<T>>,
}

#[async_trait::async_trait]
pub trait CacheDataProvider<T>: Send + Sync {
    async fn get_data(&self, key: &str) -> Result<Option<T>, Box<dyn std::error::Error>>;
    async fn get_related_keys(&self, key: &str) -> Result<HashSet<String>, Box<dyn std::error::Error>>;
    async fn get_frequent_keys(&self, limit: usize) -> Result<Vec<String>, Box<dyn std::error::Error>>;
    fn get_provider_name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct WarmingJob<T> {
    pub name: String,
    pub schedule: WarmingSchedule,
    pub data_provider: Arc<dyn CacheDataProvider<T>>,
    pub key_pattern: String,
    pub priority: super::advanced::CachePriority,
    pub ttl: Duration,
    pub tags: HashSet<String>,
    pub last_run: Option<DateTime<Utc>>,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub enum WarmingSchedule {
    Interval(Duration),
    Daily(Vec<(u32, u32)>), // (hour, minute) pairs
    Weekly(Vec<(chrono::Weekday, u32, u32)>), // (weekday, hour, minute)
    Manual,
}

impl<T> CacheWarmingEngine<T> {
    pub fn new(config: CacheWarmingConfig) -> Self {
        Self {
            config,
            access_patterns: Arc::new(RwLock::new(HashMap::new())),
            warming_candidates: Arc::new(RwLock::new(VecDeque::new())),
            data_providers: HashMap::new(),
            warming_jobs: Vec::new(),
        }
    }

    pub fn register_data_provider(&mut self, provider: Arc<dyn CacheDataProvider<T>>) {
        let name = provider.get_provider_name().to_string();
        self.data_providers.insert(name, provider);
    }

    pub fn add_warming_job(&mut self, job: WarmingJob<T>) {
        self.warming_jobs.push(job);
    }

    pub async fn record_access(&self, key: &str) {
        if !self.config.enable_pattern_analysis {
            return;
        }

        let now = Utc::now();
        let key = key.to_string();

        let mut patterns = self.access_patterns.write().await;
        
        let pattern = patterns.entry(key.clone()).or_insert_with(|| AccessPattern {
            key: key.clone(),
            access_count: 0,
            last_accessed: now,
            average_access_interval: None,
            access_times: Vec::new(),
            related_keys: HashSet::new(),
        });

        pattern.access_count += 1;
        pattern.last_accessed = now;
        pattern.access_times.push(now);

        // Keep only recent access times
        let cutoff = now - chrono::Duration::hours(self.config.analysis_window_hours as i64);
        pattern.access_times.retain(|&time| time > cutoff);

        // Calculate average access interval
        if pattern.access_times.len() >= 2 {
            let intervals: Vec<Duration> = pattern.access_times.windows(2)
                .map(|window| {
                    let duration = window[1].signed_duration_since(window[0]);
                    Duration::from_secs(duration.num_seconds().max(1) as u64)
                })
                .collect();

            let avg_interval = intervals.iter().sum::<Duration>() / intervals.len() as u32;
            pattern.average_access_interval = Some(avg_interval);
        }

        // Limit the number of patterns we track
        if patterns.len() > self.config.max_patterns_to_track {
            // Remove least recently accessed patterns
            let mut patterns_vec: Vec<_> = patterns.iter().collect();
            patterns_vec.sort_by(|a, b| a.1.last_accessed.cmp(&b.1.last_accessed));
            
            for (key, _) in patterns_vec.into_iter().take(100) {
                patterns.remove(key);
            }
        }

        // If this key meets the threshold, add it to warming candidates
        if pattern.access_count >= self.config.min_access_count_threshold {
            let mut candidates = self.warming_candidates.write().await;
            if !candidates.contains(&key) {
                candidates.push_back(key);
            }
        }
    }

    pub async fn get_frequent_patterns(&self, limit: usize) -> Vec<AccessPattern> {
        let patterns = self.access_patterns.read().await;
        let mut patterns_vec: Vec<_> = patterns.values().cloned().collect();
        
        patterns_vec.sort_by(|a, b| b.access_count.cmp(&a.access_count));
        patterns_vec.into_iter().take(limit).collect()
    }

    pub async fn predict_access_patterns(&self) -> Vec<String> {
        if !self.config.enable_predictive_warming {
            return Vec::new();
        }

        let patterns = self.access_patterns.read().await;
        let now = Utc::now();
        let mut predictions = Vec::new();

        for pattern in patterns.values() {
            if let Some(avg_interval) = pattern.average_access_interval {
                let time_since_last_access = now.signed_duration_since(pattern.last_accessed);
                let expected_access_time = pattern.last_accessed + chrono::Duration::from_std(avg_interval).unwrap();
                
                // If we're within 80% of the expected access time, predict this will be accessed soon
                if now > expected_access_time - chrono::Duration::from_std(avg_interval * 8 / 10).unwrap() {
                    predictions.push(pattern.key.clone());
                }
            }
        }

        predictions
    }

    pub async fn warm_up_cache(&self, cache: &mut super::advanced::LRUCache<T>) -> Result<WarmingResult, Box<dyn std::error::Error>> {
        let mut result = WarmingResult::default();
        let candidates = self.warming_candidates.read().await;
        
        if candidates.is_empty() {
            debug!("No cache warming candidates available");
            return Ok(result);
        }

        let keys_to_warm: Vec<String> = candidates.iter()
            .take(self.config.warming_batch_size)
            .cloned()
            .collect();

        for key in keys_to_warm {
            if let Some(provider) = self.find_data_provider(&key) {
                match provider.get_data(&key).await {
                    Ok(Some(data)) => {
                        let mut tags = HashSet::new();
                        tags.insert("warmed".to_string());
                        tags.insert(provider.get_provider_name().to_string());

                        cache.put(
                            key.clone(),
                            data,
                            Some(Duration::from_secs(3600)), // 1 hour TTL
                            super::advanced::CachePriority::Medium,
                            tags,
                        );

                        result.entries_warmed += 1;

                        // Also warm up related keys
                        if let Ok(related_keys) = provider.get_related_keys(&key).await {
                            for related_key in related_keys.into_iter().take(5) {
                                if let Ok(Some(related_data)) = provider.get_data(&related_key).await {
                                    let mut related_tags = HashSet::new();
                                    related_tags.insert("warmed".to_string());
                                    related_tags.insert("related".to_string());
                                    related_tags.insert(provider.get_provider_name().to_string());

                                    cache.put(
                                        related_key,
                                        related_data,
                                        Some(Duration::from_secs(1800)), // 30 min TTL
                                        super::advanced::CachePriority::Low,
                                        related_tags,
                                    );

                                    result.related_entries_warmed += 1;
                                }
                            }
                        }
                    }
                    Ok(None) => {
                        result.misses += 1;
                    }
                    Err(e) => {
                        warn!("Error warming cache for key {}: {}", key, e);
                        result.errors += 1;
                    }
                }
            }
        }

        info!("Cache warming completed: {}", result);
        Ok(result)
    }

    pub async fn run_scheduled_warming(&self, cache: &mut super::advanced::LRUCache<T>) -> Result<(), Box<dyn std::error::Error>> {
        if !self.config.enable_scheduled_warming {
            return Ok(());
        }

        let now = Utc::now();
        let current_time = (now.hour(), now.minute());

        for job in &self.warming_jobs {
            if !job.enabled {
                continue;
            }

            let should_run = match &job.schedule {
                WarmingSchedule::Daily(times) => {
                    times.contains(&current_time) &&
                    job.last_run.map_or(true, |last| {
                        let hours_since_last = now.signed_duration_since(last).num_hours();
                        hours_since_last >= 22 // Allow some flexibility for exact timing
                    })
                }
                WarmingSchedule::Interval(interval) => {
                    job.last_run.map_or(true, |last| {
                        let elapsed = now.signed_duration_since(last);
                        elapsed.to_std().unwrap_or(Duration::from_secs(0)) >= *interval
                    })
                }
                WarmingSchedule::Weekly(schedule) => {
                    let current_weekday = now.weekday();
                    schedule.iter().any(|(weekday, hour, minute)| {
                        *weekday == current_weekday && (*hour, *minute) == current_time
                    }) && job.last_run.map_or(true, |last| {
                        let days_since_last = now.signed_duration_since(last).num_days();
                        days_since_last >= 6
                    })
                }
                WarmingSchedule::Manual => false,
            };

            if should_run {
                info!("Running scheduled warming job: {}", job.name);
                
                if let Ok(result) = self.run_warming_job(cache, job).await {
                    info!("Scheduled warming job {} completed: {}", job.name, result);
                } else {
                    warn!("Scheduled warming job {} failed", job.name);
                }
            }
        }

        Ok(())
    }

    async fn run_warming_job(&self, cache: &mut super::advanced::LRUCache<T>, job: &WarmingJob<T>) -> Result<WarmingResult, Box<dyn std::error::Error>> {
        let mut result = WarmingResult::default();
        let provider = &job.data_provider;

        // Get frequent keys from the provider
        let frequent_keys = provider.get_frequent_keys(100).await?;

        for key in frequent_keys {
            if key.contains(&job.key_pattern) {
                match provider.get_data(&key).await {
                    Ok(Some(data)) => {
                        cache.put(
                            key.clone(),
                            data,
                            Some(job.ttl.as_secs()),
                            job.priority,
                            job.tags.clone(),
                        );
                        result.entries_warmed += 1;
                    }
                    Ok(None) => result.misses += 1,
                    Err(e) => {
                        warn!("Error in warming job {} for key {}: {}", job.name, key, e);
                        result.errors += 1;
                    }
                }
            }
        }

        Ok(result)
    }

    fn find_data_provider(&self, key: &str) -> Option<&Arc<dyn CacheDataProvider<T>>> {
        // Simple pattern matching - in production you'd want more sophisticated routing
        for (name, provider) in &self.data_providers {
            if key.starts_with(&format!("{}:", name)) || key.contains(name) {
                return Some(provider);
            }
        }
        self.data_providers.values().next() // Fallback to first provider
    }

    pub async fn get_warming_stats(&self) -> WarmingStats {
        let patterns = self.access_patterns.read().await;
        let candidates = self.warming_candidates.read().await;

        WarmingStats {
            total_patterns_tracked: patterns.len(),
            total_warming_candidates: candidates.len(),
            average_access_count: if patterns.is_empty() {
                0.0
            } else {
                patterns.values().map(|p| p.access_count).sum::<u64>() as f64 / patterns.len() as f64
            },
            active_jobs: self.warming_jobs.iter().filter(|j| j.enabled).count(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WarmingResult {
    pub entries_warmed: usize,
    pub related_entries_warmed: usize,
    pub misses: usize,
    pub errors: usize,
}

impl std::fmt::Display for WarmingResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "warmed {} entries ({} related), {} misses, {} errors",
            self.entries_warmed, self.related_entries_warmed, self.misses, self.errors
        )
    }
}

#[derive(Debug, Clone)]
pub struct WarmingStats {
    pub total_patterns_tracked: usize,
    pub total_warming_candidates: usize,
    pub average_access_count: f64,
    pub active_jobs: usize,
}

/// Example data providers for common cache scenarios

pub struct OrderDataProvider {
    // In a real implementation, this would hold database connections, etc.
}

#[async_trait::async_trait]
impl CacheDataProvider<serde_json::Value> for OrderDataProvider {
    async fn get_data(&self, key: &str) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        // Simulate fetching order data
        if key.starts_with("order:") {
            let order_id = key.strip_prefix("order:").unwrap_or("unknown");
            let order_data = serde_json::json!({
                "id": order_id,
                "status": "pending",
                "total": 99.99,
                "items": ["item1", "item2"]
            });
            Ok(Some(order_data))
        } else {
            Ok(None)
        }
    }

    async fn get_related_keys(&self, key: &str) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
        let mut related = HashSet::new();
        if key.starts_with("order:") {
            related.insert(format!("customer:123")); // Related customer
            related.insert(format!("inventory:item1")); // Related inventory
        }
        Ok(related)
    }

    async fn get_frequent_keys(&self, limit: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // In a real implementation, this would query the database for most accessed orders
        let frequent_orders: Vec<String> = (1..=limit.min(100))
            .map(|i| format!("order:{:03}", i))
            .collect();
        Ok(frequent_orders)
    }

    fn get_provider_name(&self) -> &str {
        "orders"
    }
}

pub struct InventoryDataProvider;

#[async_trait::async_trait]
impl CacheDataProvider<serde_json::Value> for InventoryDataProvider {
    async fn get_data(&self, key: &str) -> Result<Option<serde_json::Value>, Box<dyn std::error::Error>> {
        if key.starts_with("inventory:") {
            let item_id = key.strip_prefix("inventory:").unwrap_or("unknown");
            let inventory_data = serde_json::json!({
                "id": item_id,
                "quantity": 100,
                "location": "warehouse_a",
                "price": 29.99
            });
            Ok(Some(inventory_data))
        } else {
            Ok(None)
        }
    }

    async fn get_related_keys(&self, key: &str) -> Result<HashSet<String>, Box<dyn std::error::Error>> {
        let mut related = HashSet::new();
        if key.starts_with("inventory:") {
            related.insert(format!("supplier:456"));
            related.insert(format!("category:electronics"));
        }
        Ok(related)
    }

    async fn get_frequent_keys(&self, limit: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let frequent_items: Vec<String> = (1..=limit.min(50))
            .map(|i| format!("inventory:item{:03}", i))
            .collect();
        Ok(frequent_items)
    }

    fn get_provider_name(&self) -> &str {
        "inventory"
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_access_pattern_recording() {
        let engine = CacheWarmingEngine::<serde_json::Value>::new(CacheWarmingConfig::default());

        // Record some access patterns
        engine.record_access("order:123").await;
        engine.record_access("order:123").await;
        engine.record_access("order:456").await;
        engine.record_access("inventory:item1").await;

        tokio::time::sleep(Duration::from_millis(10)).await; // Small delay for timestamps

        engine.record_access("order:123").await;
        engine.record_access("order:456").await;

        // Check frequent patterns
        let patterns = engine.get_frequent_patterns(5).await;
        assert_eq!(patterns.len(), 3);
        
        // order:123 should be most frequent
        assert_eq!(patterns[0].key, "order:123");
        assert_eq!(patterns[0].access_count, 3);
        
        assert_eq!(patterns[1].key, "order:456");
        assert_eq!(patterns[1].access_count, 2);
    }

    #[tokio::test]
    async fn test_warming_candidates() {
        let mut config = CacheWarmingConfig::default();
        config.min_access_count_threshold = 2;
        
        let engine = CacheWarmingEngine::<serde_json::Value>::new(config);

        // Record accesses below threshold
        engine.record_access("order:123").await;
        {
            let candidates = engine.warming_candidates.read().await;
            assert_eq!(candidates.len(), 0);
        }

        // Record more accesses to reach threshold
        engine.record_access("order:123").await;
        {
            let candidates = engine.warming_candidates.read().await;
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0], "order:123");
        }
    }

    #[tokio::test]
    async fn test_data_provider() {
        let provider = Arc::new(OrderDataProvider);
        
        // Test getting data
        let data = provider.get_data("order:123").await.unwrap().unwrap();
        assert_eq!(data["id"], "123");
        assert_eq!(data["status"], "pending");
        
        // Test getting related keys
        let related = provider.get_related_keys("order:123").await.unwrap();
        assert!(related.contains("customer:123"));
        assert!(related.contains("inventory:item1"));
        
        // Test getting frequent keys
        let frequent = provider.get_frequent_keys(5).await.unwrap();
        assert_eq!(frequent.len(), 5);
        assert!(frequent[0].starts_with("order:"));
    }

    #[tokio::test]
    async fn test_inventory_provider() {
        let provider = Arc::new(InventoryDataProvider);
        
        // Test getting inventory data
        let data = provider.get_data("inventory:item1").await.unwrap().unwrap();
        assert_eq!(data["id"], "item1");
        assert_eq!(data["quantity"], 100);
        
        // Test getting related keys
        let related = provider.get_related_keys("inventory:item1").await.unwrap();
        assert!(related.contains("supplier:456"));
        assert!(related.contains("category:electronics"));
    }
}
