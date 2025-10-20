/*!
 * # Circuit Breaker Implementation
 *
 * This module provides a circuit breaker pattern implementation for handling
 * service failures gracefully and preventing cascading failures.
 */

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Circuit breaker states
#[derive(Debug, Clone, PartialEq)]
pub enum CircuitState {
    /// Circuit is closed, allowing requests
    Closed,
    /// Circuit is open, rejecting requests
    Open,
    /// Circuit is half-open, allowing limited requests to test recovery
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Maximum number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Duration to wait before transitioning from Open to HalfOpen
    pub timeout: Duration,
    /// Number of successful requests needed in HalfOpen to close the circuit
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
        }
    }
}

/// Internal state of the circuit breaker
#[derive(Debug)]
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
}

/// Circuit breaker implementation
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Arc<Mutex<CircuitBreakerState>>,
}

/// Circuit breaker errors
#[derive(Error, Debug)]
pub enum CircuitBreakerError {
    #[error("Circuit breaker is open")]
    CircuitOpen,
    #[error("Service call failed: {0}")]
    ServiceFailure(String),
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration
    pub fn new(failure_threshold: u32, timeout: Duration, success_threshold: u32) -> Self {
        let config = CircuitBreakerConfig {
            failure_threshold,
            timeout,
            success_threshold,
        };

        Self {
            config,
            state: Arc::new(Mutex::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
            })),
        }
    }

    /// Create a circuit breaker with custom configuration
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
            })),
        }
    }

    /// Execute a closure with circuit breaker protection
    pub async fn call<F, R, E>(&self, f: F) -> Result<R, CircuitBreakerError>
    where
        F: FnOnce() -> Result<R, E>,
        E: std::fmt::Display,
    {
        // Check if we can make the call
        if !self.can_execute() {
            return Err(CircuitBreakerError::CircuitOpen);
        }

        // Execute the function
        match f() {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(err) => {
                self.on_failure();
                Err(CircuitBreakerError::ServiceFailure(err.to_string()))
            }
        }
    }

    /// Check if the circuit breaker allows execution
    fn can_execute(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        match state.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if let Some(last_failure) = state.last_failure_time {
                    if last_failure.elapsed() >= self.config.timeout {
                        // Transition to half-open
                        state.state = CircuitState::HalfOpen;
                        state.success_count = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Handle successful execution
    fn on_success(&self) {
        let mut state = self.state.lock().unwrap();

        match state.state {
            CircuitState::Closed => {
                state.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                state.success_count += 1;
                if state.success_count >= self.config.success_threshold {
                    // Close the circuit
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.last_failure_time = None;
                }
            }
            CircuitState::Open => {
                // This shouldn't happen, but reset anyway
                state.state = CircuitState::Closed;
                state.failure_count = 0;
                state.success_count = 0;
                state.last_failure_time = None;
            }
        }
    }

    /// Handle failed execution
    fn on_failure(&self) {
        let mut state = self.state.lock().unwrap();

        state.failure_count += 1;
        state.last_failure_time = Some(Instant::now());

        match state.state {
            CircuitState::Closed => {
                if state.failure_count >= self.config.failure_threshold {
                    state.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Go back to open on any failure in half-open state
                state.state = CircuitState::Open;
                state.success_count = 0;
            }
            CircuitState::Open => {
                // Already open, just update the failure time
            }
        }
    }

    /// Get the current state of the circuit breaker
    pub fn state(&self) -> CircuitState {
        self.state.lock().unwrap().state.clone()
    }

    /// Get circuit breaker metrics
    pub fn metrics(&self) -> CircuitBreakerMetrics {
        let state = self.state.lock().unwrap();
        CircuitBreakerMetrics {
            state: state.state.clone(),
            failure_count: state.failure_count,
            success_count: state.success_count,
        }
    }
}

/// Circuit breaker metrics
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
}

/// Registry for managing multiple circuit breakers
use std::collections::HashMap;

#[derive(Debug)]
pub struct CircuitBreakerRegistry {
    breakers: Arc<Mutex<HashMap<String, Arc<CircuitBreaker>>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerRegistry {
    /// Create a new circuit breaker registry
    pub fn new(default_config: Option<CircuitBreakerConfig>) -> Self {
        Self {
            breakers: Arc::new(Mutex::new(HashMap::new())),
            default_config: default_config.unwrap_or_default(),
        }
    }

    /// Get or create a circuit breaker for the given service
    pub fn get(&self, service_name: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self.breakers.lock().unwrap();

        if let Some(breaker) = breakers.get(service_name) {
            breaker.clone()
        } else {
            let breaker = Arc::new(CircuitBreaker::new(
                self.default_config.failure_threshold,
                self.default_config.timeout,
                self.default_config.success_threshold,
            ));
            breakers.insert(service_name.to_string(), breaker.clone());
            breaker
        }
    }

    /// Get all circuit breaker metrics
    pub fn metrics(&self) -> HashMap<String, CircuitBreakerMetrics> {
        let breakers = self.breakers.lock().unwrap();
        breakers
            .iter()
            .map(|(name, breaker)| (name.clone(), breaker.metrics()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(100), 2);

        // Should be closed initially
        assert_eq!(cb.state(), CircuitState::Closed);

        // Successful calls should keep it closed
        let result = cb.call(|| Ok::<i32, &str>(42)).await;
        assert!(result.is_ok());
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_on_failures() {
        let cb = CircuitBreaker::new(2, Duration::from_millis(100), 2);

        // First failure
        let _result = cb.call(|| Err::<i32, &str>("error")).await;
        assert_eq!(cb.state(), CircuitState::Closed);

        // Second failure should open the circuit
        let _result = cb.call(|| Err::<i32, &str>("error")).await;
        assert_eq!(cb.state(), CircuitState::Open);

        // Next call should be rejected
        let result = cb.call(|| Ok::<i32, &str>(42)).await;
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }
}
