use axum::{extract::Request, middleware::Next, response::Response, http::{StatusCode, HeaderName}};
use http_body_util::BodyExt as _;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tracing::{warn, error};

#[derive(Serialize, Deserialize)]
struct Stored {
    status: u16,
    content_type: Option<String>,
    body: String,
}

pub async fn idempotency_redis_middleware(
    axum::extract::State(redis_client): axum::extract::State<Arc<redis::Client>>,
    req: Request,
    next: Next,
) -> Response {
    const HEADER: &str = "idempotency-key";
    const TTL_SECS: usize = 600; // 10 minutes

    let method = req.method().clone();
    let is_mutating = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
    if !is_mutating { return next.run(req).await; }

    let Some(key) = req.headers().get(HEADER).and_then(|v| v.to_str().ok()).map(|s| s.trim().to_string()) else {
        return next.run(req).await;
    };

    // Compose cache key
    let path = req.uri().path().to_string();
    let cache_key = format!("idem:{}:{}:{}", method, path, key);
    let lock_key = format!("{}:lock", cache_key);

    // Get redis connection; on failure, bypass idempotency
    let mut conn = match redis_client.get_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            warn!("Idempotency Redis connection failed: {}", e);
            return next.run(req).await;
        }
    };

    // Replay if cached
    if let Ok(Some(json)) = conn.get::<_, String>(&cache_key).await {
        if let Ok(stored) = serde_json::from_str::<Stored>(&json) {
            let mut resp = Response::new(axum::body::Body::from(stored.body));
            *resp.status_mut() = StatusCode::from_u16(stored.status).unwrap_or(StatusCode::OK);
            if let Some(ct) = stored.content_type.and_then(|s| s.parse().ok()) {
                resp.headers_mut().insert(HeaderName::from_static("content-type"), ct);
            }
            return resp;
        }
    }

    // Try to acquire processing lock
    match conn.set_nx::<_, _, bool>(&lock_key, "1").await {
        Ok(true) => {
            let _ : Result<(), _> = conn.expire(&lock_key, TTL_SECS).await;
        }
        Ok(false) => {
            // Someone else is processing; see if result has landed
            if let Ok(Some(json)) = conn.get::<_, String>(&cache_key).await {
                if let Ok(stored) = serde_json::from_str::<Stored>(&json) {
                    let mut resp = Response::new(axum::body::Body::from(stored.body));
                    *resp.status_mut() = StatusCode::from_u16(stored.status).unwrap_or(StatusCode::OK);
                    if let Some(ct) = stored.content_type.and_then(|s| s.parse().ok()) {
                        resp.headers_mut().insert(HeaderName::from_static("content-type"), ct);
                    }
                    return resp;
                }
            }
            // No cached result yet; reject duplicate-in-progress to prevent double-execution
            return (StatusCode::CONFLICT, "Duplicate request in progress").into_response();
        }
        Err(e) => {
            warn!("Idempotency Redis SETNX failed: {}", e);
            return next.run(req).await;
        }
    }

    // Process request and cache response
    let resp = next.run(req).await;
    let (parts, body) = resp.into_parts();
    match body.collect().await {
        Ok(collected) => {
            let bytes = collected.to_bytes();
            let ct = parts.headers.get("content-type").and_then(|v| v.to_str().ok()).map(|s| s.to_string());
            let stored = Stored { status: parts.status.as_u16(), content_type: ct, body: String::from_utf8_lossy(&bytes).to_string() };
            if let Ok(json) = serde_json::to_string(&stored) {
                let _: Result<(), _> = conn.set_ex(&cache_key, json, TTL_SECS).await;
            }
            let _: Result<(), _> = conn.del(&lock_key).await;
            Response::from_parts(parts, axum::body::Body::from(bytes))
        }
        Err(e) => {
            error!("Failed to buffer response body for idempotency: {}", e);
            let _: Result<(), _> = conn.del(&lock_key).await;
            Response::from_parts(parts, axum::body::Body::empty())
        }
    }
}

