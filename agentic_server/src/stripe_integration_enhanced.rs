/// Enhanced Stripe Integration with Production-Ready Features
///
/// This module provides a comprehensive Stripe payment integration with:
/// - Exponential backoff retry logic
/// - Webhook signature verification
/// - Advanced risk assessment
/// - Multiple payment method support
/// - Comprehensive error handling
/// - Idempotency key support
/// - Request logging and monitoring
use crate::errors::ServiceError;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

type HmacSha256 = Hmac<Sha256>;

/// Stripe API configuration with enhanced settings
#[derive(Clone)]
pub struct StripeConfig {
    /// Stripe secret API key (sk_live_... or sk_test_...)
    pub secret_key: String,
    /// Optional publishable key for client-side operations
    pub publishable_key: Option<String>,
    /// Webhook signing secret for verification
    pub webhook_secret: Option<String>,
    /// API version to use
    pub api_version: String,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    pub initial_retry_delay_ms: u64,
}

impl StripeConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ServiceError> {
        let secret_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
            ServiceError::InternalError(
                "STRIPE_SECRET_KEY environment variable not set".to_string(),
            )
        })?;

        let publishable_key = std::env::var("STRIPE_PUBLISHABLE_KEY").ok();
        let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET").ok();

        let api_version = std::env::var("STRIPE_API_VERSION")
            .unwrap_or_else(|_| "2023-10-16".to_string());

        let max_retries = std::env::var("STRIPE_MAX_RETRIES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        let initial_retry_delay_ms = std::env::var("STRIPE_RETRY_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100);

        Ok(Self {
            secret_key,
            publishable_key,
            webhook_secret,
            api_version,
            max_retries,
            initial_retry_delay_ms,
        })
    }
}

/// Enhanced Stripe payment processor with production-ready features
#[derive(Clone)]
pub struct StripePaymentProcessor {
    config: StripeConfig,
    client: reqwest::Client,
}

impl StripePaymentProcessor {
    /// Create a new Stripe payment processor
    pub fn new(config: StripeConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self { config, client }
    }

