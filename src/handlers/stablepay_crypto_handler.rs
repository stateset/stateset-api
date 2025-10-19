use crate::{
    errors::ServiceError,
    services::stablepay_crypto_service::{
        AddCryptoWalletRequest, CreateCryptoPaymentRequest, StablePayCryptoService,
    },
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

/// Shared state for StablePay Crypto handlers
#[derive(Clone)]
pub struct StablePayCryptoState {
    pub service: Arc<StablePayCryptoService>,
}

/// API response wrapper
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: String) -> ApiResponse<()> {
        ApiResponse {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

/// Create a crypto payment (USDC/USDT)
pub async fn create_crypto_payment(
    State(state): State<Arc<StablePayCryptoState>>,
    Json(request): Json<CreateCryptoPaymentRequest>,
) -> impl IntoResponse {
    info!("Creating crypto payment: {:?}", request);

    match state.service.create_crypto_payment(request).await {
        Ok(payment) => {
            info!(
                "Crypto payment created successfully: {}",
                payment.payment_id
            );
            (StatusCode::CREATED, Json(ApiResponse::success(payment))).into_response()
        }
        Err(e) => {
            error!("Failed to create crypto payment: {:?}", e);
            handle_error(e)
        }
    }
}

/// Add a crypto wallet
pub async fn add_crypto_wallet(
    State(state): State<Arc<StablePayCryptoState>>,
    Json(request): Json<AddCryptoWalletRequest>,
) -> impl IntoResponse {
    info!("Adding crypto wallet: {:?}", request);

    match state.service.add_crypto_wallet(request).await {
        Ok(wallet) => {
            info!("Crypto wallet added successfully: {}", wallet.id);
            (StatusCode::CREATED, Json(ApiResponse::success(wallet))).into_response()
        }
        Err(e) => {
            error!("Failed to add crypto wallet: {:?}", e);
            handle_error(e)
        }
    }
}

/// List customer's crypto wallets
pub async fn list_customer_wallets(
    State(state): State<Arc<StablePayCryptoState>>,
    Path(customer_id): Path<Uuid>,
) -> impl IntoResponse {
    info!("Listing crypto wallets for customer: {}", customer_id);

    match state.service.list_customer_wallets(customer_id).await {
        Ok(wallets) => {
            info!("Retrieved {} crypto wallets", wallets.len());
            (StatusCode::OK, Json(ApiResponse::success(wallets))).into_response()
        }
        Err(e) => {
            error!("Failed to list crypto wallets: {:?}", e);
            handle_error(e)
        }
    }
}

/// Get supported blockchains
pub async fn get_supported_blockchains(
    State(state): State<Arc<StablePayCryptoState>>,
) -> impl IntoResponse {
    info!("Getting supported blockchains");

    match state.service.get_supported_blockchains().await {
        Ok(blockchains) => {
            info!("Retrieved {} supported blockchains", blockchains.len());

            // Map to a simpler response format
            #[derive(Serialize)]
            struct BlockchainInfo {
                id: Uuid,
                blockchain: String,
                network: String,
                full_name: String,
                chain_id: Option<i32>,
                native_token: String,
                explorer_url: Option<String>,
                is_layer2: bool,
                estimated_confirmation_time_minutes: Option<i32>,
            }

            let blockchain_info: Vec<BlockchainInfo> = blockchains
                .into_iter()
                .map(|b| BlockchainInfo {
                    id: b.id,
                    blockchain: b.blockchain.clone(),
                    network: b.network.clone(),
                    full_name: b.full_name(),
                    chain_id: b.chain_id,
                    native_token: b.native_token_symbol.clone(),
                    explorer_url: b.explorer_url.clone(),
                    is_layer2: b.is_layer2(),
                    estimated_confirmation_time_minutes: b.estimated_confirmation_time_minutes(),
                })
                .collect();

            (StatusCode::OK, Json(ApiResponse::success(blockchain_info))).into_response()
        }
        Err(e) => {
            error!("Failed to get supported blockchains: {:?}", e);
            handle_error(e)
        }
    }
}

/// Health check endpoint for crypto payments
pub async fn health_check() -> impl IntoResponse {
    #[derive(Serialize)]
    struct Health {
        status: String,
        service: String,
        supported_tokens: Vec<String>,
    }

    Json(Health {
        status: "healthy".to_string(),
        service: "StablePay Crypto".to_string(),
        supported_tokens: vec!["USDC".to_string(), "USDT".to_string()],
    })
}

fn handle_error(error: ServiceError) -> axum::response::Response {
    let (status, message) = match error {
        ServiceError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        ServiceError::ValidationError(msg) => (StatusCode::BAD_REQUEST, msg),
        ServiceError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
        ServiceError::ExternalApiError(msg) => (StatusCode::BAD_GATEWAY, msg),
        ServiceError::db_error(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Database error".to_string(),
        ),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal server error".to_string(),
        ),
    };

    (status, Json(ApiResponse::<()>::error(message))).into_response()
}

/// Register StablePay Crypto routes
pub fn stablepay_crypto_routes() -> axum::Router<Arc<StablePayCryptoState>> {
    use axum::routing::{get, post};

    axum::Router::new()
        .route("/health", get(health_check))
        .route("/crypto/payments", post(create_crypto_payment))
        .route("/crypto/wallets", post(add_crypto_wallet))
        .route(
            "/crypto/customers/:customer_id/wallets",
            get(list_customer_wallets),
        )
        .route("/crypto/blockchains", get(get_supported_blockchains))
}
