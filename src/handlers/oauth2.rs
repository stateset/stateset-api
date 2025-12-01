/*!
 * # OAuth2 HTTP Handlers
 *
 * This module provides HTTP handlers for OAuth2 authentication flows.
 *
 * ## Endpoints
 *
 * - `GET /auth/oauth2/providers` - List configured OAuth2 providers
 * - `GET /auth/oauth2/{provider}/authorize` - Get authorization URL
 * - `GET /auth/oauth2/{provider}/callback` - Handle OAuth2 callback
 * - `POST /auth/oauth2/{provider}/token` - Exchange code for tokens (for SPAs)
 */

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect},
    routing::{get, post},
    Json, Router,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

use crate::{
    auth::{
        oauth2::{OAuth2Error, OAuth2Provider, OAuth2Service, OAuth2UserInfo},
        Claims,
    },
    errors::ApiError,
    handlers::common::success_response,
};

/// OAuth2 state for handlers
#[derive(Clone)]
pub struct OAuth2State {
    pub oauth2_service: Arc<OAuth2Service>,
    pub jwt_secret: String,
    pub jwt_expiration: i64,
    pub frontend_url: String,
}

/// Query parameters for OAuth2 callback
#[derive(Debug, Deserialize)]
pub struct OAuth2CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// Request body for token exchange (SPA flow)
#[derive(Debug, Deserialize)]
pub struct TokenExchangeRequest {
    pub code: String,
    pub state: String,
}

/// Response for provider list
#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderInfo>,
}

/// Information about a configured provider
#[derive(Debug, Serialize)]
pub struct ProviderInfo {
    pub name: String,
    pub display_name: String,
    pub authorize_url: String,
}

/// Authorization URL response
#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    pub authorization_url: String,
    pub state: String,
    pub provider: String,
}

/// Token response after successful OAuth2 flow
#[derive(Debug, Serialize)]
pub struct OAuth2TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i64,
    pub user: OAuth2UserResponse,
}

/// User info in token response
#[derive(Debug, Serialize)]
pub struct OAuth2UserResponse {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub provider: String,
}

/// Create OAuth2 router
pub fn oauth2_router() -> Router<OAuth2State> {
    Router::new()
        .route("/providers", get(list_providers))
        .route("/:provider/authorize", get(get_authorize_url))
        .route("/:provider/callback", get(handle_callback))
        .route("/:provider/token", post(exchange_token))
}

/// List configured OAuth2 providers
///
/// GET /auth/oauth2/providers
pub async fn list_providers(
    State(state): State<OAuth2State>,
) -> Result<impl IntoResponse, ApiError> {
    let providers: Vec<ProviderInfo> = state
        .oauth2_service
        .configured_providers()
        .into_iter()
        .map(|p| {
            let display_name = match p {
                OAuth2Provider::Google => "Google",
                OAuth2Provider::GitHub => "GitHub",
                OAuth2Provider::Microsoft => "Microsoft",
                OAuth2Provider::Custom => "Custom",
            };
            ProviderInfo {
                name: p.to_string(),
                display_name: display_name.to_string(),
                authorize_url: format!("/auth/oauth2/{}/authorize", p),
            }
        })
        .collect();

    Ok(success_response(ProvidersResponse { providers }))
}

/// Get authorization URL for a provider
///
/// GET /auth/oauth2/{provider}/authorize
pub async fn get_authorize_url(
    State(state): State<OAuth2State>,
    Path(provider): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let provider: OAuth2Provider = provider
        .parse()
        .map_err(|_| ApiError::BadRequest {
            message: format!("Unknown OAuth2 provider: {}", provider),
            error_code: Some("invalid_provider".to_string()),
        })?;

    let auth_response = state
        .oauth2_service
        .get_authorization_url(provider)
        .await
        .map_err(|e| match e {
            OAuth2Error::ProviderNotConfigured(p) => ApiError::BadRequest {
                message: format!("Provider not configured: {}", p),
                error_code: Some("provider_not_configured".to_string()),
            },
            OAuth2Error::ConfigurationError(_) => ApiError::InternalServerError,
            _ => ApiError::InternalServerError,
        })?;

    Ok(success_response(AuthorizeResponse {
        authorization_url: auth_response.authorization_url,
        state: auth_response.state,
        provider: auth_response.provider,
    }))
}

