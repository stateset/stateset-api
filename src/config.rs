use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::sync::Arc;
use config::{Config, ConfigError, Environment, File};
use validator::{Validate, ValidationError};
use thiserror::Error;
use tracing::{error, info};

/// Default log level if not specified in configuration.
fn default_log_level() -> String {
    "info".to_string()
}

/// Application configuration structure.
#[derive(Clone, Debug, Deserialize, Validate)]
pub struct AppConfig {
    /// Database connection URL.
    #[validate(url)]
    pub database_url: String,

    /// Redis connection URL.
    #[validate(url)]
    pub redis_url: String,

    /// Secret key for signing JWT tokens.
    #[validate(length(min = 32))]
    pub jwt_secret: String,

    /// JWT token expiration time in seconds (5 minutes to 24 hours).
    #[validate(range(min = 300, max = 86400))]
    pub jwt_expiration: usize,

    /// Refresh token expiration time in seconds (1 day to 30 days).
    #[validate(range(min = 86400, max = 2592000))]
    pub refresh_token_expiration: usize,

    /// Server host address.
    #[validate(url)]
    pub host: String,

    /// Server port (1024 to 65535).
    #[validate(range(min = 1024, max = 65535))]
    pub port: u16,

    /// Application environment (e.g., development, production).
    #[validate(length(min = 1))]
    pub environment: String,

    /// Logging level (default: "info").
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl AppConfig {
    /// Returns the database URL.
    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    /// Returns the Redis URL.
    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }

    /// Checks if the environment is production.
    pub fn is_production(&self) -> bool {
        self.environment.eq_ignore_ascii_case("production")
    }

    /// Returns the log level.
    pub fn log_level(&self) -> &str {
        &self.log_level
    }
}

