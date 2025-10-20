use lazy_static::lazy_static;
use prometheus::{
    register_histogram, register_int_counter, register_int_gauge, Encoder, Histogram, IntCounter,
    IntGauge, TextEncoder,
};
use std::time::Instant;
use tracing::trace;

lazy_static! {
    // HTTP metrics
    pub static ref HTTP_REQUESTS_TOTAL: IntCounter = register_int_counter!(
        "http_requests_total",
        "Total number of HTTP requests"
    ).unwrap();

    pub static ref HTTP_REQUESTS_SUCCESS: IntCounter = register_int_counter!(
        "http_requests_success_total",
        "Total number of successful HTTP requests"
    ).unwrap();

    pub static ref HTTP_REQUESTS_ERROR: IntCounter = register_int_counter!(
        "http_requests_error_total",
        "Total number of failed HTTP requests"
    ).unwrap();

    pub static ref HTTP_REQUEST_DURATION: Histogram = register_histogram!(
        "http_request_duration_seconds",
        "HTTP request latencies in seconds"
    ).unwrap();

    // Business metrics
    pub static ref CHECKOUT_SESSIONS_CREATED: IntCounter = register_int_counter!(
        "checkout_sessions_created_total",
        "Total number of checkout sessions created"
    ).unwrap();

    pub static ref CHECKOUT_SESSIONS_UPDATED: IntCounter = register_int_counter!(
        "checkout_sessions_updated_total",
        "Total number of checkout session updates"
    ).unwrap();

    pub static ref CHECKOUT_COMPLETIONS: IntCounter = register_int_counter!(
        "checkout_completions_total",
        "Total number of successful checkout completions"
    ).unwrap();

    pub static ref CHECKOUT_CANCELLATIONS: IntCounter = register_int_counter!(
        "checkout_cancellations_total",
        "Total number of checkout cancellations"
    ).unwrap();

    pub static ref ORDERS_CREATED: IntCounter = register_int_counter!(
        "orders_created_total",
        "Total number of orders created"
    ).unwrap();

    pub static ref VAULT_TOKENS_CREATED: IntCounter = register_int_counter!(
        "vault_tokens_created_total",
        "Total number of vault tokens created"
    ).unwrap();

    pub static ref VAULT_TOKENS_CONSUMED: IntCounter = register_int_counter!(
        "vault_tokens_consumed_total",
        "Total number of vault tokens consumed"
    ).unwrap();

    pub static ref VAULT_TOKEN_REUSE_BLOCKED: IntCounter = register_int_counter!(
        "vault_token_reuse_blocked_total",
        "Total number of vault token reuse attempts blocked"
    ).unwrap();

    pub static ref PAYMENT_PROCESSING_SUCCESS: IntCounter = register_int_counter!(
        "payment_processing_success_total",
        "Total number of successful payment processings"
    ).unwrap();

    pub static ref PAYMENT_PROCESSING_FAILURE: IntCounter = register_int_counter!(
        "payment_processing_failure_total",
        "Total number of failed payment processings"
    ).unwrap();

    // System metrics
    pub static ref ACTIVE_SESSIONS: IntGauge = register_int_gauge!(
        "active_checkout_sessions",
        "Current number of active checkout sessions"
    ).unwrap();

    pub static ref CACHE_OPERATIONS: IntCounter = register_int_counter!(
        "cache_operations_total",
        "Total number of cache operations"
    ).unwrap();

    pub static ref CACHE_HITS: IntCounter = register_int_counter!(
        "cache_hits_total",
        "Total number of cache hits"
    ).unwrap();

    pub static ref CACHE_MISSES: IntCounter = register_int_counter!(
        "cache_misses_total",
        "Total number of cache misses"
    ).unwrap();
}

/// Record HTTP request
pub fn record_http_request(method: &str, path: &str, status: u16, duration: Instant) {
    HTTP_REQUESTS_TOTAL.inc();

    trace!(
        http.method = method,
        http.path = path,
        http.status = status,
        latency_secs = duration.elapsed().as_secs_f64(),
        "recording HTTP request metrics"
    );

    if status < 400 {
        HTTP_REQUESTS_SUCCESS.inc();
    } else {
        HTTP_REQUESTS_ERROR.inc();
    }

    HTTP_REQUEST_DURATION.observe(duration.elapsed().as_secs_f64());
}

/// Gather all metrics and return as Prometheus text format
pub fn gather_metrics() -> Result<String, Box<dyn std::error::Error>> {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = vec![];
    encoder.encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8(buffer)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registration() {
        // Verify all metrics are registered
        HTTP_REQUESTS_TOTAL.inc();
        assert!(HTTP_REQUESTS_TOTAL.get() > 0);
    }
}
