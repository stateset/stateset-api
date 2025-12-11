use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;
use std::env as std_env;
use std::path::Path;
use thiserror::Error;
use tracing::{error, info};
use validator::{Validate, ValidationError, ValidationErrors};

/// Default values for configuration
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_ENV: &str = "development";
const DEFAULT_PORT: u16 = 8080;
const CONFIG_DIR: &str = "config";
const DEFAULT_CACHE_TYPE: &str = "in-memory";
const DEFAULT_CACHE_CAPACITY: usize = 1000;
const DEFAULT_CLEANUP_INTERVAL: u64 = 60; // 60 seconds
const DEFAULT_RATE_LIMIT_REQUESTS: u32 = 100;
const DEFAULT_RATE_LIMIT_WINDOW_SECS: u64 = 60;
const DEFAULT_MESSAGE_QUEUE_BACKEND: &str = "in-memory";
const DEFAULT_MESSAGE_QUEUE_NAMESPACE: &str = "stateset:mq";
const DEFAULT_MESSAGE_QUEUE_BLOCK_TIMEOUT_SECS: u64 = 5;
const DEFAULT_RATE_LIMIT_NAMESPACE: &str = "stateset:rl";
const DEV_DEFAULT_JWT_SECRET: &str =
    "this_is_a_development_secret_key_that_is_at_least_64_characters_long_for_testing";

/// Cache configuration
#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct CacheConfig {
    /// Type of cache to use: "in-memory", "redis", or "multi-level"
    #[serde(default = "default_cache_type")]
    pub cache_type: String,

    /// Redis connection URL for cache
    pub redis_url: String,

    /// Maximum number of in-memory cache entries
    #[serde(default = "default_cache_capacity")]
    pub capacity: usize,

    /// Interval in seconds for cleaning expired entries
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval_secs: u64,

    /// Default TTL (Time To Live) for cache entries in seconds
    #[serde(default)]
    pub default_ttl_secs: Option<u64>,

    /// Enable cache debug logging
    #[serde(default)]
    pub debug: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            cache_type: default_cache_type(),
            redis_url: "redis://localhost:6379".to_string(),
            capacity: default_cache_capacity(),
            cleanup_interval_secs: default_cleanup_interval(),
            default_ttl_secs: Some(300), // Default 5 minutes
            debug: false,
        }
    }
}

/// Application configuration structure with validation
#[derive(Clone, Debug, Deserialize, Validate)]
#[serde(deny_unknown_fields)]
pub struct AppConfig {
    /// Database connection URL
    pub database_url: String,

    /// Database URL alias (for backward compatibility)
    #[serde(skip)]
    pub db_url: String,

    /// Redis connection URL
    pub redis_url: String,

    /// JWT secret key (minimum 64 characters for enhanced security)
    #[validate(length(min = 64), custom = "validate_jwt_secret")]
    pub jwt_secret: String,

    /// JWT expiration time in seconds (5min - 24h)
    pub jwt_expiration: usize,

    /// Refresh token expiration (1d - 30d)
    pub refresh_token_expiration: usize,

    /// Server host address
    pub host: String,

    /// Server port (1024-65535)
    #[serde(default = "default_port")]
    pub port: u16,

    /// gRPC server port (optional, defaults to port + 1)
    pub grpc_port: Option<u16>,

    /// Application environment
    pub environment: String,

    /// Logging level
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Log in JSON format (structured logging)
    #[serde(default)]
    pub log_json: bool,

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,

    /// Whether to run database migrations on startup
    #[serde(default)]
    pub auto_migrate: bool,

    /// CORS: comma-separated list of allowed origins (production)
    #[serde(default)]
    pub cors_allowed_origins: Option<String>,

    /// Allow permissive CORS fallback
    #[serde(default = "default_false_bool")]
    pub cors_allow_any_origin: bool,

    /// CORS: allow credentials
    #[serde(default)]
    pub cors_allow_credentials: bool,

    /// DB pool: max connections
    #[serde(default = "default_db_max_connections")]
    pub db_max_connections: u32,

    /// DB pool: min connections
    #[serde(default = "default_db_min_connections")]
    pub db_min_connections: u32,