/// Handle OAuth2 callback (redirect flow)
///
/// GET /auth/oauth2/{provider}/callback
pub async fn handle_callback(
    State(state): State<OAuth2State>,
    Path(provider): Path<String>,
    Query(query): Query<OAuth2CallbackQuery>,
) -> Result<impl IntoResponse, ApiError> {
    // Check for OAuth2 error response
    if let Some(error) = &query.error {
        let description = query.error_description.as_deref().unwrap_or("Unknown error");
        warn!(
            provider = %provider,
            error = %error,
            description = %description,
            "OAuth2 callback error"
        );
        // Redirect to frontend with error
        let redirect_url = format!(
            "{}?error={}&error_description={}",
            state.frontend_url,
            urlencoding::encode(error),
            urlencoding::encode(description)
        );
        return Ok(Redirect::temporary(&redirect_url).into_response());
    }

    let code = query.code.ok_or_else(|| ApiError::BadRequest {
        message: "Missing authorization code".to_string(),
        error_code: Some("missing_code".to_string()),
    })?;
    let csrf_state = query.state.ok_or_else(|| ApiError::BadRequest {
        message: "Missing state parameter".to_string(),
        error_code: Some("missing_state".to_string()),
    })?;

    let provider: OAuth2Provider = provider.parse().map_err(|_| ApiError::BadRequest {
        message: "Unknown OAuth2 provider".to_string(),
        error_code: Some("invalid_provider".to_string()),
    })?;

    // Exchange code for tokens
    let token_result = state
        .oauth2_service
        .exchange_code(provider, code, csrf_state)
        .await
        .map_err(|e| {
            error!(error = %e, "OAuth2 token exchange failed");
            match e {
                OAuth2Error::InvalidState => ApiError::BadRequest {
                    message: "Invalid state parameter".to_string(),
                    error_code: Some("invalid_state".to_string()),
                },
                OAuth2Error::PkceVerifierNotFound => ApiError::BadRequest {
                    message: "Session expired, please try again".to_string(),
                    error_code: Some("session_expired".to_string()),
                },
                _ => ApiError::InternalServerError,
            }
        })?;

    // Fetch user info
    let user_info = state
        .oauth2_service
        .fetch_user_info(provider, &token_result.access_token)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch OAuth2 user info");
            ApiError::InternalServerError
        })?;

    // Generate JWT token for our API
    let (access_token, refresh_token) = generate_tokens(&state, &user_info)?;

    info!(
        provider = %provider,
        user_id = %user_info.provider_user_id,
        email = ?user_info.email,
        "OAuth2 authentication successful"
    );

    // Redirect to frontend with tokens
    let redirect_url = format!(
        "{}?access_token={}&refresh_token={}&provider={}",
        state.frontend_url,
        urlencoding::encode(&access_token),
        urlencoding::encode(&refresh_token),
        provider
    );

    Ok(Redirect::temporary(&redirect_url).into_response())
}

/// Exchange authorization code for tokens (SPA flow)
///
/// POST /auth/oauth2/{provider}/token
pub async fn exchange_token(
    State(state): State<OAuth2State>,
    Path(provider): Path<String>,
    Json(payload): Json<TokenExchangeRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let provider: OAuth2Provider = provider.parse().map_err(|_| ApiError::BadRequest {
        message: "Unknown OAuth2 provider".to_string(),
        error_code: Some("invalid_provider".to_string()),
    })?;

    // Exchange code for tokens
    let token_result = state
        .oauth2_service
        .exchange_code(provider, payload.code, payload.state)
        .await
        .map_err(|e| {
            error!(error = %e, "OAuth2 token exchange failed");
            match e {
                OAuth2Error::InvalidState => ApiError::BadRequest {
                    message: "Invalid state parameter".to_string(),
                    error_code: Some("invalid_state".to_string()),
                },
                OAuth2Error::PkceVerifierNotFound => ApiError::BadRequest {
                    message: "Session expired, please try again".to_string(),
                    error_code: Some("session_expired".to_string()),
                },
                OAuth2Error::TokenExchangeFailed(msg) => ApiError::BadRequest {
                    message: format!("Token exchange failed: {}", msg),
                    error_code: Some("token_exchange_failed".to_string()),
                },
                _ => ApiError::InternalServerError,
            }
        })?;

    // Fetch user info
    let user_info = state
        .oauth2_service
        .fetch_user_info(provider, &token_result.access_token)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to fetch OAuth2 user info");
            ApiError::InternalServerError
        })?;

    // Generate JWT tokens for our API
    let (access_token, refresh_token) = generate_tokens(&state, &user_info)?;

    info!(
        provider = %provider,
        user_id = %user_info.provider_user_id,
        email = ?user_info.email,
        "OAuth2 token exchange successful"
    );

    Ok(success_response(OAuth2TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
        expires_in: state.jwt_expiration,
        user: OAuth2UserResponse {
            id: user_info.provider_user_id,
            email: user_info.email,
            name: user_info.name,
            picture: user_info.picture,
            provider: provider.to_string(),
        },
    }))
}

/// Generate JWT tokens for authenticated user
fn generate_tokens(state: &OAuth2State, user_info: &OAuth2UserInfo) -> Result<(String, String), ApiError> {
    let now = Utc::now();
    let exp = (now + Duration::seconds(state.jwt_expiration)).timestamp();
    let jti = Uuid::new_v4().to_string();

    let claims = Claims {
        sub: user_info.provider_user_id.clone(),
        name: user_info.name.clone(),
        email: user_info.email.clone(),
        roles: vec!["user".to_string()],
        permissions: vec![],
        tenant_id: None,
        jti: jti.clone(),
        iat: now.timestamp(),
        exp,
        nbf: now.timestamp(),
        iss: "stateset-api".to_string(),
        aud: "stateset-api".to_string(),
        scope: Some("openid email profile".to_string()),
    };

    let access_token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|e| {
        error!(error = %e, "Failed to encode JWT");
        ApiError::InternalServerError
    })?;

    let refresh_token = Uuid::new_v4().to_string();

    Ok((access_token, refresh_token))
}

/// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}
