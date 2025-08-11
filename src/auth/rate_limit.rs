/*!
 * # Rate Limiting Module for Authentication
 *
 * This module provides specialized rate limiting for authentication-related
 * endpoints to prevent brute force attacks.
 */

use axum::{
    extract::{ConnectInfo, State},
    http::Request,
    middleware::Next,
    response::Response,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};
use tracing::debug;

/// Rate limit configuration
#[derive(Clone, Debug)]
pub struct AuthRateLimitConfig {
    pub login_max_attempts: u32,
    pub login_window: Duration,
    pub login_lockout_duration: Duration,
    pub signup_max_attempts: u32,
    pub signup_window: Duration,
    pub api_key_creation_max: u32,
    pub api_key_creation_window: Duration,
    pub password_reset_max: u32,
    pub password_reset_window: Duration,
}

impl Default for AuthRateLimitConfig {
    fn default() -> Self {
        Self {
            login_max_attempts: 5,
            login_window: Duration::from_secs(60 * 5), // 5 minutes
            login_lockout_duration: Duration::from_secs(60 * 15), // 15 minutes
            signup_max_attempts: 3,
            signup_window: Duration::from_secs(60 * 60), // 1 hour
            api_key_creation_max: 5,
            api_key_creation_window: Duration::from_secs(60 * 60 * 24), // 24 hours
            password_reset_max: 3,
            password_reset_window: Duration::from_secs(60 * 60), // 1 hour
        }
    }
}

/// Rate limit entry
#[derive(Debug, Clone)]
struct RateLimitEntry {
    attempts: u32,
    first_attempt: Instant,
    locked_until: Option<Instant>,
}

impl RateLimitEntry {
    fn new() -> Self {
        Self {
            attempts: 1,
            first_attempt: Instant::now(),
            locked_until: None,
        }
    }

    fn increment(&mut self) -> u32 {
        self.attempts += 1;
        self.attempts
    }

    fn is_locked(&self) -> bool {
        if let Some(locked_until) = self.locked_until {
            Instant::now() < locked_until
        } else {
            false
        }
    }

    fn lock(&mut self, duration: Duration) {
        self.locked_until = Some(Instant::now() + duration);
    }

    fn time_since_first(&self) -> Duration {
        Instant::now().duration_since(self.first_attempt)
    }

    fn remaining_lockout(&self) -> Option<Duration> {
        self.locked_until.map(|t| {
            if t > Instant::now() {
                t.duration_since(Instant::now())
            } else {
                Duration::from_secs(0)
            }
        })
    }

    fn should_reset(&self, window: Duration) -> bool {
        self.time_since_first() > window
    }
}

/// Rate limit type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum RateLimitType {
    Login,
    Signup,
    ApiKeyCreation,
    PasswordReset,
}

/// Auth rate limiter for preventing brute force attacks
#[derive(Clone)]
pub struct AuthRateLimiter {
    config: AuthRateLimitConfig,
    limits: Arc<Mutex<HashMap<(String, RateLimitType), RateLimitEntry>>>,
}

