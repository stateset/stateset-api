use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct CircuitBreaker {
    failure_threshold: usize,
    reset_timeout: Duration,
    failure_count: AtomicUsize,
    last_failure_time: Mutex<Option<Instant>>,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, reset_timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            failure_threshold,
            reset_timeout,
            failure_count: AtomicUsize::new(0),
            last_failure_time: Mutex::new(None),
        })
    }

    pub async fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        *self.last_failure_time.lock().await = None;
    }

    pub async fn record_failure(&self) {
        let count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= self.failure_threshold {
            *self.last_failure_time.lock().await = Some(Instant::now());
        }
    }

    pub async fn is_open(&self) -> bool {
        if self.failure_count.load(Ordering::Relaxed) >= self.failure_threshold {
            if let Some(last_failure) = *self.last_failure_time.lock().await {
                if last_failure.elapsed() < self.reset_timeout {
                    return true;
                }
            }
        }
        false
    }
}