    /// DB timeouts (seconds)
    #[serde(default = "default_db_connect_timeout_secs")]
    pub db_connect_timeout_secs: u64,
    #[serde(default = "default_db_idle_timeout_secs")]
    pub db_idle_timeout_secs: u64,
    #[serde(default = "default_db_acquire_timeout_secs")]
    pub db_acquire_timeout_secs: u64,
    /// Statement timeout (seconds), 0 = disabled
    #[serde(default)]
    pub db_statement_timeout_secs: Option<u64>,

    /// Rate limiting: requests per window
    #[serde(default = "default_rate_limit_requests")]
    pub rate_limit_requests_per_window: u32,
    /// Rate limiting: window size (seconds)
    #[serde(default = "default_rate_limit_window_secs")]
    pub rate_limit_window_seconds: u64,
    /// Rate limiting: include headers
    #[serde(default = "default_true_bool")]
    pub rate_limit_enable_headers: bool,

    /// Rate limiting: API key policies `api_key:limit:window_secs` comma-separated
    #[serde(default)]
    pub rate_limit_api_key_policies: Option<String>,

    /// Rate limiting: User policies `user_id:limit:window_secs` comma-separated
    #[serde(default)]
    pub rate_limit_user_policies: Option<String>,

    /// Rate limit path policies: comma-separated list of `prefix:limit:window_secs`
    /// Example: "/api/v1/orders:60:60,/api/v1/inventory:120:60"
    #[serde(default)]
    pub rate_limit_path_policies: Option<String>,

    /// Enable Redis-backed rate limiter
    #[serde(default = "default_false_bool")]
    pub rate_limit_use_redis: bool,

    /// Namespace for rate limiter keys when Redis is enabled
    #[serde(default = "default_rate_limit_namespace")]
    pub rate_limit_namespace: String,

    /// Message queue backend selection ("in-memory" or "redis")
    #[serde(default = "default_message_queue_backend")]
    #[validate(custom = "validate_message_queue_backend")]
    pub message_queue_backend: String,

    /// Namespace prefix for queue keys when using Redis backend
    #[serde(default = "default_message_queue_namespace")]
    pub message_queue_namespace: String,

    /// Blocking timeout (seconds) for queue subscriptions
    #[serde(default = "default_message_queue_block_timeout_secs")]
    pub message_queue_block_timeout_secs: u64,

    /// Payment provider identifier (e.g., "stripe")
    #[serde(default)]
    pub payment_provider: Option<String>,

    /// Default tax rate (as decimal, e.g., 0.08 for 8%)
    #[serde(default = "default_tax_rate")]
    #[validate(custom = "validate_tax_rate")]
    pub default_tax_rate: f64,

    /// Event channel capacity for async event processing
    #[serde(default = "default_event_channel_capacity")]
    #[validate(custom = "validate_event_channel_capacity")]
    pub event_channel_capacity: usize,

    /// Webhook secret for verifying payment gateway callbacks
    #[serde(default)]
    pub payment_webhook_secret: Option<String>,

    /// Webhook timestamp tolerance (seconds)
    #[serde(default)]
    pub payment_webhook_tolerance_secs: Option<u64>,

    /// Agentic Commerce: OpenAI webhook URL for order events
    #[serde(default)]
    pub agentic_commerce_webhook_url: Option<String>,

    /// Agentic Commerce: Webhook secret for HMAC signatures
    #[serde(default)]
    pub agentic_commerce_webhook_secret: Option<String>,

    /// Agentic Commerce: Shared secret for verifying inbound ACP signatures
    #[serde(default)]
    pub agentic_commerce_signing_secret: Option<String>,

    /// Agentic Commerce: Signature timestamp tolerance in seconds (default 300s)
    #[serde(default)]
    pub agentic_commerce_signature_tolerance_secs: Option<u64>,

    // ========== OAuth2 Configuration ==========
    /// Enable OAuth2 authentication
    #[serde(default)]
    pub oauth2_enabled: bool,

    /// OAuth2 frontend redirect URL (where to redirect after auth)
    #[serde(default)]
    pub oauth2_frontend_url: Option<String>,

    /// Google OAuth2 Client ID
    #[serde(default)]
    pub oauth2_google_client_id: Option<String>,

    /// Google OAuth2 Client Secret
    #[serde(default)]
    pub oauth2_google_client_secret: Option<String>,

    /// Google OAuth2 Redirect URL
    #[serde(default)]
    pub oauth2_google_redirect_url: Option<String>,

    /// GitHub OAuth2 Client ID
    #[serde(default)]
    pub oauth2_github_client_id: Option<String>,

