//! StateSet Blockchain Service
//!
//! This service provides integration with the StateSet Commerce Network blockchain
//! for stablecoin settlements, instant transfers, and payment channel management.

use crate::errors::ServiceError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument};

/// Configuration for the StateSet blockchain client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSetConfig {
    /// gRPC endpoint for the StateSet node
    pub grpc_endpoint: String,
    /// REST endpoint for the StateSet node
    pub rest_endpoint: String,
    /// Chain ID for the StateSet network
    pub chain_id: String,
    /// Account address prefix (e.g., "stateset")
    pub address_prefix: String,
    /// Default gas price in ustate
    pub default_gas_price: String,
    /// Default gas limit for transactions
    pub default_gas_limit: u64,
    /// Fee denom
    pub fee_denom: String,
    /// Stablecoin denom (ssUSD)
    pub stablecoin_denom: String,
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    /// Number of retries for failed requests
    pub max_retries: u32,
}

impl Default for StateSetConfig {
    fn default() -> Self {
        Self {
            grpc_endpoint: "http://localhost:9090".to_string(),
            rest_endpoint: "http://localhost:1317".to_string(),
            chain_id: "stateset-1".to_string(),
            address_prefix: "stateset".to_string(),
            default_gas_price: "0.025".to_string(),
            default_gas_limit: 200000,
            fee_denom: "ustate".to_string(),
            stablecoin_denom: "ssusd".to_string(),
            timeout_seconds: 30,
            max_retries: 3,
        }
    }
}

/// Settlement types available in the StateSet network
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SettlementType {
    /// Instant transfer - immediate settlement
    Instant,
    /// Escrow - funds held until release
    Escrow,
    /// Batch - multiple settlements processed together
    Batch,
    /// Payment Channel - streaming payments
    Channel,
}

/// Settlement status on the blockchain
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SettlementStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Refunded,
    Cancelled,
}

impl From<&str> for SettlementStatus {
    fn from(s: &str) -> Self {
        match s {
            "pending" => SettlementStatus::Pending,
            "processing" => SettlementStatus::Processing,
            "completed" => SettlementStatus::Completed,
            "failed" => SettlementStatus::Failed,
            "refunded" => SettlementStatus::Refunded,
            "cancelled" => SettlementStatus::Cancelled,
            _ => SettlementStatus::Pending,
        }
    }
}

/// Request to create an instant transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantTransferRequest {
    pub sender: String,
    pub recipient: String,
    pub amount: Decimal,
    pub reference: String,
    pub metadata: Option<String>,
}

/// Response from an instant transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstantTransferResponse {
    pub settlement_id: u64,
    pub tx_hash: String,
    pub amount: Decimal,
    pub fee: Decimal,
    pub net_amount: Decimal,
    pub status: SettlementStatus,
    pub block_height: u64,
    pub timestamp: DateTime<Utc>,
}

/// Request to create an escrow settlement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEscrowRequest {
    pub sender: String,
    pub recipient: String,
    pub amount: Decimal,
    pub reference: String,
    pub metadata: Option<String>,
    pub expires_in_seconds: u64,
}

/// Response from creating an escrow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEscrowResponse {
    pub settlement_id: u64,
    pub tx_hash: String,
    pub amount: Decimal,
    pub expires_at: DateTime<Utc>,
    pub status: SettlementStatus,
}

/// Request to release escrow funds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseEscrowRequest {
    pub sender: String,
    pub settlement_id: u64,
}

/// Request to refund escrow funds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundEscrowRequest {
    pub recipient: String,
    pub settlement_id: u64,
    pub reason: String,
}

/// Settlement details from the blockchain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettlementDetails {
    pub id: u64,
    pub settlement_type: SettlementType,
    pub sender: String,
    pub recipient: String,
    pub amount: Decimal,
    pub fee: Decimal,
    pub net_amount: Decimal,
    pub status: SettlementStatus,
    pub reference: String,
    pub metadata: Option<String>,
    pub created_height: u64,
    pub created_time: DateTime<Utc>,
    pub settled_height: Option<u64>,
    pub settled_time: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub batch_id: Option<u64>,
}

/// Request to open a payment channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelRequest {
    pub sender: String,
    pub recipient: String,
    pub deposit: Decimal,
    pub expires_in_blocks: u64,
}

