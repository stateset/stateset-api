/*!
 * # Metrics Module
 *
 * This module provides a comprehensive metrics collection system for the Stateset API.
 * It exposes metrics for monitoring the health, performance, and usage of the API.
 *
 * ## Features
 *
 * - HTTP request/response metrics (count, latency, status codes)
 * - Database query performance metrics
 * - Rate limiting metrics
 * - Resource utilization metrics (memory, CPU)
 * - Business metrics (orders, inventory, etc.)
 * - Circuit breaker metrics (calls, failures, successes, state changes)
 *
 * ## Metrics Formats
 *
 * Metrics are exposed in the following formats:
 * - Prometheus text format at `/metrics`
 * - JSON format at `/metrics/json`
 * - Health dashboard at `/metrics/dashboard`
 */

use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::time::Duration;
use tracing::{debug, error, info};

// Simple in-memory metrics implementation
#[derive(Debug, Error)]
pub enum MetricsError {
    #[error("Failed to export metrics: {0}")]
    ExportError(String),
    #[error("Invalid metric name: {0}")]
    InvalidName(String),
    #[error("Metric not found: {0}")]
    NotFound(String),
}

#[derive(Debug, Clone)]
pub struct Counter {
    value: Arc<AtomicU64>,
}

impl Counter {
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_by(&self, value: u64) {
        self.value.fetch_add(value, Ordering::Relaxed);
    }

    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }
}

#[derive(Debug, Clone)]
pub struct Gauge {
    value: Arc<AtomicU64>,
}

impl Gauge {
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn set(&self, value: f64) {
        self.value.store(value as u64, Ordering::Relaxed);
    }

    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    pub fn dec(&self) {
        self.value.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn get(&self) -> f64 {
        self.value.load(Ordering::Relaxed) as f64
    }
}

#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: Arc<DashMap<String, AtomicU64>>,
    sum: Arc<AtomicU64>,
    count: Arc<AtomicU64>,
}

impl Histogram {
    pub fn new() -> Self {
        Self {
            buckets: Arc::new(DashMap::new()),
            sum: Arc::new(AtomicU64::new(0)),
            count: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn observe(&self, value: f64) {
        self.sum.fetch_add(value as u64, Ordering::Relaxed);
        self.count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    pub fn get_sum(&self) -> f64 {
        self.sum.load(Ordering::Relaxed) as f64
    }
}

#[derive(Debug)]
pub struct MetricsRegistry {
    counters: Arc<DashMap<String, Counter>>,
    gauges: Arc<DashMap<String, Gauge>>,
    histograms: Arc<DashMap<String, Histogram>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            counters: Arc::new(DashMap::new()),
            gauges: Arc::new(DashMap::new()),
            histograms: Arc::new(DashMap::new()),
        }
    }

    pub fn get_or_create_counter(&self, name: &str) -> Counter {
        self.counters
            .entry(name.to_string())
            .or_insert_with(Counter::new)
            .clone()
    }

    pub fn get_or_create_gauge(&self, name: &str) -> Gauge {
        self.gauges
            .entry(name.to_string())
            .or_insert_with(Gauge::new)
            .clone()
    }

    pub fn get_or_create_histogram(&self, name: &str) -> Histogram {
        self.histograms
            .entry(name.to_string())
            .or_insert_with(Histogram::new)
            .clone()
    }

    pub async fn export_metrics(&self) -> Result<String, MetricsError> {
        let mut output = String::new();

        // Export counters
        for entry in self.counters.iter() {
            let (name, counter) = entry.pair();
            output.push_str(&format!("# TYPE {} counter\n", name));
            output.push_str(&format!("{} {}\n", name, counter.get()));
        }

        // Export gauges
        for entry in self.gauges.iter() {
            let (name, gauge) = entry.pair();
            output.push_str(&format!("# TYPE {} gauge\n", name));
            output.push_str(&format!("{} {}\n", name, gauge.get()));
        }

        // Export histograms
        for entry in self.histograms.iter() {
            let (name, histogram) = entry.pair();
            output.push_str(&format!("# TYPE {} histogram\n", name));
            output.push_str(&format!("{}_count {}\n", name, histogram.get_count()));
            output.push_str(&format!("{}_sum {}\n", name, histogram.get_sum()));
        }

        Ok(output)
    }

    pub async fn export_metrics_json(&self) -> Result<serde_json::Value, MetricsError> {
        let mut counters = serde_json::Map::new();
        for entry in self.counters.iter() {
            let (name, counter) = entry.pair();
            counters.insert(name.to_string(), json!(counter.get()));
        }

        let mut gauges = serde_json::Map::new();
        for entry in self.gauges.iter() {
            let (name, gauge) = entry.pair();
            gauges.insert(name.to_string(), json!(gauge.get()));
        }

        let mut histograms = serde_json::Map::new();
        for entry in self.histograms.iter() {
            let (name, histogram) = entry.pair();
            histograms.insert(
                name.to_string(),
                json!({
                    "count": histogram.get_count(),
                    "sum": histogram.get_sum(),
                }),
            );
        }

        Ok(json!({
            "counters": counters,
            "gauges": gauges,
            "histograms": histograms,
        }))
    }
}

// Global metrics registry
lazy_static::lazy_static! {
    pub static ref METRICS: MetricsRegistry = MetricsRegistry::new();
}

// Metrics collection functions
pub fn increment_counter(name: &str) {
    METRICS.get_or_create_counter(name).inc();
}

pub fn increment_counter_by(name: &str, value: u64) {
    METRICS.get_or_create_counter(name).inc_by(value);
}

pub fn set_gauge(name: &str, value: f64) {
    METRICS.get_or_create_gauge(name).set(value);
}

pub fn observe_histogram(name: &str, value: f64) {
    METRICS.get_or_create_histogram(name).observe(value);
}

// Application-specific metrics
pub struct AppMetrics {
    pub requests_total: Counter,
    pub requests_duration: Histogram,
    pub database_connections: Gauge,
    pub cache_hits: Counter,
    pub cache_misses: Counter,
    pub errors_total: Counter,
}

impl AppMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: METRICS.get_or_create_counter("http_requests_total"),
            requests_duration: METRICS.get_or_create_histogram("http_request_duration_seconds"),
            database_connections: METRICS.get_or_create_gauge("database_connections_active"),
            cache_hits: METRICS.get_or_create_counter("cache_hits_total"),
            cache_misses: METRICS.get_or_create_counter("cache_misses_total"),
            errors_total: METRICS.get_or_create_counter("errors_total"),
        }
    }

    pub fn record_request(&self, duration: Duration) {
        self.requests_total.inc();
        self.requests_duration.observe(duration.as_secs_f64());
    }

    pub fn record_error(&self) {
        self.errors_total.inc();
    }

    pub fn record_cache_hit(&self) {
        self.cache_hits.inc();
    }

    pub fn record_cache_miss(&self) {
        self.cache_misses.inc();
    }

    pub fn set_database_connections(&self, count: u64) {
        self.database_connections.set(count as f64);
    }
}

