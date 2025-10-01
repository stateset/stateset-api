use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Request types

#[derive(Debug, Deserialize)]
pub struct DelegatePaymentRequest {
    pub payment_method: PaymentMethod,
    pub allowance: Allowance,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_address: Option<BillingAddress>,
    pub risk_signals: Vec<RiskSignal>,
    pub metadata: serde_json::Value,
}

// Response types

#[derive(Debug, Serialize)]
pub struct DelegatePaymentResponse {
    pub id: String,
    pub created: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct DelegatedPaymentError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
}

// HTTP API request/response types for additional PSP endpoints
#[derive(Debug, Deserialize)]
pub struct ValidateTokenApiRequest {
    pub token: String,
    pub amount: i64,
    pub checkout_session_id: String,
}

#[derive(Debug, Serialize)]
pub struct ValidateTokenApiResponse {
    pub valid: bool,
    pub token: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ConsumeTokenApiRequest {
    pub token: String,
}

#[derive(Debug, Serialize)]
pub struct ConsumeTokenApiResponse {
    pub consumed: bool,
}

// Data models

#[derive(Debug, Deserialize)]
pub struct PaymentMethod {
    #[serde(rename = "type")]
    pub payment_type: String, // "card"
    pub card_number_type: String, // "fpan" or "network_token"
    pub number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp_month: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp_year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cvc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cryptogram: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks_performed: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iin: Option<String>,
    pub display_card_funding_type: String, // "credit", "debit", "prepaid"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_wallet_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_last4: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BillingAddress {
    pub name: String,
    pub line_one: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_two: Option<String>,
    pub city: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    pub country: String,
    pub postal_code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Allowance {
    pub reason: String, // "one_time"
    pub max_amount: i64,
    pub currency: String,
    pub checkout_session_id: String,
    pub merchant_id: String,
    pub expires_at: String,
}

#[derive(Debug, Deserialize)]
pub struct RiskSignal {
    #[serde(rename = "type")]
    pub signal_type: String,
    pub score: i32,
    pub action: String, // "blocked", "manual_review", "authorized"
}

// Service

use crate::cache::InMemoryCache;
use crate::errors::ServiceError;
use chrono::Datelike;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct DelegatedPaymentService {
    cache: Arc<InMemoryCache>,
}

impl DelegatedPaymentService {
    pub fn new(cache: Arc<InMemoryCache>) -> Self {
        Self { cache }
    }

    /// Process a delegated payment request and return a vault token
    #[instrument(skip(self, request))]
    pub async fn delegate_payment(
        &self,
        request: DelegatePaymentRequest,
    ) -> Result<DelegatePaymentResponse, ServiceError> {
        // Validate payment method
        if request.payment_method.payment_type != "card" {
            return Err(ServiceError::InvalidInput(
                "Only card payment methods are supported".to_string()
            ));
        }

        // Validate card number type
        if request.payment_method.card_number_type != "fpan" 
            && request.payment_method.card_number_type != "network_token" {
            return Err(ServiceError::InvalidInput(
                "card_number_type must be 'fpan' or 'network_token'".to_string()
            ));
        }

        // Validate allowance reason
        if request.allowance.reason != "one_time" {
            return Err(ServiceError::InvalidInput(
                "Only 'one_time' allowance reason is supported".to_string()
            ));
        }

        // Check risk signals for blocks
        for signal in &request.risk_signals {
            if signal.action == "blocked" {
                return Err(ServiceError::InvalidOperation(
                    format!("Payment blocked due to risk signal: {}", signal.signal_type)
                ));
            }
        }

        // Perform basic card validation
        self.validate_card(&request.payment_method)?;

        // Generate vault token
        let vault_token_id = format!("vt_{}", Uuid::new_v4());
        let created = chrono::Utc::now().to_rfc3339();

        // Store the delegated payment data (in production, this would be in a secure vault)
        let cache_key = format!("vault_token:{}", vault_token_id);
        let token_data = serde_json::json!({
            "vault_token_id": vault_token_id,
            "payment_method": {
                "type": request.payment_method.payment_type,
                "last4": request.payment_method.display_last4,
                "brand": request.payment_method.display_brand,
                "funding_type": request.payment_method.display_card_funding_type,
            },
            "allowance": request.allowance,
            "billing_address": request.billing_address,
            "created": created,
            "metadata": request.metadata,
        });

        let ttl = self.calculate_ttl(&request.allowance.expires_at)?;
        
        self.cache
            .set(&cache_key, &token_data.to_string(), Some(ttl))
            .await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        info!("Created vault token: {}", vault_token_id);

        Ok(DelegatePaymentResponse {
            id: vault_token_id,
            created,
            metadata: request.metadata,
        })
    }

