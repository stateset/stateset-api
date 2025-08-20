use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{warn, debug};

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Factor to multiply delay by after each attempt
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_factor: 2.0,
        }
    }
}

/// Retry policy for determining if an error is retryable
pub trait RetryPolicy<E> {
    fn is_retryable(&self, error: &E) -> bool;
}

/// Default retry policy for database errors
pub struct DbRetryPolicy;

impl RetryPolicy<sea_orm::DbErr> for DbRetryPolicy {
    fn is_retryable(&self, error: &sea_orm::DbErr) -> bool {
        use sea_orm::DbErr;
        
        matches!(
            error,
            DbErr::ConnectionAcquire(_) |
            DbErr::Conn(_) |
            DbErr::ConnAcquire(_) |
            DbErr::TryIntoErr { .. }
        )
    }
}

/// Execute an async function with retries
pub async fn with_retry<F, Fut, T, E>(
    config: &RetryConfig,
    policy: impl RetryPolicy<E>,
    mut operation: F,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut delay = config.initial_delay;
    let mut attempts = 0;
    
    loop {
        attempts += 1;
        
        match operation().await {
            Ok(result) => {
                if attempts > 1 {
                    debug!("Operation succeeded after {} attempts", attempts);
                }
                return Ok(result);
            }
            Err(error) => {
                if attempts >= config.max_attempts || !policy.is_retryable(&error) {
                    warn!(
                        "Operation failed after {} attempts: {}",
                        attempts, error
                    );
                    return Err(error);
                }
                
                warn!(
                    "Attempt {} failed: {}. Retrying in {:?}...",
                    attempts, error, delay
                );
                
                sleep(delay).await;
                
                // Calculate next delay with exponential backoff
                delay = Duration::from_secs_f64(
                    (delay.as_secs_f64() * config.backoff_factor).min(config.max_delay.as_secs_f64())
                );
            }
        }
    }
}

/// Extension trait for adding retry capability to futures
pub trait RetryExt: Sized {
    type Output;
    type Error;
    
    fn with_retry(
        self,
        config: RetryConfig,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>>
    where
        Self::Error: std::fmt::Display;
}

/// Macro to easily add retry to database operations
#[macro_export]
macro_rules! retry_db {
    ($operation:expr) => {{
        use $crate::middleware::retry::{with_retry, RetryConfig, DbRetryPolicy};
        
        with_retry(
            &RetryConfig::default(),
            DbRetryPolicy,
            || async { $operation.await }
        ).await
    }};
    ($operation:expr, $config:expr) => {{
        use $crate::middleware::retry::{with_retry, DbRetryPolicy};
        
        with_retry(
            &$config,
            DbRetryPolicy,
            || async { $operation.await }
        ).await
    }};
}