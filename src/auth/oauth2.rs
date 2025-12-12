/*!
 * # OAuth2 Authentication Module
 *
 * This module provides OAuth2 authentication support for the StateSet API.
 * It supports multiple OAuth2 providers:
 *
 * - Google
 * - GitHub
 * - Microsoft (Azure AD)
 * - Custom/Generic OAuth2 providers
 *
 * ## Flow
 *
 * 1. Client initiates OAuth2 flow via `/auth/oauth2/{provider}/authorize`
 * 2. User is redirected to provider's authorization page
 * 3. Provider redirects back to `/auth/oauth2/{provider}/callback` with code
 * 4. Server exchanges code for tokens and creates/links user account
 * 5. Server issues JWT tokens to the client
 */

use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, AuthorizationCode, ClientId,
    ClientSecret, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, RevocationUrl,
    Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// OAuth2 error types
#[derive(Error, Debug)]
pub enum OAuth2Error {
    #[error("Provider not configured: {0}")]
    ProviderNotConfigured(String),

    #[error("Invalid state parameter")]
    InvalidState,

    #[error("Token exchange failed: {0}")]
    TokenExchangeFailed(String),

    #[error("Failed to fetch user info: {0}")]
    UserInfoFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("PKCE verifier not found for state")]
    PkceVerifierNotFound,

    #[error("Invalid callback: {0}")]
    InvalidCallback(String),

    #[error("Provider error: {0}")]
    ProviderError(String),
}

/// Supported OAuth2 providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuth2Provider {
    Google,
    GitHub,
    Microsoft,
    Custom,
}

impl std::fmt::Display for OAuth2Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OAuth2Provider::Google => write!(f, "google"),
            OAuth2Provider::GitHub => write!(f, "github"),
            OAuth2Provider::Microsoft => write!(f, "microsoft"),
            OAuth2Provider::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for OAuth2Provider {
    type Err = OAuth2Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "google" => Ok(OAuth2Provider::Google),
            "github" => Ok(OAuth2Provider::GitHub),
            "microsoft" => Ok(OAuth2Provider::Microsoft),
            "custom" => Ok(OAuth2Provider::Custom),
            _ => Err(OAuth2Error::ProviderNotConfigured(s.to_string())),
        }
    }
}

/// Configuration for a single OAuth2 provider
#[derive(Debug, Clone, Deserialize)]
pub struct OAuth2ProviderConfig {
    /// Client ID from the OAuth2 provider
    pub client_id: String,

    /// Client secret from the OAuth2 provider
    pub client_secret: String,

    /// Authorization endpoint URL
    pub auth_url: String,

    /// Token endpoint URL
    pub token_url: String,

    /// User info endpoint URL (for fetching user profile)
    pub user_info_url: String,

    /// Redirect URL (callback URL)
    pub redirect_url: String,

    /// OAuth2 scopes to request
    #[serde(default)]
    pub scopes: Vec<String>,

    /// Optional revocation URL
    pub revocation_url: Option<String>,
}

impl OAuth2ProviderConfig {
    /// Create a Google OAuth2 configuration
    pub fn google(client_id: String, client_secret: String, redirect_url: String) -> Self {
        Self {
            client_id,
            client_secret,
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            user_info_url: "https://www.googleapis.com/oauth2/v3/userinfo".to_string(),
            redirect_url,
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
            ],
            revocation_url: Some("https://oauth2.googleapis.com/revoke".to_string()),
        }
    }

    /// Create a GitHub OAuth2 configuration
    pub fn github(client_id: String, client_secret: String, redirect_url: String) -> Self {
        Self {
            client_id,
            client_secret,
            auth_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
            user_info_url: "https://api.github.com/user".to_string(),
            redirect_url,
            scopes: vec!["user:email".to_string(), "read:user".to_string()],
            revocation_url: None,
        }
    }

    /// Create a Microsoft (Azure AD) OAuth2 configuration
    pub fn microsoft(
        client_id: String,
        client_secret: String,
        redirect_url: String,
        tenant_id: Option<String>,
    ) -> Self {
        let tenant = tenant_id.unwrap_or_else(|| "common".to_string());
        Self {
            client_id,
            client_secret,
            auth_url: format!(
                "https://login.microsoftonline.com/{}/oauth2/v2.0/authorize",
                tenant
            ),
            token_url: format!(
                "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
                tenant
            ),
            user_info_url: "https://graph.microsoft.com/v1.0/me".to_string(),
            redirect_url,
            scopes: vec![
                "openid".to_string(),
                "email".to_string(),
                "profile".to_string(),
                "User.Read".to_string(),
            ],
            revocation_url: None,
        }
    }
}

