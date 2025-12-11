use crate::{errors::ServiceError, AppState};
use axum::{body::Bytes, extract::State, http::HeaderMap, response::IntoResponse};
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::Sha256;
use tracing::{info, warn};

type HmacSha256 = Hmac<Sha256>;

// POST /api/v1/payments/webhook
#[utoipa::path(
    post,
    path = "/api/v1/payments/webhook",
    request_body = String,
    responses(
        (status = 200, description = "Webhook accepted"),
        (status = 401, description = "Invalid signature", body = crate::errors::ErrorResponse),
        (status = 400, description = "Invalid payload", body = crate::errors::ErrorResponse)
    ),
    tag = "Payments"
)]
pub async fn payment_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ServiceError> {
    let payload = body.clone();

    // Require a configured secret; reject unsigned webhooks to avoid spoofed events.
    let Some(secret) = state.config.payment_webhook_secret.as_ref() else {
        warn!("Payment webhook rejected because APP__PAYMENT_WEBHOOK_SECRET is not set");
        return Err(ServiceError::Unauthorized(
            "webhook signature verification is not configured".to_string(),
        ));
    };

    let ok = verify_signature(
        &headers,
        &payload,
        secret,
        state.config.payment_webhook_tolerance_secs.unwrap_or(300),
    );
    if !ok {
        warn!("Payment webhook signature verification failed");
        return Err(ServiceError::Unauthorized(
            "invalid webhook signature".to_string(),
        ));
    }

    // Parse JSON
    let json: Value = serde_json::from_slice(&payload)
        .map_err(|e| ServiceError::BadRequest(format!("invalid json: {}", e)))?;

    // Idempotency for webhooks using event id (if available)
    if let Some(event_id) = json.get("id").and_then(|v| v.as_str()) {
        let key = format!("wh:{}", event_id);
        if let Ok(mut conn) = state.redis.get_async_connection().await {
            let exists: Result<bool, _> = redis::cmd("SET")
                .arg(&key)
                .arg("1")
                .arg("NX")
                .arg("EX")
                .arg(24 * 3600)
                .query_async(&mut conn)
                .await;
            if let Ok(false) = exists {
                // already processed
                info!("Webhook event {} already processed", event_id);
                return Ok((axum::http::StatusCode::OK, "ok"));
            }
        }
    }

    // Minimal event handling
    let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match event_type {
        "payment.succeeded" | "charge.succeeded" => {
            let _ = crate::events::outbox::enqueue(
                &*state.db,
                "payment",
                None,
                "PaymentSucceeded",
                &json,
            )
            .await;
        }
        "payment.failed" | "charge.failed" => {
            let _ =
                crate::events::outbox::enqueue(&*state.db, "payment", None, "PaymentFailed", &json)
                    .await;
        }
        "payment.refunded" | "charge.refunded" => {
            let _ = crate::events::outbox::enqueue(
                &*state.db,
                "payment",
                None,
                "PaymentRefunded",
                &json,
            )
            .await;
        }
        _ => {
            info!("Unhandled payment webhook type: {}", event_type);
        }
    }

    Ok((axum::http::StatusCode::OK, "ok"))
}

fn verify_signature(
    headers: &HeaderMap,
    payload: &Bytes,
    secret: &str,
    tolerance_secs: u64,
) -> bool {
    // Generic HMAC: x-timestamp and x-signature headers
    if let (Some(ts), Some(sig)) = (headers.get("x-timestamp"), headers.get("x-signature")) {
        if let (Ok(ts), Ok(sig)) = (ts.to_str(), sig.to_str()) {
            // Optional: check timestamp tolerance
            if let Ok(ts_i) = ts.parse::<i64>() {
                let now = chrono::Utc::now().timestamp();
                if (now - ts_i).unsigned_abs() > tolerance_secs {
                    return false;
                }
            }
            // Explicitly validate UTF-8 encoding - do not silently fallback
            let payload_str = match std::str::from_utf8(payload) {
                Ok(s) => s,
                Err(_) => {
                    warn!("Webhook payload is not valid UTF-8");
                    return false;
                }
            };
            let signed = format!("{}.{}", ts, payload_str);

            // Safely create HMAC - secret must be valid
            let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
                Ok(m) => m,
                Err(_) => {
                    warn!("Invalid webhook secret for HMAC");
                    return false;
                }
            };
            mac.update(signed.as_bytes());
            let expected = hex::encode(mac.finalize().into_bytes());
            return constant_time_eq(&expected, sig);
        }
    }
    // Stripe-like support: Stripe-Signature with t=, v1=
    if let Some(sig) = headers
        .get("Stripe-Signature")
        .and_then(|h| h.to_str().ok())
    {
        let mut ts = "";
        let mut v1 = "";
        for part in sig.split(',') {
            let mut it = part.split('=');
            match (it.next(), it.next()) {
                (Some("t"), Some(val)) => ts = val,
                (Some("v1"), Some(val)) => v1 = val,
                _ => {}
            }
        }
        if !ts.is_empty() && !v1.is_empty() {
            // Explicitly validate UTF-8 encoding - do not silently fallback
            let payload_str = match std::str::from_utf8(payload) {
                Ok(s) => s,
                Err(_) => {
                    warn!("Webhook payload is not valid UTF-8");
                    return false;
                }
            };
            let signed = format!("{}.{}", ts, payload_str);

            // Safely create HMAC - secret must be valid
            let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
                Ok(m) => m,
                Err(_) => {
                    warn!("Invalid webhook secret for HMAC");
                    return false;
                }
            };
            mac.update(signed.as_bytes());
            let expected = hex::encode(mac.finalize().into_bytes());
            return constant_time_eq(&expected, v1);
        }
    }
    false
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut res = 0u8;
    for (x, y) in a.as_bytes().iter().zip(b.as_bytes()) {
        res |= x ^ y;
    }
    res == 0
}
