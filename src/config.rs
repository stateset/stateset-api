use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct AppConfig {
    pub database_url: String,
    pub redis_url: String,
    pub jwt_secret: String,
    pub jwt_expiration: i64,
    pub refresh_token_expiration: i64,
    pub host: String,
    pub port: u16,
}

pub fn load_config() -> AppConfig {
    let mut settings = config::Config::default();
    settings
        .merge(config::File::with_name("config/default"))
        .unwrap()
        .merge(config::Environment::with_prefix("APP"))
        .unwrap();

    settings.try_into().unwrap()
}