/// Overall OAuth2 configuration
#[derive(Debug, Clone, Default, Deserialize)]
pub struct OAuth2Config {
    /// Whether OAuth2 is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Google OAuth2 configuration
    pub google: Option<OAuth2ProviderConfig>,

    /// GitHub OAuth2 configuration
    pub github: Option<OAuth2ProviderConfig>,

    /// Microsoft OAuth2 configuration
    pub microsoft: Option<OAuth2ProviderConfig>,

    /// Custom OAuth2 provider configuration
    pub custom: Option<OAuth2ProviderConfig>,
}

impl OAuth2Config {
    /// Check if a provider is configured
    pub fn is_provider_configured(&self, provider: OAuth2Provider) -> bool {
        match provider {
            OAuth2Provider::Google => self.google.is_some(),
            OAuth2Provider::GitHub => self.github.is_some(),
            OAuth2Provider::Microsoft => self.microsoft.is_some(),
            OAuth2Provider::Custom => self.custom.is_some(),
        }
    }

    /// Get configuration for a specific provider
    pub fn get_provider_config(&self, provider: OAuth2Provider) -> Option<&OAuth2ProviderConfig> {
        match provider {
            OAuth2Provider::Google => self.google.as_ref(),
            OAuth2Provider::GitHub => self.github.as_ref(),
            OAuth2Provider::Microsoft => self.microsoft.as_ref(),
            OAuth2Provider::Custom => self.custom.as_ref(),
        }
    }

    /// Get list of configured providers
    pub fn configured_providers(&self) -> Vec<OAuth2Provider> {
        let mut providers = Vec::new();
        if self.google.is_some() {
            providers.push(OAuth2Provider::Google);
        }
        if self.github.is_some() {
            providers.push(OAuth2Provider::GitHub);
        }
        if self.microsoft.is_some() {
            providers.push(OAuth2Provider::Microsoft);
        }
        if self.custom.is_some() {
            providers.push(OAuth2Provider::Custom);
        }
        providers
    }
}

/// User information retrieved from OAuth2 provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2UserInfo {
    /// Provider-specific user ID
    pub provider_user_id: String,

    /// User's email address
    pub email: Option<String>,

    /// Whether the email is verified
    pub email_verified: Option<bool>,

    /// User's display name
    pub name: Option<String>,

    /// User's given (first) name
    pub given_name: Option<String>,

    /// User's family (last) name
    pub family_name: Option<String>,

    /// URL to user's profile picture
    pub picture: Option<String>,

    /// User's locale
    pub locale: Option<String>,

    /// The OAuth2 provider
    pub provider: OAuth2Provider,

    /// Raw user info JSON (for provider-specific fields)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// Authorization URL response
#[derive(Debug, Clone, Serialize)]
pub struct AuthorizationUrlResponse {
    /// The URL to redirect the user to
    pub authorization_url: String,

    /// CSRF state token (should be stored and validated on callback)
    pub state: String,

    /// The OAuth2 provider
    pub provider: String,
}

/// Token exchange result
#[derive(Debug, Clone)]
pub struct OAuth2TokenResult {
    /// Access token
    pub access_token: String,

    /// Refresh token (if provided)
    pub refresh_token: Option<String>,

    /// Token expiration (seconds from now)
    pub expires_in: Option<u64>,