    /// GitHub OAuth2 Client Secret
    #[serde(default)]
    pub oauth2_github_client_secret: Option<String>,

    /// GitHub OAuth2 Redirect URL
    #[serde(default)]
    pub oauth2_github_redirect_url: Option<String>,

    /// Microsoft OAuth2 Client ID
    #[serde(default)]
    pub oauth2_microsoft_client_id: Option<String>,

    /// Microsoft OAuth2 Client Secret
    #[serde(default)]
    pub oauth2_microsoft_client_secret: Option<String>,

    /// Microsoft OAuth2 Redirect URL
    #[serde(default)]
    pub oauth2_microsoft_redirect_url: Option<String>,

    /// Microsoft OAuth2 Tenant ID (defaults to "common")
    #[serde(default)]
    pub oauth2_microsoft_tenant_id: Option<String>,

    // ========== API Pagination Configuration ==========
    /// Default page size for paginated API responses
    #[serde(default = "default_api_page_size")]
    pub api_default_page_size: u32,

    /// Maximum page size allowed for paginated API responses
    #[serde(default = "default_api_max_page_size")]
    pub api_max_page_size: u32,

    /// Default currency code for commerce operations
    #[serde(default = "default_currency")]
    pub default_currency: String,

    /// Maximum request body size in bytes (default 10MB)
    #[serde(default = "default_max_body_size")]
    pub max_body_size: usize,

    /// Maximum string length for input validation (default 10000)
    #[serde(default = "default_max_string_length")]
    pub max_string_length: usize,

    // ========== Circuit Breaker Configuration ==========
    /// Number of failures before circuit breaker opens
    #[serde(default = "default_circuit_breaker_failures")]
    pub circuit_breaker_failure_threshold: u32,

    /// Circuit breaker reset timeout in seconds
    #[serde(default = "default_circuit_breaker_timeout")]
    pub circuit_breaker_timeout_secs: u64,

    /// Circuit breaker backoff multiplier
    #[serde(default = "default_circuit_breaker_multiplier")]
    pub circuit_breaker_backoff_multiplier: f64,

    // ========== Auth Configuration ==========
    /// API key prefix (e.g., "sk_" for secret keys)
    #[serde(default = "default_api_key_prefix")]
    pub api_key_prefix: String,

    /// JWT issuer name
    #[serde(default = "default_auth_issuer")]
    pub auth_issuer: String,

    /// JWT audience/subject
    #[serde(default = "default_auth_audience")]
    pub auth_audience: String,
}