/// Custom error type for configuration-related errors.
#[derive(Debug, Error)]
pub enum AppConfigError {
    #[error("Failed to load configuration: {0}")]
    LoadError(#[from] config::ConfigError),

    #[error("Invalid configuration: {0}")]
    ValidationError(#[from] validator::ValidationErrors),
}

/// Loads the application configuration.
///
/// The function performs the following steps:
/// 1. Determines the current environment (`RUN_ENV`), defaulting to "development" if not set.
/// 2. Builds a configuration by layering:
///    - Default configuration from `config/default.toml`.
///    - Environment-specific configuration from `config/{env}.toml` (optional).
///    - Docker-specific configuration from `config/docker.toml` if the `DOCKER` environment variable is set.
///    - Environment variables prefixed with `APP` (overrides previous configurations).
/// 3. Deserializes the configuration into `AppConfig`.
/// 4. Validates the configuration fields.
///
pub fn load_config() -> Result<AppConfig, AppConfigError> {
    // Initialize tracing for configuration loading
    tracing_subscriber::fmt::init();

    // Determine the current environment, defaulting to "development"
    let run_env = env::var("RUN_ENV").unwrap_or_else(|_| "development".to_string());
    info!("Loading configuration for environment: {}", run_env);

    // Initialize the configuration builder
    let mut builder = Config::builder()
        // Load default configuration
        .add_source(File::with_name("config/default").required(true))
        // Load environment-specific configuration (optional)
        .add_source(File::with_name(&format!("config/{}", run_env)).required(false))
        // If running in Docker, load Docker-specific configuration
        .add_source(Environment::with_prefix("APP"));

    // Check if running in Docker and load Docker-specific config if so
    if env::var("DOCKER").is_ok() {
        info!("Docker environment detected. Loading Docker-specific configuration.");
        builder = builder.add_source(File::with_name("config/docker").required(false));
    }

    // Build the configuration
    let config = builder.build()?;

    // Deserialize into AppConfig
    let app_config: AppConfig = config.try_deserialize()?;

    // Validate the configuration
    app_config.validate()?;

    info!("Configuration loaded successfully.");
    Ok(app_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::NamedTempFile;
    use std::io::Write;
    use validator::Validate;

    /// Helper function to create a temporary configuration file.
    fn create_temp_config(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().expect("Failed to create temp file");
        write!(file, "{}", content).expect("Failed to write to temp file");
        file
    }

    #[test]
    fn test_load_config_success() {
        // Create a default configuration file
        let default_config = create_temp_config(r#"
            database_url = "postgres://user:pass@localhost/defaultdb"
            redis_url = "redis://localhost:6379"
            jwt_secret = "a_very_secure_jwt_secret_key_with_minimum_length"
            jwt_expiration = 3600
            refresh_token_expiration = 86400
            host = "http://localhost"
            port = 8080
            environment = "development"
            log_level = "debug"
        "#);

        // Create an environment-specific configuration file (optional)
        let env_config = create_temp_config(r#"
            database_url = "postgres://user:pass@localhost/envdb"
            environment = "staging"
        "#);

        // Set environment variables to point to the temp config files
        env::set_var("RUN_ENV", "staging");
        env::set_var("APP_DATABASE_URL", "postgres://user:pass@localhost/env_override_db");
        env::set_var("APP_JWT_SECRET", "another_secure_jwt_secret_key_with_minimum_length");
        env::set_var("APP_JWT_EXPIRATION", "7200");
        env::set_var("APP_REFRESH_TOKEN_EXPIRATION", "172800");
        env::set_var("APP_HOST", "http://staging.local");
        env::set_var("APP_PORT", "9090");
        env::set_var("APP_ENVIRONMENT", "staging_override");
        env::set_var("APP_LOG_LEVEL", "info");

        // Mock file paths by temporarily renaming the temp files
        std::fs::rename(default_config.path(), "config/default.toml").expect("Failed to rename default config");
        std::fs::rename(env_config.path(), "config/staging.toml").expect("Failed to rename env config");

        // Load the configuration
        let config = load_config().expect("Failed to load config");

        // Assertions
        assert_eq!(config.database_url, "postgres://user:pass@localhost/env_override_db"); // Overridden by env variable
        assert_eq!(config.redis_url, "redis://localhost:6379"); // From default
        assert_eq!(config.jwt_secret, "another_secure_jwt_secret_key_with_minimum_length"); // Overridden by env variable
        assert_eq!(config.jwt_expiration, 7200); // Overridden by env variable
        assert_eq!(config.refresh_token_expiration, 172800); // Overridden by env variable
        assert_eq!(config.host, "http://staging.local"); // Overridden by env variable
        assert_eq!(config.port, 9090); // Overridden by env variable
        assert_eq!(config.environment, "staging_override"); // Overridden by env variable
        assert_eq!(config.log_level, "info"); // Overridden by env variable

        // Clean up: Remove the temporary config files
        std::fs::remove_file("config/default.toml").expect("Failed to remove default config");
        std::fs::remove_file("config/staging.toml").expect("Failed to remove env config");

        // Clear environment variables
        env::remove_var("RUN_ENV");
        env::remove_var("APP_DATABASE_URL");
        env::remove_var("APP_JWT_SECRET");
        env::remove_var("APP_JWT_EXPIRATION");
        env::remove_var("APP_REFRESH_TOKEN_EXPIRATION");
        env::remove_var("APP_HOST");
        env::remove_var("APP_PORT");
        env::remove_var("APP_ENVIRONMENT");
        env::remove_var("APP_LOG_LEVEL");
    }

    #[test]
    fn test_load_config_validation_failure() {
        // Create a default configuration file with invalid fields
        let default_config = create_temp_config(r#"
            database_url = "invalid_url"
            redis_url = "redis://localhost:6379"
            jwt_secret = "short"
            jwt_expiration = 100   # Below the minimum (300)
            refresh_token_expiration = 1000  # Below the minimum (86400)
            host = "localhost"  # Invalid URL format
            port = 80  # Below the allowed range (1024)
            environment = ""  # Empty environment
            log_level = "info"
        "#);

        // Set environment variables
        env::set_var("RUN_ENV", "development");

        // Mock file paths by temporarily renaming the temp file
        std::fs::rename(default_config.path(), "config/default.toml").expect("Failed to rename default config");

        // Attempt to load the configuration, expecting a validation error
        let result = load_config();
        assert!(result.is_err());

        if let Err(AppConfigError::ValidationError(e)) = result {
            // Check that the appropriate validation errors are present
            assert!(e.field_errors().contains_key("database_url"));
            assert!(e.field_errors().contains_key("jwt_secret"));
            assert!(e.field_errors().contains_key("jwt_expiration"));
            assert!(e.field_errors().contains_key("refresh_token_expiration"));
            assert!(e.field_errors().contains_key("host"));
            assert!(e.field_errors().contains_key("port"));
            assert!(e.field_errors().contains_key("environment"));
        } else {
            panic!("Expected ValidationError, got {:?}", result);
        }

        // Clean up: Remove the temporary config file
        std::fs::remove_file("config/default.toml").expect("Failed to remove default config");

        // Clear environment variables
        env::remove_var("RUN_ENV");
    }

    #[test]
    fn test_load_config_docker_environment() {
        // Create Docker-specific configuration file
        let docker_config = create_temp_config(r#"
            database_url = "postgres://user:pass@localhost/dockerdb"
            host = "http://docker.local"
            port = 8081
        "#);

        // Create a default configuration file
        let default_config = create_temp_config(r#"
            database_url = "postgres://user:pass@localhost/defaultdb"
            redis_url = "redis://localhost:6379"
            jwt_secret = "a_very_secure_jwt_secret_key_with_minimum_length"
            jwt_expiration = 3600
            refresh_token_expiration = 86400
            host = "http://localhost"
            port = 8080
            environment = "production"
            log_level = "error"
        "#);

        // Set environment variables
        env::set_var("RUN_ENV", "production");
        env::set_var("DOCKER", "1");

        // Mock file paths by temporarily renaming the temp files
        std::fs::rename(default_config.path(), "config/default.toml").expect("Failed to rename default config");
        std::fs::rename(docker_config.path(), "config/docker.toml").expect("Failed to rename docker config");

        // Load the configuration
        let config = load_config().expect("Failed to load config");

        // Assertions
        assert_eq!(config.database_url, "postgres://user:pass@localhost/dockerdb"); // Overridden by Docker config
        assert_eq!(config.redis_url, "redis://localhost:6379"); // From default
        assert_eq!(config.jwt_secret, "a_very_secure_jwt_secret_key_with_minimum_length"); // From default
        assert_eq!(config.jwt_expiration, 3600); // From default
        assert_eq!(config.refresh_token_expiration, 86400); // From default
        assert_eq!(config.host, "http://docker.local"); // Overridden by Docker config
        assert_eq!(config.port, 8081); // Overridden by Docker config
        assert_eq!(config.environment, "production"); // From default
        assert_eq!(config.log_level, "error"); // From default

        // Clean up: Remove the temporary config files
        std::fs::remove_file("config/default.toml").expect("Failed to remove default config");
        std::fs::remove_file("config/docker.toml").expect("Failed to remove docker config");

        // Clear environment variables
        env::remove_var("RUN_ENV");
        env::remove_var("DOCKER");
    }
}