    /// Token scopes
    pub scopes: Vec<String>,
}

/// State storage for PKCE verifiers (in production, use Redis or database)
#[derive(Default)]
pub struct OAuth2StateStore {
    /// Maps CSRF state to PKCE verifier
    verifiers: RwLock<HashMap<String, (PkceCodeVerifier, OAuth2Provider)>>,
}

impl OAuth2StateStore {
    pub fn new() -> Self {
        Self {
            verifiers: RwLock::new(HashMap::new()),
        }
    }

    /// Store a PKCE verifier for a state token
    pub async fn store_verifier(
        &self,
        state: String,
        verifier: PkceCodeVerifier,
        provider: OAuth2Provider,
    ) {
        let mut store = self.verifiers.write().await;
        store.insert(state, (verifier, provider));
    }

    /// Retrieve and remove a PKCE verifier for a state token
    pub async fn take_verifier(&self, state: &str) -> Option<(PkceCodeVerifier, OAuth2Provider)> {
        let mut store = self.verifiers.write().await;
        store.remove(state)
    }

    /// Clean up expired state tokens (call periodically)
    pub async fn cleanup_expired(&self, _max_age_secs: u64) {
        // In a production implementation, you'd track creation time
        // and remove entries older than max_age_secs
        let mut store = self.verifiers.write().await;
        if store.len() > 1000 {
            // Simple cleanup: if too many entries, clear oldest half
            let to_remove: Vec<_> = store.keys().take(store.len() / 2).cloned().collect();
            for key in to_remove {
                store.remove(&key);
            }
            warn!(
                "OAuth2 state store cleanup: removed {} entries",
                store.len()
            );
        }
    }
}

/// OAuth2 service for handling authentication flows
#[derive(Clone)]
pub struct OAuth2Service {
    config: OAuth2Config,
    state_store: Arc<OAuth2StateStore>,
    http_client: reqwest::Client,
}

impl OAuth2Service {
    /// Create a new OAuth2 service
    pub fn new(config: OAuth2Config) -> Self {
        Self {
            config,
            state_store: Arc::new(OAuth2StateStore::new()),
            http_client: reqwest::Client::new(),
        }
    }

    /// Check if OAuth2 is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get list of configured providers
    pub fn configured_providers(&self) -> Vec<OAuth2Provider> {
        self.config.configured_providers()
    }

    /// Build OAuth2 client for a provider
    fn build_client(&self, provider: OAuth2Provider) -> Result<BasicClient, OAuth2Error> {
        let config = self
            .config
            .get_provider_config(provider)
            .ok_or_else(|| OAuth2Error::ProviderNotConfigured(provider.to_string()))?;

        let client = BasicClient::new(
            ClientId::new(config.client_id.clone()),
            Some(ClientSecret::new(config.client_secret.clone())),
            AuthUrl::new(config.auth_url.clone())
                .map_err(|e| OAuth2Error::ConfigurationError(e.to_string()))?,
            Some(
                TokenUrl::new(config.token_url.clone())
                    .map_err(|e| OAuth2Error::ConfigurationError(e.to_string()))?,
            ),
        )
        .set_redirect_uri(
            RedirectUrl::new(config.redirect_url.clone())
                .map_err(|e| OAuth2Error::ConfigurationError(e.to_string()))?,
        );

        // Add revocation URL if configured
        let client = if let Some(ref revocation_url) = config.revocation_url {
            client.set_revocation_uri(
                RevocationUrl::new(revocation_url.clone())
                    .map_err(|e| OAuth2Error::ConfigurationError(e.to_string()))?,
            )
        } else {
            client
        };

        Ok(client)
    }

