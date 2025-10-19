use crate::errors::ServiceError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, instrument, warn};

/// Stripe configuration
#[derive(Clone)]
pub struct StripeConfig {
    pub secret_key: String,
    pub publishable_key: Option<String>,
}

impl StripeConfig {
    pub fn from_env() -> Result<Self, ServiceError> {
        let secret_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
            ServiceError::InternalError(
                "STRIPE_SECRET_KEY environment variable not set".to_string(),
            )
        })?;

        let publishable_key = std::env::var("STRIPE_PUBLISHABLE_KEY").ok();

        Ok(Self {
            secret_key,
            publishable_key,
        })
    }
}

/// Stripe payment processor
#[derive(Clone)]
pub struct StripePaymentProcessor {
    config: StripeConfig,
    client: reqwest::Client,
}

impl StripePaymentProcessor {
    pub fn new(config: StripeConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Process payment using SharedPaymentToken
    #[instrument(skip(self))]
    pub async fn process_shared_payment_token(
        &self,
        token: &str,
        amount: i64,
        currency: &str,
        metadata: HashMap<String, String>,
    ) -> Result<PaymentIntentResponse, ServiceError> {
        info!(
            "Processing SharedPaymentToken: {} for amount {}",
            token, amount
        );

        // Create PaymentIntent with SharedPaymentToken
        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("amount".to_string(), amount.to_string());
        params.insert("currency".to_string(), currency.to_string());
        params.insert("shared_payment_token".to_string(), token.to_string());
        params.insert("confirm".to_string(), "true".to_string());

        // Add metadata
        for (key, value) in metadata {
            params.insert(format!("metadata[{}]", key), value);
        }

        let response = self
            .client
            .post("https://api.stripe.com/v1/payment_intents")
            .basic_auth(&self.config.secret_key, Some(""))
            .form(&params)
            .send()
            .await
            .map_err(|e| ServiceError::InternalError(format!("Stripe API error: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            warn!("Stripe API error: {}", error_text);
            return Err(ServiceError::PaymentFailed(format!(
                "Stripe error: {}",
                error_text
            )));
        }

        let payment_intent: PaymentIntentResponse = response.json().await.map_err(|e| {
            ServiceError::ParseError(format!("Failed to parse Stripe response: {}", e))
        })?;

        info!("PaymentIntent created: {}", payment_intent.id);
        Ok(payment_intent)
    }

    /// Retrieve granted token details (risk assessment)
    #[instrument(skip(self))]
    pub async fn get_granted_token(
        &self,
        token_id: &str,
    ) -> Result<GrantedTokenResponse, ServiceError> {
        let url = format!(
            "https://api.stripe.com/v1/shared_payment/granted_tokens/{}",
            token_id
        );

        let response = self
            .client
            .get(&url)
            .basic_auth(&self.config.secret_key, Some(""))
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

    /// Assess risk from granted token
    pub fn assess_risk(&self, token: &GrantedTokenResponse) -> RiskAssessment {
        let mut should_block = false;
        let mut warnings = Vec::new();

        if let Some(risks) = &token.risk_details {
            // Check fraudulent dispute risk
            if let Some(fraud_score) = risks.fraudulent_dispute {
                if fraud_score > 75 {
                    should_block = true;
                    warnings.push("High fraudulent dispute risk".to_string());
                } else if fraud_score > 50 {
                    warnings.push("Moderate fraudulent dispute risk".to_string());
                }
            }

            // Check stolen card risk
            if let Some(stolen_score) = risks.stolen_card {
                if stolen_score > 75 {
                    should_block = true;
                    warnings.push("High stolen card risk".to_string());
                }
            }

            // Check card testing
            if let Some(testing_score) = risks.card_testing {
                if testing_score > 0.8 {
                    should_block = true;
                    warnings.push("Card testing detected".to_string());
                }
            }
        }

        RiskAssessment {
            should_block,
            warnings,
            recommendation: if should_block { "block" } else { "continue" }.to_string(),
        }
    }

    /// Capture a payment intent
    #[instrument(skip(self))]
    pub async fn capture_payment(
        &self,
        payment_intent_id: &str,
    ) -> Result<PaymentIntentResponse, ServiceError> {
        let url = format!(
            "https://api.stripe.com/v1/payment_intents/{}/capture",
            payment_intent_id
        );

        let response = self
            .client
            .post(&url)
            .basic_auth(&self.config.secret_key, Some(""))
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

        info!("Payment captured: {}", payment_intent.id);
        Ok(payment_intent)
    }
}

// Response types

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentIntentResponse {
    pub id: String,
    pub object: String,
    pub amount: i64,
    pub currency: String,
    pub status: String,
    pub payment_method: Option<String>,
    pub client_secret: Option<String>,
}

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
    pub funding: String, // credit, debit, prepaid
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsageLimits {
    pub currency: String,
    pub max_amount: i64,
    pub expires_at: i64, // Unix timestamp
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RiskDetails {
    pub fraudulent_dispute: Option<i32>,  // 0-100
    pub card_testing: Option<f32>,        // 0.0-1.0
    pub stolen_card: Option<i32>,         // 0-100
    pub card_issuer_decline: Option<f32>, // 0.0-1.0
    pub bot: Option<f32>,                 // 0.0-1.0
}

#[derive(Debug)]
pub struct RiskAssessment {
    pub should_block: bool,
    pub warnings: Vec<String>,
    pub recommendation: String, // "block" or "continue"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_assessment() {
        let token = GrantedTokenResponse {
            id: "spt_test".to_string(),
            object: "granted_token".to_string(),
            payment_method_preview: PaymentMethodPreview {
                card: Some(CardPreview {
                    brand: "visa".to_string(),
                    last4: "4242".to_string(),
                    funding: "credit".to_string(),
                }),
            },
            usage_limits: UsageLimits {
                currency: "usd".to_string(),
                max_amount: 10000,
                expires_at: 1735689599,
            },
            risk_details: Some(RiskDetails {
                fraudulent_dispute: Some(25),
                card_testing: Some(0.1),
                stolen_card: Some(10),
                card_issuer_decline: Some(0.05),
                bot: Some(0.02),
            }),
        };

        let processor = StripePaymentProcessor {
            config: StripeConfig {
                secret_key: "sk_test_123".to_string(),
                publishable_key: None,
            },
            client: reqwest::Client::new(),
        };

        let assessment = processor.assess_risk(&token);
        assert!(!assessment.should_block);
        assert_eq!(assessment.recommendation, "continue");
    }

    #[test]
    fn test_high_risk_blocking() {
        let token = GrantedTokenResponse {
            id: "spt_test".to_string(),
            object: "granted_token".to_string(),
            payment_method_preview: PaymentMethodPreview {
                card: Some(CardPreview {
                    brand: "visa".to_string(),
                    last4: "0000".to_string(),
                    funding: "credit".to_string(),
                }),
            },
            usage_limits: UsageLimits {
                currency: "usd".to_string(),
                max_amount: 10000,
                expires_at: 1735689599,
            },
            risk_details: Some(RiskDetails {
                fraudulent_dispute: Some(90), // High risk!
                card_testing: Some(0.9),      // High risk!
                stolen_card: Some(85),        // High risk!
                card_issuer_decline: Some(0.5),
                bot: Some(0.3),
            }),
        };

        let processor = StripePaymentProcessor {
            config: StripeConfig {
                secret_key: "sk_test_123".to_string(),
                publishable_key: None,
            },
            client: reqwest::Client::new(),
        };

        let assessment = processor.assess_risk(&token);
        assert!(assessment.should_block);
        assert_eq!(assessment.recommendation, "block");
        assert!(!assessment.warnings.is_empty());
    }
}
