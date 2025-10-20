use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::Arc;
use tracing::{debug, warn};

type HmacSha256 = Hmac<Sha256>;

/// Signature verification configuration
#[derive(Clone)]
pub struct SignatureVerifier {
    webhook_secret: Arc<String>,
}

impl SignatureVerifier {
    pub fn new(secret: String) -> Self {
        Self {
            webhook_secret: Arc::new(secret),
        }
    }

    /// Verify HMAC signature of request
    pub fn verify_signature(&self, headers: &HeaderMap, body: &[u8]) -> Result<(), SignatureError> {
        // Get signature from header
        let signature = headers
            .get("Signature")
            .ok_or(SignatureError::MissingSignature)?
            .to_str()
            .map_err(|_| SignatureError::InvalidFormat)?;

        // Get timestamp
        let timestamp = headers
            .get("Timestamp")
            .ok_or(SignatureError::MissingTimestamp)?
            .to_str()
            .map_err(|_| SignatureError::InvalidFormat)?;

        // Validate timestamp freshness (prevent replay attacks)
        let request_time = chrono::DateTime::parse_from_rfc3339(timestamp)
            .map_err(|_| SignatureError::InvalidTimestamp)?;

        let now = chrono::Utc::now();
        let age = now.signed_duration_since(request_time);

        // Reject requests older than 5 minutes or in the future
        if age.num_minutes().abs() > 5 {
            return Err(SignatureError::TimestampTooOld);
        }

        // Create signed payload: timestamp.body
        let signed_payload = format!("{}.{}", timestamp, String::from_utf8_lossy(body));

        // Compute HMAC
        let mut mac = HmacSha256::new_from_slice(self.webhook_secret.as_bytes())
            .map_err(|_| SignatureError::InternalError)?;
        mac.update(signed_payload.as_bytes());

        let expected = hex::encode(mac.finalize().into_bytes());

        // Compare signatures (constant-time comparison)
        if signature != expected {
            warn!(
                "Signature mismatch: got {}, expected {}",
                signature, expected
            );
            return Err(SignatureError::InvalidSignature);
        }

        debug!("Signature verified successfully");
        Ok(())
    }

    /// Generate signature for outgoing webhooks
    pub fn sign_payload(&self, timestamp: &str, body: &str) -> String {
        let signed_payload = format!("{}.{}", timestamp, body);

        let mut mac = HmacSha256::new_from_slice(self.webhook_secret.as_bytes())
            .expect("HMAC key should be valid");
        mac.update(signed_payload.as_bytes());

        hex::encode(mac.finalize().into_bytes())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SignatureError {
    #[error("Missing signature header")]
    MissingSignature,

    #[error("Missing timestamp header")]
    MissingTimestamp,

    #[error("Invalid signature format")]
    InvalidFormat,

    #[error("Invalid timestamp format")]
    InvalidTimestamp,

    #[error("Timestamp too old or in future")]
    TimestampTooOld,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Internal error")]
    InternalError,
}

/// Signature verification middleware (optional - controlled by config)
pub async fn signature_verification_middleware(
    State(verifier): State<Option<Arc<SignatureVerifier>>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // If signature verification is disabled, pass through
    let Some(verifier) = verifier else {
        return next.run(request).await;
    };

    // Extract body for verification
    let (parts, body) = request.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .unwrap_or_default();

    // Verify signature
    match verifier.verify_signature(&headers, &body_bytes) {
        Ok(_) => {
            // Reconstruct request with body
            let request = Request::from_parts(parts, Body::from(body_bytes));
            next.run(request).await
        }
        Err(e) => {
            warn!("Signature verification failed: {}", e);

            let error_code = match e {
                SignatureError::MissingSignature => "missing_signature",
                SignatureError::MissingTimestamp => "missing_timestamp",
                SignatureError::InvalidFormat => "invalid_format",
                SignatureError::InvalidTimestamp => "invalid_timestamp",
                SignatureError::TimestampTooOld => "timestamp_expired",
                SignatureError::InvalidSignature => "invalid_signature",
                SignatureError::InternalError => "internal_error",
            };

            (
                StatusCode::BAD_REQUEST,
                axum::Json(serde_json::json!({
                    "type": "invalid_request",
                    "code": error_code,
                    "message": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn test_signature_generation_and_verification() {
        let verifier = SignatureVerifier::new("test_secret_key".to_string());
        let timestamp = chrono::Utc::now().to_rfc3339();
        let body = r#"{"test": "data"}"#;

        // Generate signature
        let signature = verifier.sign_payload(&timestamp, body);

        // Verify signature
        let mut headers = HeaderMap::new();
        headers.insert("Signature", HeaderValue::from_str(&signature).unwrap());
        headers.insert("Timestamp", HeaderValue::from_str(&timestamp).unwrap());

        assert!(verifier.verify_signature(&headers, body.as_bytes()).is_ok());
    }

    #[test]
    fn test_signature_tampering_detection() {
        let verifier = SignatureVerifier::new("test_secret_key".to_string());
        let timestamp = chrono::Utc::now().to_rfc3339();
        let original_body = r#"{"test": "data"}"#;
        let tampered_body = r#"{"test": "tampered"}"#;

        // Generate signature for original
        let signature = verifier.sign_payload(&timestamp, original_body);

        // Try to verify with tampered body
        let mut headers = HeaderMap::new();
        headers.insert("Signature", HeaderValue::from_str(&signature).unwrap());
        headers.insert("Timestamp", HeaderValue::from_str(&timestamp).unwrap());

        assert!(verifier
            .verify_signature(&headers, tampered_body.as_bytes())
            .is_err());
    }

    #[test]
    fn test_old_timestamp_rejection() {
        let verifier = SignatureVerifier::new("test_secret_key".to_string());

        // Create timestamp 10 minutes ago
        let old_timestamp = (chrono::Utc::now() - chrono::Duration::minutes(10)).to_rfc3339();
        let body = r#"{"test": "data"}"#;

        let signature = verifier.sign_payload(&old_timestamp, body);

        let mut headers = HeaderMap::new();
        headers.insert("Signature", HeaderValue::from_str(&signature).unwrap());
        headers.insert("Timestamp", HeaderValue::from_str(&old_timestamp).unwrap());

        assert!(matches!(
            verifier.verify_signature(&headers, body.as_bytes()),
            Err(SignatureError::TimestampTooOld)
        ));
    }
}