impl AppConfig {
    /// Gets database URL reference
    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    /// Creates a new configuration
    pub fn new(
        database_url: String,
        redis_url: String,
        jwt_secret: String,
        jwt_expiration: usize,
        refresh_token_expiration: usize,
        host: String,
        port: u16,
        environment: String,
    ) -> Self {
        let mut config = Self {
            database_url: database_url.clone(),
            db_url: database_url,
            redis_url: redis_url.clone(),
            jwt_secret,
            jwt_expiration,
            refresh_token_expiration,
            host,
            port,
            grpc_port: None,
            environment,
            log_level: default_log_level(),
            log_json: false,
            cache: CacheConfig {
                redis_url, // Use the same Redis URL for cache by default
                ..Default::default()
            },
            auto_migrate: false,
            cors_allowed_origins: None,
            cors_allow_any_origin: false,
            cors_allow_credentials: false,
            db_max_connections: default_db_max_connections(),
            db_min_connections: default_db_min_connections(),
            db_connect_timeout_secs: default_db_connect_timeout_secs(),
            db_idle_timeout_secs: default_db_idle_timeout_secs(),
            db_acquire_timeout_secs: default_db_acquire_timeout_secs(),
            db_statement_timeout_secs: None,
            rate_limit_requests_per_window: default_rate_limit_requests(),
            rate_limit_window_seconds: default_rate_limit_window_secs(),
            rate_limit_enable_headers: default_true_bool(),
            // Default: no additional policy overrides configured
            rate_limit_api_key_policies: None,
            rate_limit_user_policies: None,
            rate_limit_path_policies: None,
            rate_limit_use_redis: default_false_bool(),
            rate_limit_namespace: default_rate_limit_namespace(),
            message_queue_backend: default_message_queue_backend(),
            message_queue_namespace: default_message_queue_namespace(),
            message_queue_block_timeout_secs: default_message_queue_block_timeout_secs(),
            payment_provider: None,
            default_tax_rate: default_tax_rate(),
            event_channel_capacity: default_event_channel_capacity(),
            payment_webhook_secret: None,
            payment_webhook_tolerance_secs: None,
            agentic_commerce_webhook_url: None,
            agentic_commerce_webhook_secret: None,
            agentic_commerce_signing_secret: None,
            agentic_commerce_signature_tolerance_secs: None,
            // OAuth2 defaults
            oauth2_enabled: false,
            oauth2_frontend_url: None,
            oauth2_google_client_id: None,
            oauth2_google_client_secret: None,
            oauth2_google_redirect_url: None,
            oauth2_github_client_id: None,
            oauth2_github_client_secret: None,
            oauth2_github_redirect_url: None,
            oauth2_microsoft_client_id: None,
            oauth2_microsoft_client_secret: None,
            oauth2_microsoft_redirect_url: None,
            oauth2_microsoft_tenant_id: None,
            // API pagination defaults
            api_default_page_size: default_api_page_size(),
            api_max_page_size: default_api_max_page_size(),
            default_currency: default_currency(),
            max_body_size: default_max_body_size(),
            max_string_length: default_max_string_length(),
            // Circuit breaker defaults
            circuit_breaker_failure_threshold: default_circuit_breaker_failures(),
            circuit_breaker_timeout_secs: default_circuit_breaker_timeout(),
            circuit_breaker_backoff_multiplier: default_circuit_breaker_multiplier(),
            // Auth defaults
            api_key_prefix: default_api_key_prefix(),
            auth_issuer: default_auth_issuer(),
            auth_audience: default_auth_audience(),
        };
        config.db_url = config.database_url.clone();
        config
    }

    /// Gets Redis URL reference
    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }

    /// Checks if running in production environment
    pub fn is_production(&self) -> bool {
        self.environment.eq_ignore_ascii_case("production")
    }

    /// Checks if running in development environment
    pub fn is_development(&self) -> bool {
        self.environment.eq_ignore_ascii_case("development")
    }

    /// Returns true if explicit CORS origins are configured
    pub fn has_cors_allowed_origins(&self) -> bool {
        self.cors_allowed_origins
            .as_ref()
            .map(|raw| raw.split(',').any(|origin| !origin.trim().is_empty()))
            .unwrap_or(false)
    }

    /// Whether we should fall back to permissive CORS
    pub fn should_allow_permissive_cors(&self) -> bool {
        self.is_development() || self.cors_allow_any_origin
    }

    /// Build OAuth2 configuration from app config
    pub fn build_oauth2_config(&self) -> crate::auth::OAuth2Config {
        use crate::auth::{OAuth2Config, OAuth2ProviderConfig};

        let google = match (
            &self.oauth2_google_client_id,
            &self.oauth2_google_client_secret,
            &self.oauth2_google_redirect_url,
        ) {
            (Some(id), Some(secret), Some(redirect)) if !id.is_empty() => Some(
                OAuth2ProviderConfig::google(id.clone(), secret.clone(), redirect.clone()),
            ),
            _ => None,
        };

        let github = match (
            &self.oauth2_github_client_id,
            &self.oauth2_github_client_secret,
            &self.oauth2_github_redirect_url,
        ) {
            (Some(id), Some(secret), Some(redirect)) if !id.is_empty() => Some(
                OAuth2ProviderConfig::github(id.clone(), secret.clone(), redirect.clone()),
            ),
            _ => None,
        };

        let microsoft = match (
            &self.oauth2_microsoft_client_id,
            &self.oauth2_microsoft_client_secret,
            &self.oauth2_microsoft_redirect_url,
        ) {
            (Some(id), Some(secret), Some(redirect)) if !id.is_empty() => {
                Some(OAuth2ProviderConfig::microsoft(
                    id.clone(),
                    secret.clone(),
                    redirect.clone(),
                    self.oauth2_microsoft_tenant_id.clone(),
                ))
            }
            _ => None,
        };

        OAuth2Config {
            enabled: self.oauth2_enabled,
            google,
            github,
            microsoft,
            custom: None,
        }
    }

    /// Get OAuth2 frontend redirect URL
    pub fn oauth2_frontend_url(&self) -> String {
        self.oauth2_frontend_url
            .clone()
            .unwrap_or_else(|| format!("http://{}:{}/auth/callback", self.host, self.port))
    }

    fn validate_additional_constraints(&self) -> Result<(), ValidationErrors> {
        let mut errors = ValidationErrors::new();

        if !self.should_allow_permissive_cors() && !self.has_cors_allowed_origins() {
            let mut err = ValidationError::new("cors_allowed_origins_required");
            err.message = Some(
                "Set APP__CORS_ALLOWED_ORIGINS for non-development environments or explicitly opt-in via APP__CORS_ALLOW_ANY_ORIGIN=true".into(),
            );
            errors.add("cors_allowed_origins", err);
        }

        if !self.is_development() && self.jwt_secret.trim() == DEV_DEFAULT_JWT_SECRET {
            let mut err = ValidationError::new("jwt_secret_default_dev");
            err.message = Some(
                "The bundled development JWT secret must not be used outside development. Set APP__JWT_SECRET to a unique, secure value."
                    .into(),
            );
            errors.add("jwt_secret", err);
        }

        if errors.errors().is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Gets log level reference
    pub fn log_level(&self) -> &str {
        &self.log_level
    }

    /// Gets cache configuration reference
    pub fn cache(&self) -> &CacheConfig {
        &self.cache
    }

    /// Gets cache TTL in Duration
    pub fn cache_ttl(&self) -> Option<std::time::Duration> {
        self.cache
            .default_ttl_secs
            .map(std::time::Duration::from_secs)
    }
}

