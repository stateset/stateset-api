use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;
use std::path::Path;
use thiserror::Error;
use tracing::{error, info};
use validator::{Validate, ValidationError};

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

/// Cache configuration
#[derive(Clone, Debug, Deserialize, Validate)]
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
pub struct AppConfig {
    /// Database connection URL
    pub database_url: String,

    /// Database URL alias (for backward compatibility)
    #[serde(skip)]
    pub db_url: String,

    /// Redis connection URL
    pub redis_url: String,

    /// JWT secret key (minimum 32 characters)
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

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,

    /// Whether to run database migrations on startup
    #[serde(default)]
    pub auto_migrate: bool,

    /// CORS: comma-separated list of allowed origins (production)
    #[serde(default)]
    pub cors_allowed_origins: Option<String>,

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
            cache: CacheConfig {
                redis_url, // Use the same Redis URL for cache by default
                ..Default::default()
            },
            auto_migrate: false,
            cors_allowed_origins: None,
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

fn default_db_max_connections() -> u32 { 16 }
fn default_db_min_connections() -> u32 { 2 }
fn default_db_connect_timeout_secs() -> u64 { 30 }
fn default_db_idle_timeout_secs() -> u64 { 600 }
fn default_db_acquire_timeout_secs() -> u64 { 8 }

fn default_rate_limit_requests() -> u32 { DEFAULT_RATE_LIMIT_REQUESTS }
fn default_rate_limit_window_secs() -> u64 { DEFAULT_RATE_LIMIT_WINDOW_SECS }
fn default_true_bool() -> bool { true }

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

/// Initializes tracing using the provided log level as the default filter
pub fn init_tracing(level: &str) {
    use tracing_subscriber::{fmt, EnvFilter};

    let default_directive = format!("stateset_api={},tower_http=debug", level);
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_directive));

    let _ = fmt().with_env_filter(filter).try_init();
}

/// Loads application configuration
///
/// Layers configuration sources in this order:
/// 1. Default config (config/default.toml)
/// 2. Environment-specific config (config/{env}.toml)
/// 3. Docker config (config/docker.toml) if DOCKER env var is set
/// 4. Environment variables (APP_*)
pub fn load_config() -> Result<AppConfig, AppConfigError> {
    let run_env = env::var("RUN_ENV").unwrap_or_else(|_| DEFAULT_ENV.to_string());
    info!("Loading configuration for environment: {}", run_env);

    // Ensure config directory exists
    if !Path::new(CONFIG_DIR).exists() {
        std::fs::create_dir_all(CONFIG_DIR)?;
    }

    let mut builder = Config::builder()
        .add_source(File::with_name(&format!("{}/default", CONFIG_DIR)).required(true))
        .add_source(File::with_name(&format!("{}/{}", CONFIG_DIR, run_env)).required(false));

    if env::var("DOCKER").is_ok() {
        info!("Docker environment detected");
        builder =
            builder.add_source(File::with_name(&format!("{}/docker", CONFIG_DIR)).required(false));
    }

    let config = builder
        .add_source(Environment::with_prefix("APP").separator("__"))
        .build()?;

    let app_config: AppConfig = config.try_deserialize()?;

    app_config.validate().map_err(|e| {
        error!("Configuration validation failed: {:?}", e);
        AppConfigError::Validation(e)
    })?;

    info!("Configuration loaded successfully");
    Ok(app_config)
}

#[cfg(test)]
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