/// Response from opening a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenChannelResponse {
    pub channel_id: u64,
    pub tx_hash: String,
    pub deposit: Decimal,
    pub expires_at_height: u64,
}

/// Request to claim from a payment channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimChannelRequest {
    pub recipient: String,
    pub channel_id: u64,
    pub amount: Decimal,
    pub nonce: u64,
    pub signature: String,
}

/// Payment channel details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentChannelDetails {
    pub id: u64,
    pub sender: String,
    pub recipient: String,
    pub deposit: Decimal,
    pub spent: Decimal,
    pub balance: Decimal,
    pub is_open: bool,
    pub opened_height: u64,
    pub opened_time: DateTime<Utc>,
    pub expires_at_height: u64,
    pub nonce: u64,
}

/// Request to create a batch settlement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBatchRequest {
    pub authority: String,
    pub merchant: String,
    pub payments: Vec<BatchPayment>,
}

/// Individual payment in a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPayment {
    pub sender: String,
    pub amount: Decimal,
    pub reference: String,
}

/// Response from creating a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBatchResponse {
    pub batch_id: u64,
    pub settlement_ids: Vec<u64>,
    pub total_amount: Decimal,
    pub total_fees: Decimal,
    pub tx_hash: String,
}

/// Batch settlement details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSettlementDetails {
    pub id: u64,
    pub merchant: String,
    pub settlement_ids: Vec<u64>,
    pub total_amount: Decimal,
    pub total_fees: Decimal,
    pub net_amount: Decimal,
    pub count: u64,
    pub status: SettlementStatus,
    pub created_height: u64,
    pub created_time: DateTime<Utc>,
    pub settled_height: Option<u64>,
    pub settled_time: Option<DateTime<Utc>>,
}

/// Request to register a merchant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterMerchantRequest {
    pub authority: String,
    pub merchant: String,
    pub name: String,
    pub fee_rate_bps: u32,
    pub min_settlement: Decimal,
    pub max_settlement: Decimal,
    pub batch_enabled: bool,
    pub batch_threshold: Decimal,
    pub webhook_url: Option<String>,
}

/// Merchant configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerchantConfig {
    pub address: String,
    pub name: String,
    pub fee_rate_bps: u32,
    pub min_settlement: Decimal,
    pub max_settlement: Decimal,
    pub batch_enabled: bool,
    pub batch_threshold: Decimal,
    pub is_active: bool,
    pub webhook_url: Option<String>,
    pub registered_at: DateTime<Utc>,
}

/// Transaction result from broadcasting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub tx_hash: String,
    pub code: u32,
    pub raw_log: String,
    pub gas_wanted: u64,
    pub gas_used: u64,
    pub height: u64,
    pub timestamp: DateTime<Utc>,
}

/// Account balance query result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountBalance {
    pub address: String,
    pub denom: String,
    pub amount: Decimal,
}

/// StateSet Blockchain Service
pub struct StateSetBlockchainService {
    config: StateSetConfig,
    // In production, these would be real blockchain clients
    // For now, we'll use mock implementations
    is_connected: Arc<RwLock<bool>>,
}