/// Configuration loading errors
#[derive(Debug, Error)]
pub enum AppConfigError {
    #[error("Configuration loading failed: {0}")]
    Load(#[from] ConfigError),

    #[error("Configuration validation failed: {0}")]
    Validation(#[from] validator::ValidationErrors),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Default value functions
fn default_log_level() -> String {
    DEFAULT_LOG_LEVEL.to_string()
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_cache_type() -> String {
    DEFAULT_CACHE_TYPE.to_string()
}

fn default_cache_capacity() -> usize {
    DEFAULT_CACHE_CAPACITY
}

fn default_cleanup_interval() -> u64 {
    DEFAULT_CLEANUP_INTERVAL
}

fn default_db_max_connections() -> u32 {
    16
}
fn default_db_min_connections() -> u32 {
    2
}
fn default_db_connect_timeout_secs() -> u64 {
    30
}
fn default_db_idle_timeout_secs() -> u64 {
    600
}
fn default_db_acquire_timeout_secs() -> u64 {
    8
}

fn default_rate_limit_requests() -> u32 {
    DEFAULT_RATE_LIMIT_REQUESTS
}
fn default_rate_limit_window_secs() -> u64 {
    DEFAULT_RATE_LIMIT_WINDOW_SECS
}
fn default_rate_limit_namespace() -> String {
    DEFAULT_RATE_LIMIT_NAMESPACE.to_string()
}
fn default_false_bool() -> bool {
    false
}
fn default_message_queue_backend() -> String {
    DEFAULT_MESSAGE_QUEUE_BACKEND.to_string()
}
fn default_message_queue_namespace() -> String {
    DEFAULT_MESSAGE_QUEUE_NAMESPACE.to_string()
}
fn default_message_queue_block_timeout_secs() -> u64 {
    DEFAULT_MESSAGE_QUEUE_BLOCK_TIMEOUT_SECS
}
fn default_true_bool() -> bool {
    true
}

fn default_tax_rate() -> f64 {
    0.08 // 8% default tax rate
}

fn default_event_channel_capacity() -> usize {
    1024 // Default channel capacity
}

fn default_api_page_size() -> u32 {
    20 // Default page size for API pagination
}

fn default_api_max_page_size() -> u32 {
    100 // Maximum page size allowed
}

fn default_currency() -> String {
    "USD".to_string() // Default currency
}

fn default_max_body_size() -> usize {
    10 * 1024 * 1024 // 10MB default max body size
}

fn default_max_string_length() -> usize {
    10000 // Default max string length for input validation
}

fn default_circuit_breaker_failures() -> u32 {
    5 // Number of failures before circuit opens
}

fn default_circuit_breaker_timeout() -> u64 {
    60 // 60 seconds reset timeout
}

fn default_circuit_breaker_multiplier() -> f64 {
    2.0 // Backoff multiplier
}

fn default_api_key_prefix() -> String {
    "sk_".to_string() // Default API key prefix
}

fn default_auth_issuer() -> String {
    "stateset-api".to_string() // Default JWT issuer
}

fn default_auth_audience() -> String {
    "stateset-auth".to_string() // Default JWT audience
}

fn validate_message_queue_backend(value: &str) -> Result<(), ValidationError> {
    match value.to_ascii_lowercase().as_str() {
        "in-memory" | "redis" => Ok(()),
        _ => {
            let mut err = ValidationError::new("message_queue_backend");
            err.message = Some("Must be one of: in-memory, redis".into());
            Err(err)
        }
    }
}

/// Validates log level values
fn validate_log_level(level: &str) -> Result<(), ValidationError> {
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if valid_levels.contains(&level.to_lowercase().as_str()) {
        Ok(())
    } else {
        let mut err = ValidationError::new("log_level");
        err.message = Some("Must be one of: trace, debug, info, warn, error".into());
        Err(err)
    }
}

fn validate_jwt_secret(secret: &str) -> Result<(), ValidationError> {
    let trimmed = secret.trim();

    // Enforce minimum length (should be 64+ for HS256)
    if trimmed.len() < 64 {
        let mut err = ValidationError::new("jwt_secret");
        err.message =
            Some("JWT secret must be at least 64 characters for adequate security".into());
        return Err(err);
    }

    // Reject known insecure defaults and obvious placeholders
    const DISALLOWED: [&str; 4] = [
        "CHANGE_THIS_SECRET_IN_PRODUCTION",
        "INSECURE_DEFAULT_DO_NOT_USE_IN_PRODUCTION",
        "your-secret-key",
        "default-secret-key",
    ];
    if DISALLOWED
        .iter()
        .any(|&bad| trimmed.eq_ignore_ascii_case(bad))
    {
        let mut err = ValidationError::new("jwt_secret");
        err.message = Some("JWT secret must be overridden with a secure random value".into());
        return Err(err);
    }

    // Reject trivially weak secrets (all identical characters or common patterns)
    if let Some(first) = trimmed.chars().next() {
        if trimmed.chars().all(|c| c == first) {
            let mut err = ValidationError::new("jwt_secret");
            err.message = Some("JWT secret cannot be a repeated character sequence".into());
            return Err(err);
        }
    }

    let lower = trimmed.to_ascii_lowercase();
    let weak_fragments = ["changeme", "password", "default", "12345", "abcdef"];
    if weak_fragments.iter().any(|pattern| lower.contains(pattern)) {
        let mut err = ValidationError::new("jwt_secret");
        err.message = Some(
            "JWT secret appears to be weak; use a cryptographically strong random string".into(),
        );
        return Err(err);
    }

    // Check for minimum character diversity (at least 4 unique characters)
    let unique_chars: std::collections::HashSet<char> = trimmed.chars().collect();
    if unique_chars.len() < 10 {
        let mut err = ValidationError::new("jwt_secret");
        err.message =
            Some("JWT secret must have at least 10 unique characters for adequate entropy".into());
        return Err(err);
    }

    Ok(())
}

fn validate_tax_rate(rate: &f64) -> Result<(), ValidationError> {
    if !rate.is_finite() || *rate < 0.0 || *rate > 1.0 {
        let mut err = ValidationError::new("default_tax_rate");
        err.message = Some("default_tax_rate must be a finite value between 0.0 and 1.0".into());
        return Err(err);
    }
    Ok(())
}

fn validate_event_channel_capacity(capacity: &usize) -> Result<(), ValidationError> {
    if *capacity == 0 {
        let mut err = ValidationError::new("event_channel_capacity");
        err.message = Some("event_channel_capacity must be greater than 0".into());
        return Err(err);
    }
    Ok(())
}

/// Initializes tracing using the provided log level as the default filter
pub fn init_tracing(level: &str, json: bool) {
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let default_directive = format!("stateset_api={},tower_http=debug", level);
    let filter_directive = std_env::var("RUST_LOG")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default_directive.clone());

    // Optional OpenTelemetry initialization via env (APP__OTEL_ENABLED or OTEL_EXPORTER_OTLP_ENDPOINT)
    let otel_enabled = std_env::var("APP__OTEL_ENABLED")
        .map(|v| v == "1" || v.to_lowercase() == "true")
        .unwrap_or(false)
        || std_env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok();

    if otel_enabled {
        #[allow(unused_imports)]
        use opentelemetry::{global, KeyValue};
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::{trace as sdktrace, Resource};

        let endpoint = std_env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:4317".to_string());
        let service_name =
            std_env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "stateset-api".to_string());

        let resource = Resource::new(vec![KeyValue::new("service.name", service_name.clone())]);
        let tracer = match opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint),
            )
            .with_trace_config(sdktrace::config().with_resource(resource))
            .install_batch(opentelemetry_sdk::runtime::Tokio)
        {
            Ok(tracer) => tracer,
            Err(err) => {
                error!("Failed to install OTLP pipeline: {}", err);
                if json {
                    let _ = fmt().with_env_filter(filter_directive).json().try_init();
                } else {
                    let _ = fmt().with_env_filter(filter_directive).try_init();
                }
                return;
            }
        };

        let base = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .with(EnvFilter::new(filter_directive.clone()));

        if json {
            let _ = base.with(fmt::layer().json()).try_init();
        } else {
            let _ = base.with(fmt::layer()).try_init();
        }
    } else {
        if json {
            let _ = fmt().with_env_filter(filter_directive).json().try_init();
        } else {
            let _ = fmt().with_env_filter(filter_directive).try_init();
        }
    }
}

