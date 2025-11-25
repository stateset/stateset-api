use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub webhook_url: Option<String>,
    pub openai_api_key: Option<String>,
    pub qdrant_url: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Load from environment or use defaults
        Ok(Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()?,
            log_level: std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            webhook_url: std::env::var("WEBHOOK_URL").ok(),
            openai_api_key: std::env::var("OPENAI_API_KEY").ok(),
            qdrant_url: std::env::var("QDRANT_URL").ok(),
        })
    }
}
