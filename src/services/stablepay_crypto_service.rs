use crate::{
    errors::ServiceError,
    events::{Event, EventSender},
    models::{
        stablepay_blockchain_network, stablepay_crypto_transaction, stablepay_crypto_wallet,
        stablepay_provider, stablepay_transaction,
    },
};
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;
use validator::{Validate, ValidationError};

fn validate_wallet_address(address: &str) -> Result<(), ValidationError> {
    if address.starts_with("0x") && address.len() == 42 {
        Ok(())
    } else if address.len() >= 32 && address.len() <= 44 {
        Ok(()) // Solana address
    } else {
        let mut err = ValidationError::new("invalid_address");
        err.message = Some("Invalid wallet address format".into());
        Err(err)
    }
}

/// Request to create a crypto payment
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct CreateCryptoPaymentRequest {
    pub customer_id: Uuid,
    pub order_id: Option<Uuid>,
    pub amount: Decimal,
    pub token_symbol: String, // USDC or USDT
    pub blockchain: String,   // ethereum, polygon, arbitrum, etc.
    #[validate(custom = "validate_wallet_address")]
    pub from_address: String, // Customer's wallet
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Response for crypto payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoPaymentResponse {
    pub payment_id: Uuid,
    pub transaction_number: String,
    pub crypto_transaction_id: Uuid,
    pub amount: Decimal,
    pub token_symbol: String,
    pub blockchain: String,
    pub network: String,
    pub to_address: String,
    pub payment_address_label: String,
    pub tx_hash: Option<String>,
    pub status: String,
    pub confirmations: i32,
    pub required_confirmations: i32,
    pub confirmation_progress: Decimal,
    pub estimated_confirmation_time_minutes: Option<i32>,
    pub gas_estimate_usd: Option<Decimal>,
    pub total_cost_usd: Decimal,
    pub explorer_url: Option<String>,
    pub created_at: chrono::DateTime<Utc>,
}

/// Request to add a crypto wallet
#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct AddCryptoWalletRequest {
    pub customer_id: Uuid,
    #[validate(custom = "validate_wallet_address")]
    pub wallet_address: String,
    pub blockchain: String,
    pub label: Option<String>,
    pub set_as_default: Option<bool>,
}

/// Crypto wallet response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CryptoWalletResponse {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub wallet_address: String,
    pub short_address: String,
    pub blockchain: String,
    pub wallet_type: String,
    pub label: Option<String>,
    pub is_verified: bool,
    pub is_default: bool,
    pub created_at: chrono::DateTime<Utc>,
}

/// Supported stablecoins
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StableCoin {
    USDC,
    USDT,
}

impl std::fmt::Display for StableCoin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::USDC => write!(f, "USDC"),
            Self::USDT => write!(f, "USDT"),
        }
    }
}

/// StablePay Crypto Service
pub struct StablePayCryptoService {
    db: Arc<DatabaseConnection>,
    event_sender: Arc<EventSender>,
}

impl StablePayCryptoService {
    pub fn new(db: Arc<DatabaseConnection>, event_sender: Arc<EventSender>) -> Self {
        Self { db, event_sender }
    }

