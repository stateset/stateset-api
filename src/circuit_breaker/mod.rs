// circuit_breaker/mod.rs

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::RwLock;
use tokio::time::sleep;
use thiserror::Error;
use log::{info, warn};

/// Custom error type for CircuitBreaker operations.
#[derive(Error, Debug)]
pub enum CircuitBreakerError {
    #[error("Circuit is open and not allowing the operation.")]
    CircuitOpen,
    
    #[error("Circuit breaker encountered an internal error: {0}")]
    InternalError(String),
}

/// Enum representing the state of the Circuit Breaker.
#[derive(Debug, Clone, PartialEq, Eq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

/// Circuit Breaker implementation to prevent cascading failures.
/// It transitions between states based on the number of failures and timeout durations.
pub struct CircuitBreaker {
    /// Maximum number of consecutive failures allowed before opening the circuit.
    failure_threshold: usize,
    
    /// Duration to wait before attempting to reset the circuit from Open to HalfOpen.
    reset_timeout: Duration,
    
    /// Maximum number of trial requests allowed in HalfOpen state.
    half_open_max_trials: usize,
    
    /// Current state of the circuit.
    state: RwLock<CircuitState>,
    
    /// Counts the number of consecutive failures.
    failure_count: AtomicUsize,
    
    /// Counts the number of trial requests in HalfOpen state.
    half_open_trials: AtomicUsize,
    
    /// Timestamp of the last failure.
    last_failure_time: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    /// Creates a new CircuitBreaker instance.
    ///
    /// # Arguments
    ///
    /// * `failure_threshold` - Number of consecutive failures to trigger the circuit to Open state.
    /// * `reset_timeout` - Duration to wait before transitioning from Open to HalfOpen state.
    /// * `half_open_max_trials` - Number of trial requests allowed in HalfOpen state.
    ///
    /// # Returns
    ///
    /// An `Arc` pointing to the newly created CircuitBreaker.
    pub fn new(failure_threshold: usize, reset_timeout: Duration, half_open_max_trials: usize) -> Arc<Self> {
        Arc::new(Self {
            failure_threshold,
            reset_timeout,
            half_open_max_trials,
            state: RwLock::new(CircuitState::Closed),
            failure_count: AtomicUsize::new(0),
            half_open_trials: AtomicUsize::new(0),
            last_failure_time: RwLock::new(None),
        })
    }
    
    /// Attempts to execute an operation guarded by the circuit breaker.
    ///
    /// # Arguments
    ///
    /// * `operation` - The asynchronous operation to execute.
    ///
    /// # Returns
    ///
    /// The result of the operation if allowed, or an error if the circuit is open.
    pub async fn call<F, T>(&self, operation: F) -> Result<T, CircuitBreakerError>
    where
        F: FnOnce() -> F + Send + 'static,
        F: std::future::Future<Output = Result<T, CircuitBreakerError>> + Send,
        T: Send,
    {
        // Check the current state
        {
            let state = self.state.read().await;
            match *state {
                CircuitState::Open => {
                    // Check if reset_timeout has elapsed to transition to HalfOpen
                    let should_try_reset = {
                        let last_failure = self.last_failure_time.read().await;
                        if let Some(last_failure_time) = *last_failure {
                            last_failure_time.elapsed() >= self.reset_timeout
                        } else {
                            false
                        }
                    };
                    
                    if should_try_reset {
                        drop(state); // Release the read lock before acquiring write lock
                        let mut state = self.state.write().await;
                        if *state == CircuitState::Open {
                            *state = CircuitState::HalfOpen;
                            self.half_open_trials.store(0, Ordering::Relaxed);
                            info!("CircuitBreaker transitioned to HalfOpen state.");
                        }
                    } else {
                        return Err(CircuitBreakerError::CircuitOpen);
                    }
                },
                CircuitState::HalfOpen => {
                    // Allow a limited number of trial requests
                    let trials = self.half_open_trials.fetch_add(1, Ordering::Relaxed) + 1;
                    if trials > self.half_open_max_trials {
                        return Err(CircuitBreakerError::CircuitOpen);
                    }
                },
                CircuitState::Closed => {},
            }
        }
        
        // Execute the operation
        let result = operation().await;
        
        match result {
            Ok(value) => {
                // On success, reset the failure count and possibly close the circuit
                self.record_success().await;
                Ok(value)
            },
            Err(e) => {
                // On failure, record the failure and possibly open the circuit
                self.record_failure().await;
                Err(e)
            },
        }
    }
    