/// Loads application configuration
///
/// Layers configuration sources in this order:
/// 1. Default config (config/default.toml)
/// 2. Environment-specific config (config/{env}.toml)
/// 3. Docker config (config/docker.toml) if DOCKER env var is set
/// 4. Environment variables (APP_*)
pub fn load_config() -> Result<AppConfig, AppConfigError> {
    // Support both RUN_ENV and APP_ENV for selecting config profile
    let run_env = env::var("RUN_ENV")
        .or_else(|_| env::var("APP_ENV"))
        .unwrap_or_else(|_| DEFAULT_ENV.to_string());
    info!("Loading configuration for environment: {}", run_env);

    if !Path::new(CONFIG_DIR).exists() {
        info!(
            "Config directory '{}' not found; relying on built-in defaults and environment variables",
            CONFIG_DIR
        );
    }

    // NOTE: jwt_secret has no default - it MUST be provided via environment variable
    // or config file. This prevents accidental use of insecure defaults in production.
    let mut builder = Config::builder()
        .set_default("database_url", "sqlite://stateset.db?mode=rwc")?
        .set_default("redis_url", "redis://localhost:6379")?
        .set_default("jwt_expiration", 3600)?
        .set_default("refresh_token_expiration", 604800)?
        .set_default("host", "0.0.0.0")?
        .set_default("port", 8080)?
        .set_default("environment", DEFAULT_ENV)?
        .set_default("log_level", DEFAULT_LOG_LEVEL)?
        .set_default("log_json", false)?
        .add_source(File::with_name(&format!("{}/default", CONFIG_DIR)).required(false))
        .add_source(File::with_name(&format!("{}/{}", CONFIG_DIR, run_env)).required(false));

    if env::var("DOCKER").is_ok() {
        info!("Docker environment detected");
        builder =
            builder.add_source(File::with_name(&format!("{}/docker", CONFIG_DIR)).required(false));
    }

    let config = builder
        .add_source(Environment::with_prefix("APP").separator("__"))
        .build()?;

    // Check for jwt_secret before deserialization to provide a clear error message
    if config.get_string("jwt_secret").is_err() {
        error!("JWT secret is not configured. Set APP__JWT_SECRET environment variable with a secure random string (minimum 64 characters).");
        error!("Generate a secure secret with: openssl rand -base64 64");
        return Err(AppConfigError::Load(ConfigError::NotFound(
            "jwt_secret is required but not configured. Set APP__JWT_SECRET environment variable."
                .into(),
        )));
    }

    let app_config: AppConfig = config.try_deserialize()?;

    app_config.validate().map_err(|e| {
        error!("Configuration validation failed: {:?}", e);
        AppConfigError::Validation(e)
    })?;

    app_config.validate_additional_constraints().map_err(|e| {
        error!("Configuration security validation failed: {:?}", e);
        AppConfigError::Validation(e)
    })?;

    info!("Configuration loaded successfully");
    Ok(app_config)
}