    /// Create a crypto payment
    #[instrument(skip(self, request))]
    pub async fn create_crypto_payment(
        &self,
        request: CreateCryptoPaymentRequest,
    ) -> Result<CryptoPaymentResponse, ServiceError> {
        request.validate()?;

        info!(
            customer_id = %request.customer_id,
            amount = %request.amount,
            token = %request.token_symbol,
            blockchain = %request.blockchain,
            "Creating crypto payment"
        );

        // Get blockchain network
        let network = self.get_network(&request.blockchain, "mainnet").await?;

        // Get token contract
        let token_contract = self
            .get_token_contract(network.id, &request.token_symbol)
            .await?;

        // Get or create payment address for this network
        let payment_address = self.get_payment_address(network.id).await?;

        // Get crypto provider
        let provider = self.get_crypto_provider().await?;

        // Calculate fees (0.5% for crypto, no gas fees passed to customer)
        let provider_fee = request.amount * provider.fee_percentage;
        let platform_fee = dec!(0); // No additional platform fee for crypto
        let total_fees = provider_fee;
        let net_amount = request.amount - total_fees;

        // Generate transaction number
        let transaction_number = self.generate_transaction_number().await?;

        // Create main payment transaction
        let payment_id = Uuid::new_v4();
        let now = Utc::now();

        let payment_model = stablepay_transaction::ActiveModel {
            id: Set(payment_id),
            transaction_number: Set(transaction_number.clone()),
            order_id: Set(request.order_id),
            customer_id: Set(request.customer_id),
            payment_method_id: Set(None),
            provider_id: Set(provider.id),
            amount: Set(request.amount),
            currency: Set(request.token_symbol.clone()),
            original_amount: Set(None),
            original_currency: Set(None),
            exchange_rate: Set(None),
            provider_fee: Set(provider_fee),
            platform_fee: Set(platform_fee),
            total_fees: Set(total_fees),
            net_amount: Set(net_amount),
            status: Set("pending".to_string()),
            payment_intent_id: Set(None),
            charge_id: Set(None),
            initiated_at: Set(now),
            processed_at: Set(None),
            settled_at: Set(None),
            estimated_settlement_date: Set(None), // Instant settlement for crypto
            failure_code: Set(None),
            failure_message: Set(None),
            retry_count: Set(0),
            is_reconciled: Set(false),
            reconciled_at: Set(None),
            reconciliation_id: Set(None),
            risk_score: Set(Some(dec!(5))), // Lower risk for crypto
            is_flagged_for_review: Set(false),
            fraud_indicators: Set(None),
            description: Set(request.description.clone()),
            metadata: Set(request.metadata.clone()),
            gateway_response: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
            created_by: Set(None),
            idempotency_key: Set(None),
        };

        let payment = payment_model
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Create crypto transaction record
        let crypto_tx_id = Uuid::new_v4();
        let required_confirmations = self.get_required_confirmations(&network.blockchain);

        let crypto_tx_model = stablepay_crypto_transaction::ActiveModel {
            id: Set(crypto_tx_id),
            transaction_id: Set(payment_id),
            blockchain: Set(network.blockchain.clone()),
            network: Set(network.network.clone()),
            token_contract_address: Set(token_contract.contract_address.clone()),
            token_symbol: Set(token_contract.token_symbol.clone()),
            token_decimals: Set(token_contract.decimals),
            tx_hash: Set(None), // Will be set when customer sends transaction
            block_number: Set(None),
            block_timestamp: Set(None),
            from_address: Set(request.from_address.clone()),
            to_address: Set(payment_address.clone()),
            amount_raw: Set(token_contract.to_raw_amount(request.amount)),
            amount_decimal: Set(request.amount),
            gas_price_gwei: Set(None),
            gas_used: Set(None),
            gas_cost_native: Set(None),
            gas_cost_usd: Set(None),
            status: Set("pending".to_string()),
            confirmations: Set(0),
            required_confirmations: Set(required_confirmations),
            error_code: Set(None),
            error_message: Set(None),
            nonce: Set(None),
            input_data: Set(None),
            metadata: Set(request.metadata),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };

        let crypto_tx = crypto_tx_model
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        // Estimate gas cost
        let gas_estimate = network
            .estimate_gas_cost_usd(21000, dec!(2500)) // Standard ERC20 transfer
            .unwrap_or(dec!(5));

        let total_cost = request.amount + gas_estimate;

        // Send event
        let event = Event::PaymentProcessed {
            transaction_id: payment_id,
            order_id: request.order_id,
            customer_id: request.customer_id,
            amount: request.amount,
            currency: request.token_symbol.clone(),
            status: "pending".to_string(),
        };

        if let Err(e) = self.event_sender.send(event).await {
            warn!(error = ?e, "Failed to send payment event");
        }

        let explorer_url = network
            .explorer_url
            .as_ref()
            .map(|base| format!("{}/address/{}", base, payment_address));

        Ok(CryptoPaymentResponse {
            payment_id,
            transaction_number,
            crypto_transaction_id: crypto_tx_id,
            amount: request.amount,
            token_symbol: token_contract.token_symbol,
            blockchain: network.blockchain.clone(),
            network: network.network.clone(),
            to_address: payment_address.clone(),
            payment_address_label: "StablePay Merchant".to_string(),
            tx_hash: None,
            status: "pending".to_string(),
            confirmations: 0,
            required_confirmations,
            confirmation_progress: dec!(0),
            estimated_confirmation_time_minutes: network.estimated_confirmation_time_minutes(),
            gas_estimate_usd: Some(gas_estimate),
            total_cost_usd: total_cost,
            explorer_url,
            created_at: now,
        })
    }

