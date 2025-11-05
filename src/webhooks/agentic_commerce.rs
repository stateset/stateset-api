use crate::errors::ServiceError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument, warn};

/// Webhook event types for Agentic Commerce Protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebhookEvent {
    #[serde(rename = "order_created")]
    OrderCreated { data: OrderEventData },

    #[serde(rename = "order_updated")]
    OrderUpdated { data: OrderEventData },
}

/// Order event data payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderEventData {
    #[serde(rename = "type")]
    pub data_type: String, // "order"
    pub checkout_session_id: String,
    pub permalink_url: String,
    pub status: String,
    pub refunds: Vec<Refund>,
}

/// Refund information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refund {
    #[serde(rename = "type")]
    pub refund_type: String, // "store_credit" or "original_payment"
    pub amount: i64,
}

/// HMAC signature generator for webhook authentication
pub struct SignatureGenerator {
    secret: String,
}

impl SignatureGenerator {
    pub fn new(secret: String) -> Self {
        Self { secret }
    }

    /// Generate HMAC signature for webhook payload
    pub fn sign_payload(&self, timestamp: &str, body: &str) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let signed_payload = format!("{}.{}", timestamp, body);
        let mut mac = HmacSha256::new_from_slice(self.secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(signed_payload.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }
}

/// Webhook delivery service for Agentic Commerce Protocol
#[derive(Clone)]
pub struct AgenticCommerceWebhookService {
    client: reqwest::Client,
    signature_generator: Option<Arc<SignatureGenerator>>,
    max_retries: u32,
}

impl AgenticCommerceWebhookService {
    pub fn new(webhook_secret: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            signature_generator: webhook_secret
                .map(|secret| Arc::new(SignatureGenerator::new(secret))),
            max_retries: 3,
        }
    }

    /// Send order created webhook to OpenAI
    #[instrument(skip(self))]
    pub async fn send_order_created(
        &self,
        webhook_url: &str,
        checkout_session_id: String,
        order_id: String,
        permalink_url: String,
    ) -> Result<(), ServiceError> {
        let event = WebhookEvent::OrderCreated {
            data: OrderEventData {
                data_type: "order".to_string(),
                checkout_session_id,
                permalink_url,
                status: "created".to_string(),
                refunds: vec![],
            },
        };

        self.send_async(webhook_url.to_string(), event);
        Ok(())
    }

    /// Send order updated webhook to OpenAI
    #[instrument(skip(self))]
    pub async fn send_order_updated(
        &self,
        webhook_url: &str,
        checkout_session_id: String,
        permalink_url: String,
        status: String,
        refunds: Vec<Refund>,
    ) -> Result<(), ServiceError> {
        let event = WebhookEvent::OrderUpdated {
            data: OrderEventData {
                data_type: "order".to_string(),
                checkout_session_id,
                permalink_url,
                status,
                refunds,
            },
        };

        self.send_async(webhook_url.to_string(), event);
        Ok(())
    }

    /// Send webhook with retry logic
    #[instrument(skip(self, event))]
    async fn send_webhook(
        &self,
        webhook_url: &str,
        event: WebhookEvent,
    ) -> Result<(), ServiceError> {
        let body = serde_json::to_string(&event)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))?;

        let timestamp = chrono::Utc::now().to_rfc3339();

        // Generate signature if secret available
        let signature = self
            .signature_generator
            .as_ref()
            .map(|gen| gen.sign_payload(&timestamp, &body));

        // Retry logic with exponential backoff
        for attempt in 1..=self.max_retries {
            let mut request = self
                .client
                .post(webhook_url)
                .header("Content-Type", "application/json")
                .header("Timestamp", &timestamp)
                .body(body.clone());

            // Add signature if available
            if let Some(ref sig) = signature {
                request = request.header("Merchant-Signature", sig);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        info!("Webhook delivered successfully to {}", webhook_url);
                        return Ok(());
                    } else {
                        warn!(
                            "Webhook delivery failed with status: {} (attempt {}/{})",
                            response.status(),
                            attempt,
                            self.max_retries
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Webhook delivery error: {} (attempt {}/{})",
                        e, attempt, self.max_retries
                    );
                }
            }

            // Exponential backoff: 1s, 2s, 4s
            if attempt < self.max_retries {
                let backoff = Duration::from_secs(2_u64.pow(attempt - 1));
                tokio::time::sleep(backoff).await;
            }
        }

        error!(
            "Webhook delivery failed after {} attempts",
            self.max_retries
        );
        Err(ServiceError::ExternalServiceError(format!(
            "Failed to deliver webhook after {} retries",
            self.max_retries
        )))
    }

    /// Send webhook asynchronously (fire-and-forget with logging)
    pub fn send_async(&self, webhook_url: String, event: WebhookEvent) {
        let service = self.clone();

        tokio::spawn(async move {
            if let Err(e) = service.send_webhook(&webhook_url, event).await {
                error!("Async webhook delivery failed: {}", e);
                // In production, add to dead letter queue here
            }
        });
    }
}

impl Default for AgenticCommerceWebhookService {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_event_serialization() {
        let event = WebhookEvent::OrderCreated {
            data: OrderEventData {
                data_type: "order".to_string(),
                checkout_session_id: "session_123".to_string(),
                permalink_url: "https://example.com/orders/123".to_string(),
                status: "created".to_string(),
                refunds: vec![],
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("order_created"));
        assert!(json.contains("session_123"));
    }

    #[test]
    fn test_refund_serialization() {
        let refund = Refund {
            refund_type: "store_credit".to_string(),
            amount: 5000,
        };

        let json = serde_json::to_string(&refund).unwrap();
        assert!(json.contains("store_credit"));
        assert!(json.contains("5000"));
    }

    #[test]
    fn test_signature_generation() {
        let generator = SignatureGenerator::new("test_secret".to_string());
        let timestamp = "2025-01-01T00:00:00Z";
        let body = r#"{"type":"order_created"}"#;

        let sig = generator.sign_payload(timestamp, body);
        assert!(!sig.is_empty());
        assert_eq!(sig.len(), 64); // SHA256 produces 32 bytes = 64 hex chars
    }
}