#[cfg(test)]
mod cors_validation_tests {
    use super::*;

    fn base_config() -> AppConfig {
        AppConfig::new(
            "sqlite://stateset.db?mode=memory".into(),
            "redis://127.0.0.1:6379".into(),
            "super_secure_jwt_secret_that_is_long_enough_123".into(),
            3600,
            86_400,
            "127.0.0.1".into(),
            8080,
            "production".into(),
        )
    }

    #[test]
    fn non_dev_requires_cors_origins() {
        let cfg = base_config();
        assert!(cfg.validate_additional_constraints().is_err());
    }

    #[test]
    fn non_dev_allows_override_flag() {
        let mut cfg = base_config();
        cfg.cors_allow_any_origin = true;
        assert!(cfg.validate_additional_constraints().is_ok());
    }

    #[test]
    fn non_dev_with_origins_passes() {
        let mut cfg = base_config();
        cfg.cors_allowed_origins = Some("https://example.com".into());
        assert!(cfg.validate_additional_constraints().is_ok());
    }

    #[test]
    fn development_allows_permissive_by_default() {
        let mut cfg = base_config();
        cfg.environment = "development".into();
        assert!(cfg.validate_additional_constraints().is_ok());
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_test_config(content: &str, filename: &str) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join(CONFIG_DIR);
        std::fs::create_dir(&config_path).unwrap();

        let file_path = config_path.join(filename);
        let mut file = File::create(file_path).unwrap();
        writeln!(file, "{}", content).unwrap();

        env::set_var("CARGO_MANIFEST_DIR", temp_dir.path().to_str().unwrap());
        temp_dir
    }