// Business metrics
pub struct BusinessMetrics {
    pub orders_created: Counter,
    pub orders_completed: Counter,
    pub orders_cancelled: Counter,
    pub revenue_total: Gauge,
    pub inventory_items: Gauge,
    pub inventory_reservations: Gauge,
    pub shipments_created: Counter,
    pub shipments_delivered: Counter,
    pub returns_processed: Counter,
    pub warranties_claimed: Counter,
    pub payments_processed: Counter,
    pub payments_failed: Counter,
}

impl BusinessMetrics {
    pub fn new() -> Self {
        Self {
            orders_created: METRICS.get_or_create_counter("orders_created_total"),
            orders_completed: METRICS.get_or_create_counter("orders_completed_total"),
            orders_cancelled: METRICS.get_or_create_counter("orders_cancelled_total"),
            revenue_total: METRICS.get_or_create_gauge("revenue_total_usd"),
            inventory_items: METRICS.get_or_create_gauge("inventory_items_count"),
            inventory_reservations: METRICS.get_or_create_gauge("inventory_reservations_active"),
            shipments_created: METRICS.get_or_create_counter("shipments_created_total"),
            shipments_delivered: METRICS.get_or_create_counter("shipments_delivered_total"),
            returns_processed: METRICS.get_or_create_counter("returns_processed_total"),
            warranties_claimed: METRICS.get_or_create_counter("warranties_claimed_total"),
            payments_processed: METRICS.get_or_create_counter("payments_processed_total"),
            payments_failed: METRICS.get_or_create_counter("payments_failed_total"),
        }
    }

    pub fn record_order_created(&self) {
        self.orders_created.inc();
    }

    pub fn record_order_completed(&self) {
        self.orders_completed.inc();
    }

    pub fn record_order_cancelled(&self) {
        self.orders_cancelled.inc();
    }

    pub fn set_revenue(&self, amount: f64) {
        self.revenue_total.set(amount);
    }

    pub fn set_inventory_count(&self, count: u64) {
        self.inventory_items.set(count as f64);
    }

    pub fn set_inventory_reservations(&self, count: u64) {
        self.inventory_reservations.set(count as f64);
    }

