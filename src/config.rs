use serde::Deserialize;
use config::{Config, ConfigError, Environment, File};
use validator::{Validate, ValidationError};
use thiserror::Error;
use tracing::{error, info};
use std::env;
use std::path::Path;

/// Default values for configuration
const DEFAULT_LOG_LEVEL: &str = "info";
const DEFAULT_ENV: &str = "development";
const DEFAULT_PORT: u16 = 8080;
const CONFIG_DIR: &str = "config";

/// Application configuration structure with validation
#[derive(Clone, Debug, Deserialize, Validate)]
pub struct AppConfig {
    /// Database connection URL
    #[validate(url(message = "Must be a valid URL"))]
    pub database_url: String,
    
    /// Database URL alias (for backward compatibility)
    #[serde(skip)]
    pub db_url: String,

    /// Redis connection URL
    #[validate(url(message = "Must be a valid URL"))]
    pub redis_url: String,

    /// JWT secret key (minimum 32 characters)
    #[validate(length(min = 32, message = "Must be at least 32 characters"))]
    pub jwt_secret: String,

    /// JWT expiration time in seconds (5min - 24h)
    #[validate(range(min = 300, max = 86400))]
    pub jwt_expiration: usize,

    /// Refresh token expiration (1d - 30d)
    #[validate(range(min = 86400, max = 2592000))]
    pub refresh_token_expiration: usize,

    /// Server host address
    #[validate(url(message = "Must be a valid URL"))]
    pub host: String,

    /// Server port (1024-65535)
    #[serde(default = "default_port")]
    #[validate(range(min = 1024, max = 65535))]
    pub port: u16,

    /// Application environment
    #[validate(length(min = 1, message = "Cannot be empty"))]
    pub environment: String,

    /// Logging level
    #[serde(default = "default_log_level")]
    #[validate]
    pub log_level: String,
}

impl AppConfig {
    /// Gets database URL reference
    pub fn database_url(&self) -> &str {
        &self.database_url
    }
    
    /// Creates a new configuration
    pub fn new(database_url: String, redis_url: String, jwt_secret: String, 
               jwt_expiration: usize, refresh_token_expiration: usize, 
               host: String, port: u16, environment: String) -> Self {
        let mut config = Self {
            database_url: database_url.clone(),
            db_url: database_url,
            redis_url,
            jwt_secret,
            jwt_expiration,
            refresh_token_expiration,
            host,
            port,
            environment,
            log_level: default_log_level(),
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
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(default_directive));

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
        builder = builder.add_source(File::with_name(&format!("{}/docker", CONFIG_DIR)).required(false));
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
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;

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