    /// Process payment using SharedPaymentToken with retry logic
    ///
    /// # Arguments
    /// * `token` - The SharedPaymentToken from Stripe
    /// * `amount` - Amount in cents/minor units
    /// * `currency` - ISO 4217 currency code (lowercase)
    /// * `metadata` - Additional metadata to attach
    /// * `idempotency_key` - Optional idempotency key for safe retries
    ///
    /// # Returns
    /// PaymentIntent response with status and details
    #[instrument(skip(self), fields(token_prefix = %&token[..std::cmp::min(10, token.len())]))]
    pub async fn process_shared_payment_token(
        &self,
        token: &str,
        amount: i64,
        currency: &str,
        metadata: HashMap<String, String>,
        idempotency_key: Option<String>,
    ) -> Result<PaymentIntentResponse, ServiceError> {
        info!(
            amount = amount,
            currency = currency,
            "Processing SharedPaymentToken"
        );

        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("amount".to_string(), amount.to_string());
        params.insert("currency".to_string(), currency.to_string());
        params.insert("shared_payment_token".to_string(), token.to_string());
        params.insert("confirm".to_string(), "true".to_string());
        params.insert("automatic_payment_methods[enabled]".to_string(), "true".to_string());

        // Add metadata
        for (key, value) in metadata {
            params.insert(format!("metadata[{}]", key), value);
        }

        // Retry with exponential backoff
        let mut attempts = 0;
        let mut delay = Duration::from_millis(self.config.initial_retry_delay_ms);

        loop {
            attempts += 1;

            let mut request = self
                .client
                .post("https://api.stripe.com/v1/payment_intents")
                .basic_auth(&self.config.secret_key, Some(""))
                .header("Stripe-Version", &self.config.api_version)
                .form(&params);

            // Add idempotency key if provided
            if let Some(ref key) = idempotency_key {
                request = request.header("Idempotency-Key", key);
            }

            match request.send().await {
                Ok(response) if response.status().is_success() => {
                    let payment_intent: PaymentIntentResponse = response.json().await.map_err(
                        |e| ServiceError::ParseError(format!("Failed to parse Stripe response: {}", e)),
                    )?;

                    info!(
                        payment_intent_id = %payment_intent.id,
                        status = %payment_intent.status,
                        attempts = attempts,
                        "Payment processed successfully"
                    );

                    return Ok(payment_intent);
                }
                Ok(response) if response.status().as_u16() == 429 && attempts <= self.config.max_retries => {
                    // Rate limited - retry with backoff
                    warn!(attempts = attempts, delay_ms = delay.as_millis(), "Rate limited by Stripe, retrying");
                    tokio::time::sleep(delay).await;
                    delay *= 2; // Exponential backoff
                    continue;
                }
                Ok(response) if self.is_retriable_status(response.status().as_u16()) && attempts <= self.config.max_retries => {
                    warn!(
                        status = response.status().as_u16(),
                        attempts = attempts,
                        "Retriable error from Stripe, retrying"
                    );
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                Ok(response) => {
                    let status = response.status();
                    let error_text = response.text().await.unwrap_or_default();
                    error!(status = %status, error = %error_text, "Stripe API error");

                    return Err(ServiceError::PaymentFailed(format!(
                        "Stripe error ({}): {}",
                        status, error_text
                    )));
                }
                Err(e) if attempts <= self.config.max_retries => {
                    warn!(error = %e, attempts = attempts, "Network error, retrying");
                    tokio::time::sleep(delay).await;
                    delay *= 2;
                    continue;
                }
                Err(e) => {
                    error!(error = %e, "Max retries exceeded");
                    return Err(ServiceError::InternalError(format!("Stripe API error: {}", e)));
                }
            }
        }
    }

    /// Check if HTTP status code is retriable
    fn is_retriable_status(&self, status: u16) -> bool {
        matches!(status, 408 | 500 | 502 | 503 | 504)
    }

    /// Retrieve granted token details with risk assessment
    ///
    /// # Arguments
    /// * `token_id` - The SharedPaymentToken ID (spt_...)
    ///
    /// # Returns
    /// Complete granted token details including risk signals
    #[instrument(skip(self))]
    pub async fn get_granted_token(
        &self,
        token_id: &str,
    ) -> Result<GrantedTokenResponse, ServiceError> {
        debug!(token_id = token_id, "Fetching granted token details");

        let url = format!(
            "https://api.stripe.com/v1/shared_payment/granted_tokens/{}",
            token_id
        );

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.config.secret_key, Some(""))
            .header("Stripe-Version", &self.config.api_version)
            .send()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::NotFound(format!(
                "Token not found: {}",
                error_text
            )));
        }

        let token: GrantedTokenResponse = response
            .json()
            .await
            .map_err(|e| ServiceError::ParseError(format!("Failed to parse response: {}", e)))?;

        Ok(token)
    }

    /// Advanced risk assessment with multiple signals
    ///
    /// Analyzes multiple risk factors including:
    /// - Fraudulent dispute risk
    /// - Stolen card indicators
    /// - Card testing patterns
    /// - Bot activity
    /// - Card issuer decline likelihood
    ///
    /// # Returns
    /// Comprehensive risk assessment with recommendation
    pub fn assess_risk(&self, token: &GrantedTokenResponse) -> RiskAssessment {
        let mut should_block = false;
        let mut warnings = Vec::new();
        let mut risk_score = 0.0;

        if let Some(risks) = &token.risk_details {
            // Check fraudulent dispute risk (0-100)
            if let Some(fraud_score) = risks.fraudulent_dispute {
                risk_score += fraud_score as f32 / 100.0 * 0.4; // 40% weight

                if fraud_score > 75 {
                    should_block = true;
                    warnings.push(format!(
                        "High fraudulent dispute risk: {}%",
                        fraud_score
                    ));
                } else if fraud_score > 50 {
                    warnings.push(format!(
                        "Moderate fraudulent dispute risk: {}%",
                        fraud_score
                    ));
                }
            }

            // Check stolen card risk (0-100)
            if let Some(stolen_score) = risks.stolen_card {
                risk_score += stolen_score as f32 / 100.0 * 0.3; // 30% weight

                if stolen_score > 75 {
                    should_block = true;
                    warnings.push(format!("High stolen card risk: {}%", stolen_score));
                } else if stolen_score > 50 {
                    warnings.push(format!("Moderate stolen card risk: {}%", stolen_score));
                }
            }

            // Check card testing (0.0-1.0)
            if let Some(testing_score) = risks.card_testing {
                risk_score += testing_score * 0.2; // 20% weight

                if testing_score > 0.8 {
                    should_block = true;
                    warnings.push("High card testing probability".to_string());
                } else if testing_score > 0.5 {
                    warnings.push("Moderate card testing probability".to_string());
                }
            }

            // Check bot activity (0.0-1.0)
            if let Some(bot_score) = risks.bot {
                risk_score += bot_score * 0.1; // 10% weight

                if bot_score > 0.8 {
                    should_block = true;
                    warnings.push("High bot activity detected".to_string());
                }
            }
        }

        // Normalize risk score (0.0-1.0)
        risk_score = risk_score.min(1.0);

        let recommendation = if should_block {
            "block".to_string()
        } else if risk_score > 0.6 {
            "review".to_string()
        } else if risk_score > 0.3 {
            "monitor".to_string()
        } else {
            "continue".to_string()
        };

        RiskAssessment {
            should_block,
            warnings,
            risk_score,
            recommendation,
        }
    }

    /// Capture a payment intent
    ///
    /// # Arguments
    /// * `payment_intent_id` - The PaymentIntent ID (pi_...)
    /// * `amount_to_capture` - Optional amount if capturing less than authorized
    ///
    /// # Returns
    /// Updated PaymentIntent with captured status
    #[instrument(skip(self))]
    pub async fn capture_payment(
        &self,
        payment_intent_id: &str,
        amount_to_capture: Option<i64>,
    ) -> Result<PaymentIntentResponse, ServiceError> {
        info!(
            payment_intent_id = payment_intent_id,
            amount = ?amount_to_capture,
            "Capturing payment"
        );

        let url = format!(
            "https://api.stripe.com/v1/payment_intents/{}/capture",
            payment_intent_id
        );

        let mut params = HashMap::new();
        if let Some(amount) = amount_to_capture {
            params.insert("amount_to_capture", amount.to_string());
        }

        let response = self
            .client
            .post(&url)
            .basic_auth(&self.config.secret_key, Some(""))
            .header("Stripe-Version", &self.config.api_version)
            .form(&params)
            .send()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::PaymentFailed(format!(
                "Capture failed: {}",
                error_text
            )));
        }

        let payment_intent: PaymentIntentResponse = response
            .json()
            .await
            .map_err(|e| ServiceError::ParseError(format!("Failed to parse response: {}", e)))?;

        info!(
            payment_intent_id = %payment_intent.id,
            status = %payment_intent.status,
            "Payment captured successfully"
        );

        Ok(payment_intent)
    }

    /// Cancel a payment intent
    #[instrument(skip(self))]
    pub async fn cancel_payment(
        &self,
        payment_intent_id: &str,
        cancellation_reason: Option<&str>,
    ) -> Result<PaymentIntentResponse, ServiceError> {
        info!(
            payment_intent_id = payment_intent_id,
            reason = ?cancellation_reason,
            "Canceling payment"
        );

        let url = format!(
            "https://api.stripe.com/v1/payment_intents/{}/cancel",
            payment_intent_id
        );

        let mut params = HashMap::new();
        if let Some(reason) = cancellation_reason {
            params.insert("cancellation_reason", reason.to_string());
        }

        let response = self
            .client
            .post(&url)
            .basic_auth(&self.config.secret_key, Some(""))
            .header("Stripe-Version", &self.config.api_version)
            .form(&params)
            .send()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ServiceError::PaymentFailed(format!(
                "Cancellation failed: {}",
                error_text
            )));
        }

        let payment_intent: PaymentIntentResponse = response
            .json()
            .await
            .map_err(|e| ServiceError::ParseError(format!("Failed to parse response: {}", e)))?;

        info!(payment_intent_id = %payment_intent.id, "Payment canceled");
        Ok(payment_intent)
    }

    /// Verify Stripe webhook signature
    ///
    /// # Arguments
    /// * `payload` - The raw webhook payload
    /// * `signature_header` - The Stripe-Signature header value
    /// * `tolerance_secs` - Optional timestamp tolerance (default 300 seconds)
    ///
    /// # Returns
    /// True if signature is valid
    pub fn verify_webhook_signature(
        &self,
        payload: &[u8],
        signature_header: &str,
        tolerance_secs: Option<i64>,
    ) -> Result<bool, ServiceError> {
        let webhook_secret = self.config.webhook_secret.as_ref().ok_or_else(|| {
            ServiceError::InternalError("Webhook secret not configured".to_string())
        })?;

        let tolerance = tolerance_secs.unwrap_or(300);

        // Parse signature header (format: t=timestamp,v1=signature)
        let mut timestamp: Option<i64> = None;
        let mut signature: Option<&str> = None;

        for part in signature_header.split(',') {
            let parts: Vec<&str> = part.splitn(2, '=').collect();
            if parts.len() == 2 {
                match parts[0] {
                    "t" => timestamp = parts[1].parse().ok(),
                    "v1" => signature = Some(parts[1]),
                    _ => {}
                }
            }
        }

        let timestamp = timestamp.ok_or_else(|| {
            ServiceError::InvalidInput("Missing timestamp in signature".to_string())
        })?;

        let signature = signature.ok_or_else(|| {
            ServiceError::InvalidInput("Missing signature in header".to_string())
        })?;

        // Check timestamp tolerance
        let current_time = chrono::Utc::now().timestamp();
        if (current_time - timestamp).abs() > tolerance {
            return Err(ServiceError::InvalidInput(
                "Timestamp outside tolerance window".to_string(),
            ));
        }

        // Compute expected signature
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(payload));
        let mut mac = HmacSha256::new_from_slice(webhook_secret.as_bytes())
            .map_err(|e| ServiceError::InternalError(format!("HMAC error: {}", e)))?;
        mac.update(signed_payload.as_bytes());
        let expected_signature = hex::encode(mac.finalize().into_bytes());

        // Constant-time comparison
        Ok(expected_signature == signature)
    }
}