    #[test]
    fn test_load_config_success() {
        let default_content = r#"
            database_url = "postgres://localhost/default"
            redis_url = "redis://localhost"
            jwt_secret = "verysecuresecretthatislongenough"
            jwt_expiration = 3600
            refresh_token_expiration = 86400
            host = "http://localhost"
            port = 8080
            environment = "development"
            log_level = "info"
        "#;

        let _temp_dir = setup_test_config(default_content, "default.toml");

        env::set_var("APP__DATABASE_URL", "postgres://localhost/override");
        env::set_var("RUN_ENV", "development");

        let config = load_config().unwrap();

        assert_eq!(config.database_url, "postgres://localhost/override");
        assert_eq!(config.redis_url, "redis://localhost");
        assert_eq!(config.environment, "development");
    }

    #[test]
    fn test_validation_failure() {
        let invalid_content = r#"
            database_url = "invalid"
            redis_url = "redis://localhost"
            jwt_secret = "short"
            jwt_expiration = 100
            refresh_token_expiration = 1000
            host = "invalid"
            port = 80
            environment = ""
            log_level = "invalid"
        "#;

        let _temp_dir = setup_test_config(invalid_content, "default.toml");
        env::set_var("RUN_ENV", "development");

        let result = load_config();
        assert!(matches!(result, Err(AppConfigError::Validation(_))));

        if let Err(AppConfigError::Validation(errors)) = result {
            assert!(errors.field_errors().contains_key("database_url"));
            assert!(errors.field_errors().contains_key("jwt_secret"));
            assert!(errors.field_errors().contains_key("jwt_expiration"));
            assert!(errors.field_errors().contains_key("environment"));
            assert!(errors.field_errors().contains_key("log_level"));
        }
    }

    #[test]
    fn test_docker_config() {
        let default_content = r#"
            database_url = "postgres://localhost/default"
            redis_url = "redis://localhost"
            jwt_secret = "verysecuresecretthatislongenough"
            jwt_expiration = 3600
            refresh_token_expiration = 86400
            host = "http://localhost"
            port = 8080
            environment = "production"
        "#;

        let docker_content = r#"
            database_url = "postgres://docker/db"
            host = "http://docker.local"
            port = 8081
        "#;

        let temp_dir = setup_test_config(default_content, "default.toml");
        let config_path = temp_dir.path().join(CONFIG_DIR);
        let mut docker_file = File::create(config_path.join("docker.toml")).unwrap();
        writeln!(docker_file, "{}", docker_content).unwrap();

        env::set_var("DOCKER", "1");
        env::set_var("RUN_ENV", "production");

        let config = load_config().unwrap();

        assert_eq!(config.database_url, "postgres://docker/db");
        assert_eq!(config.host, "http://docker.local");
        assert_eq!(config.port, 8081);
    }
}