    /// Validate a vault token and return payment details
    pub async fn validate_token(
        &self,
        token: &str,
        amount: i64,
        checkout_session_id: &str,
    ) -> Result<serde_json::Value, ServiceError> {
        let cache_key = format!("vault_token:{}", token);
        
        let cached = self.cache.get(&cache_key).await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        match cached {
            Some(data) => {
                let token_data: serde_json::Value = serde_json::from_str(&data)
                    .map_err(|e| ServiceError::ParseError(e.to_string()))?;

                // Validate allowance
                if let Some(allowance) = token_data.get("allowance") {
                    // Check checkout session ID
                    if allowance.get("checkout_session_id").and_then(|v| v.as_str()) 
                        != Some(checkout_session_id) {
                        return Err(ServiceError::InvalidOperation(
                            "Token is not valid for this checkout session".to_string()
                        ));
                    }

                    // Check max amount
                    if let Some(max_amount) = allowance.get("max_amount").and_then(|v| v.as_i64()) {
                        if amount > max_amount {
                            return Err(ServiceError::InvalidOperation(
                                format!("Amount {} exceeds max allowance {}", amount, max_amount)
                            ));
                        }
                    }

                    // Check expiry (expires_at is already enforced by cache TTL)
                }

                Ok(token_data)
            },
            None => Err(ServiceError::NotFound(
                format!("Vault token {} not found or expired", token)
            )),
        }
    }

    /// Mark a token as used (single-use enforcement)
    pub async fn consume_token(&self, token: &str) -> Result<(), ServiceError> {
        let cache_key = format!("vault_token:{}", token);
        
        self.cache.delete(&cache_key).await
            .map_err(|e| ServiceError::CacheError(e.to_string()))?;

        info!("Consumed vault token: {}", token);
        Ok(())
    }

    // Private helper methods

    fn validate_card(&self, payment_method: &PaymentMethod) -> Result<(), ServiceError> {
        // Basic card number validation (length check)
        let number = payment_method.number.replace(&[' ', '-'][..], "");
        
        if number.len() < 13 || number.len() > 19 {
            return Err(ServiceError::InvalidInput(
                "Invalid card number length".to_string()
            ));
        }

        // Validate expiry if provided
        if let (Some(month), Some(year)) = (&payment_method.exp_month, &payment_method.exp_year) {
            if let (Ok(m), Ok(y)) = (month.parse::<u32>(), year.parse::<u32>()) {
                if m < 1 || m > 12 {
                    return Err(ServiceError::InvalidInput(
                        "Invalid expiry month".to_string()
                    ));
                }
                
                let now = chrono::Utc::now();
                let current_year = now.year() as u32;
                let current_month = now.month();
                
                if y < current_year || (y == current_year && m < current_month) {
                    return Err(ServiceError::InvalidInput(
                        "Card is expired".to_string()
                    ));
                }
            }
        }

        Ok(())
    }

    fn calculate_ttl(&self, expires_at: &str) -> Result<Duration, ServiceError> {
        let expiry = chrono::DateTime::parse_from_rfc3339(expires_at)
            .map_err(|e| ServiceError::ParseError(format!("Invalid expires_at format: {}", e)))?;
        
        let now = chrono::Utc::now();
        let duration = expiry.signed_duration_since(now);
        
        if duration.num_seconds() <= 0 {
            return Err(ServiceError::InvalidInput(
                "expires_at must be in the future".to_string()
            ));
        }

        Ok(Duration::from_secs(duration.num_seconds() as u64))
    }
} 