    /// Generate authorization URL for a provider
    pub async fn get_authorization_url(
        &self,
        provider: OAuth2Provider,
    ) -> Result<AuthorizationUrlResponse, OAuth2Error> {
        let client = self.build_client(provider)?;
        let config = self
            .config
            .get_provider_config(provider)
            .ok_or_else(|| OAuth2Error::ProviderNotConfigured(provider.to_string()))?;

        // Generate PKCE challenge
        let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

        // Build authorization URL with scopes
        let mut auth_request = client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(pkce_challenge);

        for scope in &config.scopes {
            auth_request = auth_request.add_scope(Scope::new(scope.clone()));
        }

        let (auth_url, csrf_state) = auth_request.url();

        // Store PKCE verifier for callback
        self.state_store
            .store_verifier(csrf_state.secret().clone(), pkce_verifier, provider)
            .await;

        info!(
            provider = %provider,
            "Generated OAuth2 authorization URL"
        );

        Ok(AuthorizationUrlResponse {
            authorization_url: auth_url.to_string(),
            state: csrf_state.secret().clone(),
            provider: provider.to_string(),
        })
    }

    /// Exchange authorization code for tokens
    pub async fn exchange_code(
        &self,
        provider: OAuth2Provider,
        code: String,
        state: String,
    ) -> Result<OAuth2TokenResult, OAuth2Error> {
        // Retrieve and validate PKCE verifier
        let (pkce_verifier, stored_provider) = self
            .state_store
            .take_verifier(&state)
            .await
            .ok_or(OAuth2Error::PkceVerifierNotFound)?;

        if stored_provider != provider {
            return Err(OAuth2Error::InvalidState);
        }

        let client = self.build_client(provider)?;

        // Exchange code for token
        let token_result = client
            .exchange_code(AuthorizationCode::new(code))
            .set_pkce_verifier(pkce_verifier)
            .request_async(async_http_client)
            .await
            .map_err(|e| OAuth2Error::TokenExchangeFailed(e.to_string()))?;

        let access_token = token_result.access_token().secret().clone();
        let refresh_token = token_result.refresh_token().map(|t| t.secret().clone());
        let expires_in = token_result.expires_in().map(|d| d.as_secs());
        let scopes: Vec<String> = token_result
            .scopes()
            .map(|s| s.iter().map(|scope| scope.to_string()).collect())
            .unwrap_or_default();

        info!(
            provider = %provider,
            has_refresh_token = refresh_token.is_some(),
            "Successfully exchanged OAuth2 code for tokens"
        );

        Ok(OAuth2TokenResult {
            access_token,
            refresh_token,
            expires_in,
            scopes,
        })
    }

    /// Fetch user info from OAuth2 provider
    pub async fn fetch_user_info(
        &self,
        provider: OAuth2Provider,
        access_token: &str,
    ) -> Result<OAuth2UserInfo, OAuth2Error> {
        let config = self
            .config
            .get_provider_config(provider)
            .ok_or_else(|| OAuth2Error::ProviderNotConfigured(provider.to_string()))?;

        let response = self
            .http_client
            .get(&config.user_info_url)
            .bearer_auth(access_token)
            .header("Accept", "application/json")
            .header("User-Agent", "StateSet-API/1.0")
            .send()
            .await
            .map_err(|e| OAuth2Error::UserInfoFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OAuth2Error::UserInfoFailed(format!(
                "HTTP {}: {}",
                status, body
            )));
        }

        let raw: serde_json::Value = response
            .json()
            .await
            .map_err(|e| OAuth2Error::UserInfoFailed(e.to_string()))?;

        // Parse provider-specific user info format
        let user_info = match provider {
            OAuth2Provider::Google => self.parse_google_user_info(&raw)?,
            OAuth2Provider::GitHub => self.parse_github_user_info(&raw, access_token).await?,
            OAuth2Provider::Microsoft => self.parse_microsoft_user_info(&raw)?,
            OAuth2Provider::Custom => self.parse_generic_user_info(&raw)?,
        };

        info!(
            provider = %provider,
            user_id = %user_info.provider_user_id,
            email = ?user_info.email,
            "Fetched OAuth2 user info"
        );