    /// Add a crypto wallet for a customer
    #[instrument(skip(self, request))]
    pub async fn add_crypto_wallet(
        &self,
        request: AddCryptoWalletRequest,
    ) -> Result<CryptoWalletResponse, ServiceError> {
        request.validate()?;

        info!(
            customer_id = %request.customer_id,
            blockchain = %request.blockchain,
            "Adding crypto wallet"
        );

        // Check if wallet already exists
        let existing = stablepay_crypto_wallet::Entity::find()
            .filter(stablepay_crypto_wallet::Column::WalletAddress.eq(&request.wallet_address))
            .filter(stablepay_crypto_wallet::Column::Blockchain.eq(&request.blockchain))
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        if existing.is_some() {
            return Err(ServiceError::ValidationError(
                "Wallet already exists".to_string(),
            ));
        }

        let wallet_id = Uuid::new_v4();
        let now = Utc::now();

        let wallet_model = stablepay_crypto_wallet::ActiveModel {
            id: Set(wallet_id),
            customer_id: Set(request.customer_id),
            wallet_address: Set(request.wallet_address.clone()),
            blockchain: Set(request.blockchain.clone()),
            wallet_type: Set("non_custodial".to_string()),
            label: Set(request.label.clone()),
            is_verified: Set(false),
            is_default: Set(request.set_as_default.unwrap_or(false)),
            last_used_at: Set(None),
            created_at: Set(now),
            updated_at: Set(Some(now)),
        };

        let wallet = wallet_model
            .insert(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(CryptoWalletResponse {
            id: wallet.id,
            customer_id: wallet.customer_id,
            wallet_address: wallet.wallet_address.clone(),
            short_address: wallet.short_address(),
            blockchain: wallet.blockchain,
            wallet_type: wallet.wallet_type,
            label: wallet.label,
            is_verified: wallet.is_verified,
            is_default: wallet.is_default,
            created_at: wallet.created_at,
        })
    }

    /// List customer's crypto wallets
    pub async fn list_customer_wallets(
        &self,
        customer_id: Uuid,
    ) -> Result<Vec<CryptoWalletResponse>, ServiceError> {
        let wallets = stablepay_crypto_wallet::Entity::find()
            .filter(stablepay_crypto_wallet::Column::CustomerId.eq(customer_id))
            .order_by_desc(stablepay_crypto_wallet::Column::IsDefault)
            .order_by_desc(stablepay_crypto_wallet::Column::CreatedAt)
            .all(&*self.db)
            .await
            .map_err(ServiceError::db_error)?;

        Ok(wallets
            .into_iter()
            .map(|w| CryptoWalletResponse {
                id: w.id,
                customer_id: w.customer_id,
                wallet_address: w.wallet_address.clone(),
                short_address: w.short_address(),
                blockchain: w.blockchain,
                wallet_type: w.wallet_type,
                label: w.label,
                is_verified: w.is_verified,
                is_default: w.is_default,
                created_at: w.created_at,
            })
            .collect())
    }

    /// Get supported blockchains
    pub async fn get_supported_blockchains(
        &self,
    ) -> Result<Vec<stablepay_blockchain_network::Model>, ServiceError> {
        stablepay_blockchain_network::Entity::find()
            .filter(stablepay_blockchain_network::Column::IsActive.eq(true))
            .filter(stablepay_blockchain_network::Column::IsTestnet.eq(false))
            .order_by_asc(stablepay_blockchain_network::Column::Blockchain)
            .all(&*self.db)
            .await
            .map_err(ServiceError::db_error)
    }

    // Private helper methods

    async fn get_network(
        &self,
        blockchain: &str,
        network: &str,
    ) -> Result<stablepay_blockchain_network::Model, ServiceError> {
        stablepay_blockchain_network::Entity::find()
            .filter(stablepay_blockchain_network::Column::Blockchain.eq(blockchain))
            .filter(stablepay_blockchain_network::Column::Network.eq(network))
            .filter(stablepay_blockchain_network::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound(format!("Network {} not found", blockchain)))
    }

    async fn get_token_contract(
        &self,
        network_id: Uuid,
        token_symbol: &str,
    ) -> Result<stablepay_blockchain_network::TokenContract, ServiceError> {
        stablepay_blockchain_network::stablepay_token_contracts::Entity::find()
            .filter(
                stablepay_blockchain_network::stablepay_token_contracts::Column::NetworkId
                    .eq(network_id),
            )
            .filter(
                stablepay_blockchain_network::stablepay_token_contracts::Column::TokenSymbol
                    .eq(token_symbol),
            )
            .filter(
                stablepay_blockchain_network::stablepay_token_contracts::Column::IsActive.eq(true),
            )
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| {
                ServiceError::NotFound(format!("Token {} not found on network", token_symbol))
            })
    }

    async fn get_payment_address(&self, network_id: Uuid) -> Result<String, ServiceError> {
        // In production, this would return a unique payment address per transaction
        // For demo, return a static merchant address
        Ok("0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb".to_string())
    }

    async fn get_crypto_provider(&self) -> Result<stablepay_provider::Model, ServiceError> {
        stablepay_provider::Entity::find()
            .filter(stablepay_provider::Column::ProviderType.eq("crypto"))
            .filter(stablepay_provider::Column::IsActive.eq(true))
            .one(&*self.db)
            .await
            .map_err(ServiceError::db_error)?
            .ok_or_else(|| ServiceError::NotFound("Crypto provider not found".to_string()))
    }

    fn get_required_confirmations(&self, blockchain: &str) -> i32 {
        match blockchain {
            "ethereum" => 12,
            "polygon" => 128,
            "arbitrum" => 1,
            "optimism" => 1,
            "base" => 1,
            _ => 12,
        }
    }

    async fn generate_transaction_number(&self) -> Result<String, ServiceError> {
        let timestamp = Utc::now().format("%Y%m%d");
        let random = Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap()
            .to_uppercase();
        Ok(format!("CRYPTO-{}-{}", timestamp, random))
    }
}
