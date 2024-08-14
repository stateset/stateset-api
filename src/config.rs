use serde::Deserialize;
use std::env;
use config::{Config, ConfigError, Environment, File};
use validator::{Validate, ValidationError};

#[derive(Clone, Debug, Deserialize, Validate)]
pub struct AppConfig {
    #[validate(url)]
    pub database_url: String,
    #[validate(url)]
    pub redis_url: String,
    #[validate(length(min = 32))]
    pub jwt_secret: String,
    #[validate(range(min = 300, max = 86400))]  // 5 minutes to 24 hours
    pub jwt_expiration: i64,
    #[validate(range(min = 86400, max = 2592000))]  // 1 day to 30 days
    pub refresh_token_expiration: i64,
    pub host: String,
    #[validate(range(min = 1024, max = 65535))]
    pub port: u16,
    #[validate(length(min = 1))]
    pub environment: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl AppConfig {
    pub fn database_url(&self) -> &str {
        &self.database_url
    }

    pub fn redis_url(&self) -> &str {
        &self.redis_url
    }

    pub fn is_production(&self) -> bool {
        self.environment == "production"
    }

    pub fn log_level(&self) -> &str {
        &self.log_level
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to load configuration: {0}")]
    LoadError(#[from] config::ConfigError),
    #[error("Invalid configuration: {0}")]
    ValidationError(#[from] validator::ValidationErrors),
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let env = env::var("RUN_ENV").unwrap_or_else(|_| "development".into());

    let mut builder = Config::builder()
        .add_source(File::with_name("config/default"))
        .add_source(File::with_name(&format!("config/{}", env)).required(false))
        .add_source(Environment::with_prefix("APP"));

    // If we're in a Docker environment, we might want to use a different config file
    if env::var("DOCKER").is_ok() {
        builder = builder.add_source(File::with_name("config/docker").required(false));
    }

    let config: AppConfig = builder.build()?.try_deserialize()?;

    config.validate()?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        // Set some environment variables for testing
        env::set_var("APP_DATABASE_URL", "postgres://user:pass@localhost/testdb");
        env::set_var("APP_REDIS_URL", "redis://localhost:6379");
        env::set_var("APP_JWT_SECRET", "testsecrettestsecrettestsecrettestsecret");
        env::set_var("APP_JWT_EXPIRATION", "3600");
        env::set_var("APP_REFRESH_TOKEN_EXPIRATION", "86400");
        env::set_var("APP_HOST", "127.0.0.1");
        env::set_var("APP_PORT", "8080");
        env::set_var("APP_ENVIRONMENT", "test");

        let config = load_config().expect("Failed to load config");

        assert_eq!(config.database_url, "postgres://user:pass@localhost/testdb");
        assert_eq!(config.redis_url, "redis://localhost:6379");
        assert_eq!(config.jwt_secret, "testsecrettestsecrettestsecrettestsecret");
        assert_eq!(config.jwt_expiration, 3600);
        assert_eq!(config.refresh_token_expiration, 86400);
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.environment, "test");
        assert_eq!(config.log_level, "info");  // default value
    }

    #[test]
    fn test_invalid_config() {
        env::set_var("APP_JWT_SECRET", "tooshort");
        env::set_var("APP_PORT", "80");  // below allowed range

        let result = load_config();
        assert!(result.is_err());
    }
}