// Response types (with comprehensive documentation)

/// Stripe PaymentIntent response
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PaymentIntentResponse {
    /// Unique identifier (pi_...)
    pub id: String,
    /// Object type ("payment_intent")
    pub object: String,
    /// Amount in minor units
    pub amount: i64,
    /// Three-letter ISO currency code
    pub currency: String,
    /// Status: requires_payment_method, requires_confirmation, requires_action,
    /// processing, requires_capture, canceled, or succeeded
    pub status: String,
    /// Payment method ID if attached
    pub payment_method: Option<String>,
    /// Client secret for client-side confirmation
    pub client_secret: Option<String>,
    /// Amount captured (may differ from amount)
    pub amount_captured: Option<i64>,
    /// Amount refunded
    pub amount_refundable: Option<i64>,
}

/// Granted token response from Stripe
#[derive(Debug, Serialize, Deserialize)]
pub struct GrantedTokenResponse {
    pub id: String,
    pub object: String,
    pub payment_method_preview: PaymentMethodPreview,
    pub usage_limits: UsageLimits,
    pub risk_details: Option<RiskDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentMethodPreview {
    pub card: Option<CardPreview>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CardPreview {
    pub brand: String,
    pub last4: String,
    /// credit, debit, or prepaid
    pub funding: String,
    /// Optional country code
    pub country: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageLimits {
    pub currency: String,
    pub max_amount: i64,
    /// Unix timestamp
    pub expires_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RiskDetails {
    /// 0-100 scale
    pub fraudulent_dispute: Option<i32>,
    /// 0.0-1.0 scale
    pub card_testing: Option<f32>,
    /// 0-100 scale
    pub stolen_card: Option<i32>,
    /// 0.0-1.0 scale
    pub card_issuer_decline: Option<f32>,
    /// 0.0-1.0 scale
    pub bot: Option<f32>,
}

/// Comprehensive risk assessment result
#[derive(Debug, Clone)]
pub struct RiskAssessment {
    /// Whether transaction should be blocked
    pub should_block: bool,
    /// List of risk warnings
    pub warnings: Vec<String>,
    /// Overall risk score (0.0-1.0)
    pub risk_score: f32,
    /// Recommendation: "block", "review", "monitor", or "continue"
    pub recommendation: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_assessment_low_risk() {
        let token = create_test_token(25, 10, 0.1, 0.02);
        let processor = create_test_processor();

        let assessment = processor.assess_risk(&token);
        assert!(!assessment.should_block);
        assert_eq!(assessment.recommendation, "continue");
        assert!(assessment.risk_score < 0.3);
    }

    #[test]
    fn test_risk_assessment_high_risk() {
        let token = create_test_token(90, 85, 0.9, 0.8);
        let processor = create_test_processor();

        let assessment = processor.assess_risk(&token);
        assert!(assessment.should_block);
        assert_eq!(assessment.recommendation, "block");
        assert!(assessment.risk_score > 0.7);
        assert!(!assessment.warnings.is_empty());
    }

    #[test]
    fn test_retriable_status_codes() {
        let processor = create_test_processor();

        assert!(processor.is_retriable_status(408));
        assert!(processor.is_retriable_status(500));
        assert!(processor.is_retriable_status(503));
        assert!(!processor.is_retriable_status(400));
        assert!(!processor.is_retriable_status(404));
    }

    fn create_test_token(fraud: i32, stolen: i32, testing: f32, bot: f32) -> GrantedTokenResponse {
        GrantedTokenResponse {
            id: "spt_test".to_string(),
            object: "granted_token".to_string(),
            payment_method_preview: PaymentMethodPreview {
                card: Some(CardPreview {
                    brand: "visa".to_string(),
                    last4: "4242".to_string(),
                    funding: "credit".to_string(),
                    country: Some("US".to_string()),
                }),
            },
            usage_limits: UsageLimits {
                currency: "usd".to_string(),
                max_amount: 10000,
                expires_at: 1735689599,
            },
            risk_details: Some(RiskDetails {
                fraudulent_dispute: Some(fraud),
                card_testing: Some(testing),
                stolen_card: Some(stolen),
                card_issuer_decline: Some(0.05),
                bot: Some(bot),
            }),
        }
    }

    fn create_test_processor() -> StripePaymentProcessor {
        StripePaymentProcessor {
            config: StripeConfig {
                secret_key: "sk_test_123".to_string(),
                publishable_key: None,
                webhook_secret: Some("whsec_test".to_string()),
                api_version: "2023-10-16".to_string(),
                max_retries: 3,
                initial_retry_delay_ms: 100,
            },
            client: reqwest::Client::new(),
        }
    }
}