    /// Records a successful operation.
    /// Resets the failure count and transitions the circuit to Closed state if it was HalfOpen.
    async fn record_success(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        
        let mut state = self.state.write().await;
        if *state == CircuitState::HalfOpen {
            *state = CircuitState::Closed;
            info!("CircuitBreaker transitioned to Closed state after successful trial.");
        }
        *self.last_failure_time.write().await = None;
    }
    
    /// Records a failed operation.
    /// Increments the failure count and transitions the circuit to Open state if threshold is reached.
    async fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        if failures >= self.failure_threshold {
            let mut state = self.state.write().await;
            if *state != CircuitState::Open {
                *state = CircuitState::Open;
                *self.last_failure_time.write().await = Some(Instant::now());
                warn!("CircuitBreaker transitioned to Open state after reaching failure threshold.");
            }
        }
    }
    
    /// Checks if the circuit is currently Open.
    ///
    /// # Returns
    ///
    /// `true` if the circuit is Open, otherwise `false`.
    pub async fn is_open(&self) -> bool {
        let state = self.state.read().await;
        *state == CircuitState::Open
    }
    
    /// Resets the circuit breaker to Closed state manually.
    /// Useful for administrative purposes.
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        *state = CircuitState::Closed;
        self.failure_count.store(0, Ordering::Relaxed);
        *self.last_failure_time.write().await = None;
        info!("CircuitBreaker manually reset to Closed state.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;
    use std::time::Duration;
    use std::sync::Arc;

    /// Helper function to create a successful operation.
    async fn successful_operation() -> Result<&'static str, CircuitBreakerError> {
        Ok("Success")
    }

    /// Helper function to create a failing operation.
    async fn failing_operation() -> Result<&'static str, CircuitBreakerError> {
        Err(CircuitBreakerError::CircuitOpen)
    }

    #[tokio::test]
    async fn test_circuit_breaker_closed_to_open() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(2), 1);
        
        // Simulate 3 consecutive failures
        for _ in 0..3 {
            let result = cb.call(failing_operation).await;
            assert!(result.is_err());
        }

        // Circuit should now be Open
        assert!(cb.is_open().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_open_to_half_open_to_closed() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(1), 1);
        
        // Trigger Open state
        for _ in 0..2 {
            let result = cb.call(failing_operation).await;
            assert!(result.is_err());
        }
        assert!(cb.is_open().await);
        
        // Wait for reset_timeout to transition to HalfOpen
        sleep(Duration::from_secs(2)).await;
        
        // Attempt a trial request (success)
        let result = cb.call(successful_operation).await;
        assert!(result.is_ok());
        
        // Circuit should now be Closed
        assert!(!cb.is_open().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_to_open() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(1), 1);
        
        // Trigger Open state
        for _ in 0..2 {
            let result = cb.call(failing_operation).await;
            assert!(result.is_err());
        }
        assert!(cb.is_open().await);
        
        // Wait for reset_timeout to transition to HalfOpen
        sleep(Duration::from_secs(2)).await;
        
        // Attempt a trial request (failure)
        let result = cb.call(failing_operation).await;
        assert!(result.is_err());
        
        // Circuit should remain Open
        assert!(cb.is_open().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_manual_reset() {
        let cb = CircuitBreaker::new(1, Duration::from_secs(1), 1);
        
        // Trigger Open state
        let result = cb.call(failing_operation).await;
        assert!(result.is_err());
        assert!(cb.is_open().await);
        
        // Manual reset
        cb.reset().await;
        assert!(!cb.is_open().await);
        
        // Successful operation should proceed
        let result = cb.call(successful_operation).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_multiple_trials() {
        let cb = CircuitBreaker::new(2, Duration::from_secs(1), 1);
        
        // Trigger Open state
        for _ in 0..2 {
            let result = cb.call(failing_operation).await;
            assert!(result.is_err());
        }
        assert!(cb.is_open().await);
        
        // Wait for reset_timeout to transition to HalfOpen
        sleep(Duration::from_secs(2)).await;
        
        // First trial request (success)
        let result = cb.call(successful_operation).await;
        assert!(result.is_ok());
        
        // Circuit should now be Closed
        assert!(!cb.is_open().await);
        
        // Second trial request (should be normal)
        let result = cb.call(successful_operation).await;
        assert!(result.is_ok());
    }
}