impl AuthRateLimiter {
    /// Create a new auth rate limiter
    pub fn new(config: AuthRateLimitConfig) -> Self {
        Self {
            config,
            limits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if a key is rate limited
    pub async fn check(&self, key: &str, limit_type: RateLimitType) -> Result<(), RateLimitError> {
        let mut limits = self.limits.lock().await;
        let entry_key = (key.to_string(), limit_type);

        // Get or create entry
        let entry = limits
            .entry(entry_key.clone())
            .or_insert_with(RateLimitEntry::new);

        // Check if locked
        if entry.is_locked() {
            let remaining = entry.remaining_lockout().unwrap_or(Duration::from_secs(0));
            return Err(RateLimitError::AccountLocked {
                remaining_seconds: remaining.as_secs(),
                limit_type,
            });
        }

        // Get config values for this limit type
        let (max_attempts, window, lockout_duration) = match limit_type {
            RateLimitType::Login => (
                self.config.login_max_attempts,
                self.config.login_window,
                Some(self.config.login_lockout_duration),
            ),
            RateLimitType::Signup => (
                self.config.signup_max_attempts,
                self.config.signup_window,
                None,
            ),
            RateLimitType::ApiKeyCreation => (
                self.config.api_key_creation_max,
                self.config.api_key_creation_window,
                None,
            ),
            RateLimitType::PasswordReset => (
                self.config.password_reset_max,
                self.config.password_reset_window,
                None,
            ),
        };

        // Reset if window has passed
        if entry.should_reset(window) {
            *entry = RateLimitEntry::new();
            return Ok(());
        }

        // Check attempts
        if entry.attempts >= max_attempts {
            // Lock account if lockout duration is set
            if let Some(duration) = lockout_duration {
                entry.lock(duration);
                return Err(RateLimitError::AccountLocked {
                    remaining_seconds: duration.as_secs(),
                    limit_type,
                });
            }

            return Err(RateLimitError::TooManyAttempts {
                limit_type,
                max_attempts,
                retry_after: (window - entry.time_since_first()).as_secs(),
            });
        }

        // Increment attempts
        entry.increment();
        Ok(())
    }

    /// Record a successful attempt (resets the counter)
    pub async fn record_success(&self, key: &str, limit_type: RateLimitType) {
        let mut limits = self.limits.lock().await;
        limits.remove(&(key.to_string(), limit_type));
    }

    /// Clean up old entries
    pub async fn cleanup(&self) {
        let mut limits = self.limits.lock().await;

        // Remove entries that should be reset
        limits.retain(|&(_, limit_type), entry| {
            let window = match limit_type {
                RateLimitType::Login => self.config.login_window,
                RateLimitType::Signup => self.config.signup_window,
                RateLimitType::ApiKeyCreation => self.config.api_key_creation_window,
                RateLimitType::PasswordReset => self.config.password_reset_window,
            };

            !entry.should_reset(window * 2) // Keep entries for 2x the window
        });
    }
}

impl Default for AuthRateLimiter {
    fn default() -> Self {
        Self::new(AuthRateLimitConfig::default())
    }
}

/// Rate limit error
#[derive(Debug, thiserror::Error, Serialize)]
pub enum RateLimitError {
    #[error("Too many attempts ({max_attempts}). Please try again in {retry_after} seconds.")]
    TooManyAttempts {
        limit_type: RateLimitType,
        max_attempts: u32,
        retry_after: u64,
    },

    #[error("Account locked. Please try again in {remaining_seconds} seconds.")]
    AccountLocked {
        limit_type: RateLimitType,
        remaining_seconds: u64,
    },
}

impl axum::response::IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        use axum::http::StatusCode;
        use axum::Json;

        let (status, headers, body) = match &self {
            Self::TooManyAttempts { retry_after, .. } => {
                let headers = [(axum::http::header::RETRY_AFTER, retry_after.to_string())];
                (StatusCode::TOO_MANY_REQUESTS, headers, Json(self))
            }
            Self::AccountLocked {
                remaining_seconds, ..
            } => {
                let headers = [(
                    axum::http::header::RETRY_AFTER,
                    remaining_seconds.to_string(),
                )];
                (StatusCode::TOO_MANY_REQUESTS, headers, Json(self))
            }
        };

        (status, headers, body).into_response()
    }
}

/// Middleware that applies rate limiting to authentication endpoints
pub async fn auth_rate_limit_middleware(
    limit_type: RateLimitType,
    State(rate_limiter): State<Arc<AuthRateLimiter>>,
    ConnectInfo(addr): ConnectInfo<std::net::SocketAddr>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Use client IP as the rate limit key
    let key = addr.ip().to_string();

    // Check rate limit
    rate_limiter.check(&key, limit_type).await?;

    // Process the request
    Ok(next.run(request).await)
}

/// Background task to clean up old rate limit entries
pub async fn cleanup_rate_limits(rate_limiter: Arc<AuthRateLimiter>) {
    loop {
        // Clean up every hour
        sleep(Duration::from_secs(60 * 60)).await;
        rate_limiter.cleanup().await;
    }
}