    pub fn record_shipment_created(&self) {
        self.shipments_created.inc();
    }

    pub fn record_shipment_delivered(&self) {
        self.shipments_delivered.inc();
    }

    pub fn record_return_processed(&self) {
        self.returns_processed.inc();
    }

    pub fn record_warranty_claimed(&self) {
        self.warranties_claimed.inc();
    }

    pub fn record_payment_processed(&self) {
        self.payments_processed.inc();
    }

    pub fn record_payment_failed(&self) {
        self.payments_failed.inc();
    }
}

// Database metrics for performance monitoring
pub struct DatabaseMetrics {
    pub queries_total: Counter,
    pub query_duration: Histogram,
    pub connections_active: Gauge,
    pub connections_idle: Gauge,
    pub connection_pool_size: Gauge,
    pub transactions_total: Counter,
    pub transaction_duration: Histogram,
    pub slow_queries: Counter,
    pub query_errors: Counter,
}

impl DatabaseMetrics {
    pub fn new() -> Self {
        Self {
            queries_total: METRICS.get_or_create_counter("db_queries_total"),
            query_duration: METRICS.get_or_create_histogram("db_query_duration_seconds"),
            connections_active: METRICS.get_or_create_gauge("db_connections_active"),
            connections_idle: METRICS.get_or_create_gauge("db_connections_idle"),
            connection_pool_size: METRICS.get_or_create_gauge("db_connection_pool_size"),
            transactions_total: METRICS.get_or_create_counter("db_transactions_total"),
            transaction_duration: METRICS
                .get_or_create_histogram("db_transaction_duration_seconds"),
            slow_queries: METRICS.get_or_create_counter("db_slow_queries_total"),
            query_errors: METRICS.get_or_create_counter("db_query_errors_total"),
        }
    }

    pub fn record_query(&self, duration: Duration) {
        self.queries_total.inc();
        self.query_duration.observe(duration.as_secs_f64());

        // Track slow queries (> 1 second)
        if duration.as_secs() >= 1 {
            self.slow_queries.inc();
        }
    }

    pub fn record_query_error(&self) {
        self.query_errors.inc();
    }

    pub fn record_transaction(&self, duration: Duration) {
        self.transactions_total.inc();
        self.transaction_duration.observe(duration.as_secs_f64());
    }

    pub fn set_connection_stats(&self, active: u64, idle: u64, pool_size: u64) {
        self.connections_active.set(active as f64);
        self.connections_idle.set(idle as f64);
        self.connection_pool_size.set(pool_size as f64);
    }
}

// Security metrics for monitoring threats and access patterns
pub struct SecurityMetrics {
    pub auth_success: Counter,
    pub auth_failures: Counter,
    pub rate_limit_hits: Counter,
    pub rate_limit_exceeded: Counter,
    pub suspicious_requests: Counter,
    pub blocked_requests: Counter,
    pub active_sessions: Gauge,
    pub token_refreshes: Counter,
    pub permission_denials: Counter,
    pub api_key_usage: Counter,
}

impl SecurityMetrics {
    pub fn new() -> Self {
        Self {
            auth_success: METRICS.get_or_create_counter("auth_success_total"),
            auth_failures: METRICS.get_or_create_counter("auth_failures_total"),
            rate_limit_hits: METRICS.get_or_create_counter("rate_limit_hits_total"),
            rate_limit_exceeded: METRICS.get_or_create_counter("rate_limit_exceeded_total"),
            suspicious_requests: METRICS.get_or_create_counter("suspicious_requests_total"),
            blocked_requests: METRICS.get_or_create_counter("blocked_requests_total"),
            active_sessions: METRICS.get_or_create_gauge("active_sessions"),
            token_refreshes: METRICS.get_or_create_counter("token_refreshes_total"),
            permission_denials: METRICS.get_or_create_counter("permission_denials_total"),
            api_key_usage: METRICS.get_or_create_counter("api_key_usage_total"),
        }
    }

    pub fn record_auth_success(&self) {
        self.auth_success.inc();
    }

    pub fn record_auth_failure(&self) {
        self.auth_failures.inc();
    }

    pub fn record_rate_limit_hit(&self) {
        self.rate_limit_hits.inc();
    }

    pub fn record_rate_limit_exceeded(&self) {
        self.rate_limit_exceeded.inc();
    }

    pub fn record_suspicious_request(&self) {
        self.suspicious_requests.inc();
    }

    pub fn record_blocked_request(&self) {
        self.blocked_requests.inc();
    }

    pub fn set_active_sessions(&self, count: u64) {
        self.active_sessions.set(count as f64);
    }