        Ok(user_info)
    }

    /// Parse Google user info response
    fn parse_google_user_info(
        &self,
        raw: &serde_json::Value,
    ) -> Result<OAuth2UserInfo, OAuth2Error> {
        Ok(OAuth2UserInfo {
            provider_user_id: raw["sub"]
                .as_str()
                .ok_or_else(|| OAuth2Error::UserInfoFailed("Missing 'sub' field".to_string()))?
                .to_string(),
            email: raw["email"].as_str().map(String::from),
            email_verified: raw["email_verified"].as_bool(),
            name: raw["name"].as_str().map(String::from),
            given_name: raw["given_name"].as_str().map(String::from),
            family_name: raw["family_name"].as_str().map(String::from),
            picture: raw["picture"].as_str().map(String::from),
            locale: raw["locale"].as_str().map(String::from),
            provider: OAuth2Provider::Google,
            raw: Some(raw.clone()),
        })
    }

    /// Parse GitHub user info response
    async fn parse_github_user_info(
        &self,
        raw: &serde_json::Value,
        access_token: &str,
    ) -> Result<OAuth2UserInfo, OAuth2Error> {
        // GitHub doesn't always include email in user info, need to fetch separately
        let email = if let Some(email) = raw["email"].as_str() {
            Some(email.to_string())
        } else {
            // Fetch email from /user/emails endpoint
            self.fetch_github_email(access_token).await.ok()
        };

        Ok(OAuth2UserInfo {
            provider_user_id: raw["id"]
                .as_i64()
                .ok_or_else(|| OAuth2Error::UserInfoFailed("Missing 'id' field".to_string()))?
                .to_string(),
            email,
            email_verified: Some(true), // GitHub emails are verified
            name: raw["name"].as_str().map(String::from),
            given_name: None,
            family_name: None,
            picture: raw["avatar_url"].as_str().map(String::from),
            locale: None,
            provider: OAuth2Provider::GitHub,
            raw: Some(raw.clone()),
        })
    }

    /// Fetch GitHub user's primary email
    async fn fetch_github_email(&self, access_token: &str) -> Result<String, OAuth2Error> {
        let response = self
            .http_client
            .get("https://api.github.com/user/emails")
            .bearer_auth(access_token)
            .header("Accept", "application/json")
            .header("User-Agent", "StateSet-API/1.0")
            .send()
            .await
            .map_err(|e| OAuth2Error::UserInfoFailed(e.to_string()))?;

        let emails: Vec<serde_json::Value> = response
            .json()
            .await
            .map_err(|e| OAuth2Error::UserInfoFailed(e.to_string()))?;

        // Find primary email
        for email_obj in &emails {
            if email_obj["primary"].as_bool() == Some(true) {
                if let Some(email) = email_obj["email"].as_str() {
                    return Ok(email.to_string());
                }
            }
        }

        // Fallback to first verified email
        for email_obj in &emails {
            if email_obj["verified"].as_bool() == Some(true) {
                if let Some(email) = email_obj["email"].as_str() {
                    return Ok(email.to_string());
                }
            }
        }

        Err(OAuth2Error::UserInfoFailed(
            "No primary email found".to_string(),
        ))
    }

    /// Parse Microsoft user info response
    fn parse_microsoft_user_info(
        &self,
        raw: &serde_json::Value,
    ) -> Result<OAuth2UserInfo, OAuth2Error> {
        Ok(OAuth2UserInfo {
            provider_user_id: raw["id"]
                .as_str()
                .ok_or_else(|| OAuth2Error::UserInfoFailed("Missing 'id' field".to_string()))?
                .to_string(),
            email: raw["mail"]
                .as_str()
                .or_else(|| raw["userPrincipalName"].as_str())
                .map(String::from),
            email_verified: Some(true), // Microsoft accounts are verified
            name: raw["displayName"].as_str().map(String::from),
            given_name: raw["givenName"].as_str().map(String::from),
            family_name: raw["surname"].as_str().map(String::from),
            picture: None, // Microsoft Graph requires separate call for photo
            locale: raw["preferredLanguage"].as_str().map(String::from),
            provider: OAuth2Provider::Microsoft,
            raw: Some(raw.clone()),
        })
    }

    /// Parse generic OAuth2 user info response
    fn parse_generic_user_info(
        &self,
        raw: &serde_json::Value,
    ) -> Result<OAuth2UserInfo, OAuth2Error> {
        // Try common field names
        let id = raw["sub"]
            .as_str()
            .or_else(|| raw["id"].as_str())
            .or_else(|| raw["user_id"].as_str())
            .ok_or_else(|| {
                OAuth2Error::UserInfoFailed(
                    "Missing user ID field (sub, id, or user_id)".to_string(),
                )
            })?;

        Ok(OAuth2UserInfo {
            provider_user_id: id.to_string(),
            email: raw["email"].as_str().map(String::from),
            email_verified: raw["email_verified"].as_bool(),
            name: raw["name"]
                .as_str()
                .or_else(|| raw["display_name"].as_str())
                .map(String::from),
            given_name: raw["given_name"]
                .as_str()
                .or_else(|| raw["first_name"].as_str())
                .map(String::from),
            family_name: raw["family_name"]
                .as_str()
                .or_else(|| raw["last_name"].as_str())
                .map(String::from),
            picture: raw["picture"]
                .as_str()
                .or_else(|| raw["avatar_url"].as_str())
                .or_else(|| raw["avatar"].as_str())
                .map(String::from),
            locale: raw["locale"].as_str().map(String::from),
            provider: OAuth2Provider::Custom,
            raw: Some(raw.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_str() {
        assert_eq!(
            "google".parse::<OAuth2Provider>().unwrap(),
            OAuth2Provider::Google
        );
        assert_eq!(
            "GitHub".parse::<OAuth2Provider>().unwrap(),
            OAuth2Provider::GitHub
        );
        assert_eq!(
            "MICROSOFT".parse::<OAuth2Provider>().unwrap(),
            OAuth2Provider::Microsoft
        );
        assert!("invalid".parse::<OAuth2Provider>().is_err());
    }

    #[test]
    fn test_google_config() {
        let config = OAuth2ProviderConfig::google(
            "client_id".to_string(),
            "client_secret".to_string(),
            "http://localhost/callback".to_string(),
        );
        assert!(config.auth_url.contains("google"));
        assert!(config.scopes.contains(&"openid".to_string()));
    }

    #[test]
    fn test_github_config() {
        let config = OAuth2ProviderConfig::github(
            "client_id".to_string(),
            "client_secret".to_string(),
            "http://localhost/callback".to_string(),
        );
        assert!(config.auth_url.contains("github"));
        assert!(config.scopes.contains(&"user:email".to_string()));
    }

    #[test]
    fn test_microsoft_config() {
        let config = OAuth2ProviderConfig::microsoft(
            "client_id".to_string(),
            "client_secret".to_string(),
            "http://localhost/callback".to_string(),
            Some("tenant123".to_string()),
        );
        assert!(config.auth_url.contains("tenant123"));
    }

    #[test]
    fn test_oauth2_config_providers() {
        let config = OAuth2Config {
            enabled: true,
            google: Some(OAuth2ProviderConfig::google(
                "id".to_string(),
                "secret".to_string(),
                "http://localhost".to_string(),
            )),
            github: None,
            microsoft: None,
            custom: None,
        };

        assert!(config.is_provider_configured(OAuth2Provider::Google));
        assert!(!config.is_provider_configured(OAuth2Provider::GitHub));
        assert_eq!(config.configured_providers().len(), 1);
    }

    #[tokio::test]
    async fn test_state_store() {
        let store = OAuth2StateStore::new();
        let verifier = PkceCodeVerifier::new("test_verifier".to_string());

        store
            .store_verifier("state1".to_string(), verifier, OAuth2Provider::Google)
            .await;

        let result = store.take_verifier("state1").await;
        assert!(result.is_some());

        let result = store.take_verifier("state1").await;
        assert!(result.is_none()); // Should be removed after first take
    }
}