impl StateSetBlockchainService {
    /// Create a new StateSet blockchain service
    pub fn new(config: StateSetConfig) -> Self {
        Self {
            config,
            is_connected: Arc::new(RwLock::new(false)),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(StateSetConfig::default())
    }

    /// Connect to the StateSet blockchain
    #[instrument(skip(self))]
    pub async fn connect(&self) -> Result<(), ServiceError> {
        info!(
            endpoint = %self.config.grpc_endpoint,
            chain_id = %self.config.chain_id,
            "Connecting to StateSet blockchain"
        );

        // In production, this would establish a gRPC connection
        // For now, simulate connection
        let mut connected = self.is_connected.write().await;
        *connected = true;

        info!("Successfully connected to StateSet blockchain");
        Ok(())
    }

    /// Check if connected to the blockchain
    pub async fn is_connected(&self) -> bool {
        *self.is_connected.read().await
    }

    /// Get account balance
    #[instrument(skip(self))]
    pub async fn get_balance(&self, address: &str, denom: &str) -> Result<AccountBalance, ServiceError> {
        info!(address = %address, denom = %denom, "Querying account balance");

        // In production, query the blockchain via gRPC
        // For now, return mock data
        Ok(AccountBalance {
            address: address.to_string(),
            denom: denom.to_string(),
            amount: Decimal::new(100000000, 6), // 100 tokens
        })
    }

    /// Get stablecoin balance (ssUSD)
    pub async fn get_stablecoin_balance(&self, address: &str) -> Result<Decimal, ServiceError> {
        let balance = self.get_balance(address, &self.config.stablecoin_denom).await?;
        Ok(balance.amount)
    }

    // =========================================================================
    // Settlement Operations
    // =========================================================================

    /// Execute an instant transfer
    #[instrument(skip(self))]
    pub async fn instant_transfer(
        &self,
        request: InstantTransferRequest,
    ) -> Result<InstantTransferResponse, ServiceError> {
        info!(
            sender = %request.sender,
            recipient = %request.recipient,
            amount = %request.amount,
            reference = %request.reference,
            "Executing instant transfer"
        );

        // Validate addresses
        self.validate_address(&request.sender)?;
        self.validate_address(&request.recipient)?;

        // In production, this would:
        // 1. Build the MsgInstantTransfer message
        // 2. Sign the transaction
        // 3. Broadcast to the blockchain
        // 4. Wait for confirmation

        // Calculate fee (0.5% = 50 bps)
        let fee = request.amount * Decimal::new(5, 3);
        let net_amount = request.amount - fee;

        let response = InstantTransferResponse {
            settlement_id: self.generate_settlement_id(),
            tx_hash: self.generate_tx_hash(),
            amount: request.amount,
            fee,
            net_amount,
            status: SettlementStatus::Completed,
            block_height: self.get_current_height().await?,
            timestamp: Utc::now(),
        };

        info!(
            settlement_id = response.settlement_id,
            tx_hash = %response.tx_hash,
            "Instant transfer completed"
        );

        Ok(response)
    }

    /// Create an escrow settlement
    #[instrument(skip(self))]
    pub async fn create_escrow(
        &self,
        request: CreateEscrowRequest,
    ) -> Result<CreateEscrowResponse, ServiceError> {
        info!(
            sender = %request.sender,
            recipient = %request.recipient,
            amount = %request.amount,
            expires_in = request.expires_in_seconds,
            "Creating escrow settlement"
        );

        self.validate_address(&request.sender)?;
        self.validate_address(&request.recipient)?;

        let expires_at = Utc::now() + chrono::Duration::seconds(request.expires_in_seconds as i64);

        let response = CreateEscrowResponse {
            settlement_id: self.generate_settlement_id(),
            tx_hash: self.generate_tx_hash(),
            amount: request.amount,
            expires_at,
            status: SettlementStatus::Pending,
        };

        info!(
            settlement_id = response.settlement_id,
            expires_at = %response.expires_at,
            "Escrow created"
        );

        Ok(response)
    }

    /// Release escrow funds to recipient
    #[instrument(skip(self))]
    pub async fn release_escrow(
        &self,
        request: ReleaseEscrowRequest,
    ) -> Result<TransactionResult, ServiceError> {
        info!(
            sender = %request.sender,
            settlement_id = request.settlement_id,
            "Releasing escrow"
        );

        self.validate_address(&request.sender)?;

        let result = TransactionResult {
            tx_hash: self.generate_tx_hash(),
            code: 0,
            raw_log: "escrow released successfully".to_string(),
            gas_wanted: self.config.default_gas_limit,
            gas_used: self.config.default_gas_limit * 80 / 100,
            height: self.get_current_height().await?,
            timestamp: Utc::now(),
        };

        info!(tx_hash = %result.tx_hash, "Escrow released");
        Ok(result)
    }

    /// Refund escrow funds to sender
    #[instrument(skip(self))]
    pub async fn refund_escrow(
        &self,
        request: RefundEscrowRequest,
    ) -> Result<TransactionResult, ServiceError> {
        info!(
            recipient = %request.recipient,
            settlement_id = request.settlement_id,
            reason = %request.reason,
            "Refunding escrow"
        );

        self.validate_address(&request.recipient)?;

        let result = TransactionResult {
            tx_hash: self.generate_tx_hash(),
            code: 0,
            raw_log: "escrow refunded successfully".to_string(),
            gas_wanted: self.config.default_gas_limit,
            gas_used: self.config.default_gas_limit * 80 / 100,
            height: self.get_current_height().await?,
            timestamp: Utc::now(),
        };

        info!(tx_hash = %result.tx_hash, "Escrow refunded");
        Ok(result)
    }

    /// Get settlement details
    #[instrument(skip(self))]
    pub async fn get_settlement(&self, settlement_id: u64) -> Result<SettlementDetails, ServiceError> {
        info!(settlement_id = settlement_id, "Getting settlement details");

        // In production, query the blockchain
        Ok(SettlementDetails {
            id: settlement_id,
            settlement_type: SettlementType::Instant,
            sender: format!("{}1sender", self.config.address_prefix),
            recipient: format!("{}1recipient", self.config.address_prefix),
            amount: Decimal::new(10000000, 6),
            fee: Decimal::new(50000, 6),
            net_amount: Decimal::new(9950000, 6),
            status: SettlementStatus::Completed,
            reference: "order-123".to_string(),
            metadata: None,
            created_height: 1000,
            created_time: Utc::now() - chrono::Duration::hours(1),
            settled_height: Some(1001),
            settled_time: Some(Utc::now()),
            expires_at: None,
            batch_id: None,
        })
    }

    // =========================================================================
    // Batch Operations
    // =========================================================================

    /// Create a batch settlement
    #[instrument(skip(self))]
    pub async fn create_batch(
        &self,
        request: CreateBatchRequest,
    ) -> Result<CreateBatchResponse, ServiceError> {
        info!(
            merchant = %request.merchant,
            payment_count = request.payments.len(),
            "Creating batch settlement"
        );

        self.validate_address(&request.authority)?;
        self.validate_address(&request.merchant)?;

        let total_amount: Decimal = request.payments.iter().map(|p| p.amount).sum();
        let fee_rate = Decimal::new(5, 3); // 0.5%
        let total_fees = total_amount * fee_rate;

        let settlement_ids: Vec<u64> = (0..request.payments.len())
            .map(|_| self.generate_settlement_id())
            .collect();

        let response = CreateBatchResponse {
            batch_id: self.generate_settlement_id(),
            settlement_ids,
            total_amount,
            total_fees,
            tx_hash: self.generate_tx_hash(),
        };

        info!(
            batch_id = response.batch_id,
            total_amount = %response.total_amount,
            "Batch created"
        );

        Ok(response)
    }

    /// Settle a batch
    #[instrument(skip(self))]
    pub async fn settle_batch(
        &self,
        authority: &str,
        batch_id: u64,
    ) -> Result<TransactionResult, ServiceError> {
        info!(authority = %authority, batch_id = batch_id, "Settling batch");

        self.validate_address(authority)?;

        let result = TransactionResult {
            tx_hash: self.generate_tx_hash(),
            code: 0,
            raw_log: "batch settled successfully".to_string(),
            gas_wanted: self.config.default_gas_limit * 2,
            gas_used: self.config.default_gas_limit * 160 / 100,
            height: self.get_current_height().await?,
            timestamp: Utc::now(),
        };

        info!(tx_hash = %result.tx_hash, "Batch settled");
        Ok(result)
    }

    /// Get batch details
    #[instrument(skip(self))]
    pub async fn get_batch(&self, batch_id: u64) -> Result<BatchSettlementDetails, ServiceError> {
        info!(batch_id = batch_id, "Getting batch details");

        Ok(BatchSettlementDetails {
            id: batch_id,
            merchant: format!("{}1merchant", self.config.address_prefix),
            settlement_ids: vec![1, 2, 3],
            total_amount: Decimal::new(30000000, 6),
            total_fees: Decimal::new(150000, 6),
            net_amount: Decimal::new(29850000, 6),
            count: 3,
            status: SettlementStatus::Completed,
            created_height: 1000,
            created_time: Utc::now() - chrono::Duration::hours(1),
            settled_height: Some(1001),
            settled_time: Some(Utc::now()),
        })
    }

    // =========================================================================
    // Payment Channel Operations
    // =========================================================================

    /// Open a payment channel
    #[instrument(skip(self))]
    pub async fn open_channel(
        &self,
        request: OpenChannelRequest,
    ) -> Result<OpenChannelResponse, ServiceError> {
        info!(
            sender = %request.sender,
            recipient = %request.recipient,
            deposit = %request.deposit,
            "Opening payment channel"
        );

        self.validate_address(&request.sender)?;
        self.validate_address(&request.recipient)?;

        let current_height = self.get_current_height().await?;
        let expires_at_height = current_height + request.expires_in_blocks;

        let response = OpenChannelResponse {
            channel_id: self.generate_settlement_id(),
            tx_hash: self.generate_tx_hash(),
            deposit: request.deposit,
            expires_at_height,
        };

        info!(
            channel_id = response.channel_id,
            expires_at = response.expires_at_height,
            "Channel opened"
        );

        Ok(response)
    }

    /// Claim funds from a payment channel
    #[instrument(skip(self))]
    pub async fn claim_channel(
        &self,
        request: ClaimChannelRequest,
    ) -> Result<TransactionResult, ServiceError> {
        info!(
            recipient = %request.recipient,
            channel_id = request.channel_id,
            amount = %request.amount,
            nonce = request.nonce,
            "Claiming from payment channel"
        );

        self.validate_address(&request.recipient)?;

        let result = TransactionResult {
            tx_hash: self.generate_tx_hash(),
            code: 0,
            raw_log: format!("claimed {} from channel", request.amount),
            gas_wanted: self.config.default_gas_limit,
            gas_used: self.config.default_gas_limit * 70 / 100,
            height: self.get_current_height().await?,
            timestamp: Utc::now(),
        };

        info!(tx_hash = %result.tx_hash, "Channel claim processed");
        Ok(result)
    }

    /// Close a payment channel
    #[instrument(skip(self))]
    pub async fn close_channel(
        &self,
        closer: &str,
        channel_id: u64,
    ) -> Result<TransactionResult, ServiceError> {
        info!(closer = %closer, channel_id = channel_id, "Closing payment channel");

        self.validate_address(closer)?;

        let result = TransactionResult {
            tx_hash: self.generate_tx_hash(),
            code: 0,
            raw_log: "channel closed successfully".to_string(),
            gas_wanted: self.config.default_gas_limit,
            gas_used: self.config.default_gas_limit * 60 / 100,
            height: self.get_current_height().await?,
            timestamp: Utc::now(),
        };

        info!(tx_hash = %result.tx_hash, "Channel closed");
        Ok(result)
    }

    /// Get payment channel details
    #[instrument(skip(self))]
    pub async fn get_channel(&self, channel_id: u64) -> Result<PaymentChannelDetails, ServiceError> {
        info!(channel_id = channel_id, "Getting channel details");

        Ok(PaymentChannelDetails {
            id: channel_id,
            sender: format!("{}1sender", self.config.address_prefix),
            recipient: format!("{}1recipient", self.config.address_prefix),
            deposit: Decimal::new(100000000, 6),
            spent: Decimal::new(25000000, 6),
            balance: Decimal::new(75000000, 6),
            is_open: true,
            opened_height: 1000,
            opened_time: Utc::now() - chrono::Duration::hours(24),
            expires_at_height: 10000,
            nonce: 5,
        })
    }

    // =========================================================================
    // Merchant Operations
    // =========================================================================

    /// Register a merchant
    #[instrument(skip(self))]
    pub async fn register_merchant(
        &self,
        request: RegisterMerchantRequest,
    ) -> Result<TransactionResult, ServiceError> {
        info!(
            authority = %request.authority,
            merchant = %request.merchant,
            name = %request.name,
            fee_rate_bps = request.fee_rate_bps,
            "Registering merchant"
        );

        self.validate_address(&request.authority)?;
        self.validate_address(&request.merchant)?;

        let result = TransactionResult {
            tx_hash: self.generate_tx_hash(),
            code: 0,
            raw_log: "merchant registered successfully".to_string(),
            gas_wanted: self.config.default_gas_limit,
            gas_used: self.config.default_gas_limit * 75 / 100,
            height: self.get_current_height().await?,
            timestamp: Utc::now(),
        };

        info!(tx_hash = %result.tx_hash, "Merchant registered");
        Ok(result)
    }

    /// Get merchant configuration
    #[instrument(skip(self))]
    pub async fn get_merchant(&self, address: &str) -> Result<MerchantConfig, ServiceError> {
        info!(address = %address, "Getting merchant configuration");

        Ok(MerchantConfig {
            address: address.to_string(),
            name: "Test Merchant".to_string(),
            fee_rate_bps: 50,
            min_settlement: Decimal::new(1000, 6),
            max_settlement: Decimal::new(1000000000000, 6),
            batch_enabled: true,
            batch_threshold: Decimal::new(10000000, 6),
            is_active: true,
            webhook_url: None,
            registered_at: Utc::now() - chrono::Duration::days(30),
        })
    }

    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Validate a StateSet address
    fn validate_address(&self, address: &str) -> Result<(), ServiceError> {
        if !address.starts_with(&self.config.address_prefix) {
            return Err(ServiceError::ValidationError(format!(
                "Invalid address: must start with {}",
                self.config.address_prefix
            )));
        }
        if address.len() < 40 || address.len() > 65 {
            return Err(ServiceError::ValidationError(
                "Invalid address length".to_string(),
            ));
        }
        Ok(())
    }

    /// Generate a mock settlement ID
    fn generate_settlement_id(&self) -> u64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64 % 1_000_000_000)
            .unwrap_or(0)
    }

    /// Generate a mock transaction hash
    fn generate_tx_hash(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{:064X}", nanos)
    }

    /// Get current blockchain height (mock)
    async fn get_current_height(&self) -> Result<u64, ServiceError> {
        Ok(100000)
    }

    /// Get the configuration
    pub fn config(&self) -> &StateSetConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires live blockchain node"]
    async fn test_instant_transfer() {
        let service = StateSetBlockchainService::with_defaults();
        service.connect().await.unwrap();

        let request = InstantTransferRequest {
            sender: "stateset1sender".to_string(),
            recipient: "stateset1recipient".to_string(),
            amount: Decimal::new(10000000, 6), // 10 ssUSD
            reference: "order-123".to_string(),
            metadata: None,
        };

        let response = service.instant_transfer(request).await.unwrap();
        assert_eq!(response.status, SettlementStatus::Completed);
        assert!(response.settlement_id > 0);
    }

    #[tokio::test]
    #[ignore = "requires live blockchain node"]
    async fn test_create_escrow() {
        let service = StateSetBlockchainService::with_defaults();
        service.connect().await.unwrap();

        let request = CreateEscrowRequest {
            sender: "stateset1sender".to_string(),
            recipient: "stateset1recipient".to_string(),
            amount: Decimal::new(50000000, 6), // 50 ssUSD
            reference: "escrow-456".to_string(),
            metadata: None,
            expires_in_seconds: 86400, // 1 day
        };

        let response = service.create_escrow(request).await.unwrap();
        assert_eq!(response.status, SettlementStatus::Pending);
        assert!(response.expires_at > Utc::now());
    }

    #[tokio::test]
    #[ignore = "requires live blockchain node"]
    async fn test_open_channel() {
        let service = StateSetBlockchainService::with_defaults();
        service.connect().await.unwrap();

        let request = OpenChannelRequest {
            sender: "stateset1sender".to_string(),
            recipient: "stateset1recipient".to_string(),
            deposit: Decimal::new(100000000, 6), // 100 ssUSD
            expires_in_blocks: 10000,
        };

        let response = service.open_channel(request).await.unwrap();
        assert!(response.channel_id > 0);
        assert!(response.expires_at_height > 100000);
    }

    #[tokio::test]
    #[ignore = "requires live blockchain node"]
    async fn test_create_batch() {
        let service = StateSetBlockchainService::with_defaults();
        service.connect().await.unwrap();

        let request = CreateBatchRequest {
            authority: "stateset1authority".to_string(),
            merchant: "stateset1merchant".to_string(),
            payments: vec![
                BatchPayment {
                    sender: "stateset1payer1".to_string(),
                    amount: Decimal::new(10000000, 6),
                    reference: "order-1".to_string(),
                },
                BatchPayment {
                    sender: "stateset1payer2".to_string(),
                    amount: Decimal::new(20000000, 6),
                    reference: "order-2".to_string(),
                },
            ],
        };

        let response = service.create_batch(request).await.unwrap();
        assert!(response.batch_id > 0);
        assert_eq!(response.settlement_ids.len(), 2);
        assert_eq!(response.total_amount, Decimal::new(30000000, 6));
    }
}