    pub fn record_token_refresh(&self) {
        self.token_refreshes.inc();
    }

    pub fn record_permission_denial(&self) {
        self.permission_denials.inc();
    }

    pub fn record_api_key_usage(&self) {
        self.api_key_usage.inc();
    }
}

// HTTP endpoint-specific metrics
pub struct EndpointMetrics {
    pub requests_by_endpoint: Counter,
    pub latency_by_endpoint: Histogram,
    pub errors_by_endpoint: Counter,
    pub status_2xx: Counter,
    pub status_4xx: Counter,
    pub status_5xx: Counter,
}

impl EndpointMetrics {
    pub fn new() -> Self {
        Self {
            requests_by_endpoint: METRICS.get_or_create_counter("http_requests_by_endpoint"),
            latency_by_endpoint: METRICS.get_or_create_histogram("http_latency_by_endpoint"),
            errors_by_endpoint: METRICS.get_or_create_counter("http_errors_by_endpoint"),
            status_2xx: METRICS.get_or_create_counter("http_status_2xx_total"),
            status_4xx: METRICS.get_or_create_counter("http_status_4xx_total"),
            status_5xx: METRICS.get_or_create_counter("http_status_5xx_total"),
        }
    }

    pub fn record_request(&self, duration: Duration, status_code: u16) {
        self.requests_by_endpoint.inc();
        self.latency_by_endpoint.observe(duration.as_secs_f64());

        match status_code {
            200..=299 => self.status_2xx.inc(),
            400..=499 => {
                self.status_4xx.inc();
                self.errors_by_endpoint.inc();
            }
            500..=599 => {
                self.status_5xx.inc();
                self.errors_by_endpoint.inc();
            }
            _ => {}
        }
    }
}

// Global instances
lazy_static::lazy_static! {
    pub static ref APP_METRICS: AppMetrics = AppMetrics::new();
    pub static ref BUSINESS_METRICS: BusinessMetrics = BusinessMetrics::new();
    pub static ref DATABASE_METRICS: DatabaseMetrics = DatabaseMetrics::new();
    pub static ref SECURITY_METRICS: SecurityMetrics = SecurityMetrics::new();
    pub static ref ENDPOINT_METRICS: EndpointMetrics = EndpointMetrics::new();
}

// Middleware for automatic metrics collection
pub struct MetricsMiddleware;

impl MetricsMiddleware {
    pub fn new() -> Self {
        Self
    }
}

// Health check for metrics
pub async fn metrics_health_check() -> Result<(), MetricsError> {
    // Simple health check - just try to export metrics
    let _metrics = METRICS.export_metrics().await?;
    Ok(())
}

// Configuration for metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub export_endpoint: String,
    pub export_interval_seconds: u64,
    pub retention_days: u32,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            export_endpoint: "/metrics".to_string(),
            export_interval_seconds: 60,
            retention_days: 30,
        }
    }
}

// Metrics exporter trait
#[async_trait]
pub trait MetricsExporter: Send + Sync {
    async fn export(&self, metrics: &str) -> Result<(), MetricsError>;
}

// Console exporter for development
pub struct ConsoleExporter;

#[async_trait]
impl MetricsExporter for ConsoleExporter {
    async fn export(&self, metrics: &str) -> Result<(), MetricsError> {
        debug!("Metrics:\n{}", metrics);
        Ok(())
    }
}

// HTTP endpoint handler for metrics
pub async fn metrics_handler() -> Result<String, MetricsError> {
    METRICS.export_metrics().await
}

pub async fn metrics_json_handler() -> Result<serde_json::Value, MetricsError> {
    METRICS.export_metrics_json().await
}

// Initialize metrics system
pub async fn init_metrics(_config: &MetricsConfig) -> Result<(), MetricsError> {
    info!("Initializing metrics system");

    // Set up initial metrics
    APP_METRICS.set_database_connections(0);
    BUSINESS_METRICS.set_inventory_count(0);
    BUSINESS_METRICS.set_revenue(0.0);

    info!("Metrics system initialized successfully");
    Ok(())
}

// Utility functions
pub fn get_metrics_summary() -> String {
    format!(
        "Requests: {}, Errors: {}, Cache Hits: {}, Cache Misses: {}, Orders: {}, Returns: {}",
        APP_METRICS.requests_total.get(),
        APP_METRICS.errors_total.get(),
        APP_METRICS.cache_hits.get(),
        APP_METRICS.cache_misses.get(),
        BUSINESS_METRICS.orders_created.get(),
        BUSINESS_METRICS.returns_processed.get()
    )
}
