use axum::{
    extract::Request,
    http::{HeaderName, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt as _;
use lazy_static::lazy_static;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{error, warn};

const IDEMPOTENCY_HEADER: &str = "idempotency-key";
const IDEMPOTENCY_TTL_SECS: usize = 600; // 10 minutes
const FALLBACK_MAX_ENTRIES: usize = 1024;
const MAX_CACHED_RESPONSE_BYTES: usize = 256 * 1024;

lazy_static! {
    static ref FALLBACK_STORE: Mutex<FallbackStore> = Mutex::new(FallbackStore::new());
}

#[derive(Clone, Serialize, Deserialize)]
struct Stored {
    status: u16,
    content_type: Option<String>,
    body: String,
}

#[derive(Clone)]
struct FallbackEntry {
    inserted_at: Instant,
    stored: Stored,
}

struct FallbackStore {
    entries: HashMap<String, FallbackEntry>,
    insertion_order: VecDeque<(String, Instant)>,
}

impl FallbackStore {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            insertion_order: VecDeque::new(),
        }
    }

    fn prune(&mut self) {
        let ttl = Duration::from_secs(IDEMPOTENCY_TTL_SECS as u64);

        while let Some((_, inserted_at)) = self.insertion_order.front() {
            if inserted_at.elapsed() <= ttl {
                break;
            }

            let Some((key, inserted_at)) = self.insertion_order.pop_front() else {
                break;
            };

            if self
                .entries
                .get(&key)
                .is_some_and(|entry| entry.inserted_at == inserted_at)
            {
                self.entries.remove(&key);
            }
        }

        while self.entries.len() > FALLBACK_MAX_ENTRIES {
            let Some((key, inserted_at)) = self.insertion_order.pop_front() else {
                self.entries.clear();
                break;
            };

            if self
                .entries
                .get(&key)
                .is_some_and(|entry| entry.inserted_at == inserted_at)
            {
                self.entries.remove(&key);
            }
        }
    }

    fn get(&mut self, key: &str) -> Option<Stored> {
        self.prune();
        let ttl = Duration::from_secs(IDEMPOTENCY_TTL_SECS as u64);

        match self.entries.get(key) {
            Some(entry) if entry.inserted_at.elapsed() <= ttl => Some(entry.stored.clone()),
            Some(_) => {
                self.entries.remove(key);
                None
            }
            None => None,
        }
    }

    fn insert(&mut self, key: String, stored: Stored) {
        self.prune();
        let inserted_at = Instant::now();
        self.entries.insert(
            key.clone(),
            FallbackEntry {
                inserted_at,
                stored,
            },
        );
        self.insertion_order.push_back((key, inserted_at));
        self.prune();
    }
}

pub async fn idempotency_redis_middleware(
    axum::extract::State(redis_client): axum::extract::State<Arc<redis::Client>>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let is_mutating = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");
    if !is_mutating {
        return next.run(req).await;
    }

    let Some(key) = req
        .headers()
        .get(IDEMPOTENCY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
    else {
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
            return fallback_idempotency(cache_key, req, next).await;
        }
    };

    // Replay if cached
    if let Ok(Some(json)) = conn.get::<_, Option<String>>(&cache_key).await {
        if let Ok(stored) = serde_json::from_str::<Stored>(&json) {
            let mut resp = Response::new(axum::body::Body::from(stored.body));
            *resp.status_mut() = StatusCode::from_u16(stored.status).unwrap_or(StatusCode::OK);
            if let Some(ct) = stored.content_type.and_then(|s| s.parse().ok()) {
                resp.headers_mut()
                    .insert(HeaderName::from_static("content-type"), ct);
            }
            return resp;
        }
    }

    // Try to acquire processing lock
    match conn.set_nx::<_, _, bool>(&lock_key, "1").await {
        Ok(true) => {
            let _: Result<(), _> = conn.expire(&lock_key, IDEMPOTENCY_TTL_SECS).await;
        }
        Ok(false) => {
            // Someone else is processing; see if result has landed
            if let Ok(Some(json)) = conn.get::<_, Option<String>>(&cache_key).await {
                if let Ok(stored) = serde_json::from_str::<Stored>(&json) {
                    let mut resp = Response::new(axum::body::Body::from(stored.body));
                    *resp.status_mut() =
                        StatusCode::from_u16(stored.status).unwrap_or(StatusCode::OK);
                    if let Some(ct) = stored.content_type.and_then(|s| s.parse().ok()) {
                        resp.headers_mut()
                            .insert(HeaderName::from_static("content-type"), ct);
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
            if bytes.len() > MAX_CACHED_RESPONSE_BYTES {
                warn!(
                    "Idempotency response too large to cache ({} bytes)",
                    bytes.len()
                );
                let _: Result<(), _> = conn.del(&lock_key).await;
                return Response::from_parts(parts, axum::body::Body::from(bytes));
            }
            let ct = parts
                .headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let stored = Stored {
                status: parts.status.as_u16(),
                content_type: ct,
                body: String::from_utf8_lossy(&bytes).to_string(),
            };
            if let Ok(json) = serde_json::to_string(&stored) {
                let _: Result<(), _> = conn.set_ex(&cache_key, json, IDEMPOTENCY_TTL_SECS).await;
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

async fn fallback_idempotency(cache_key: String, req: Request, next: Next) -> Response {
    // Replay if cached in fallback store
    if let Some(stored) = {
        let mut store = FALLBACK_STORE.lock().await;
        store.get(&cache_key)
    } {
        let mut resp = Response::new(axum::body::Body::from(stored.body));
        *resp.status_mut() = StatusCode::from_u16(stored.status).unwrap_or(StatusCode::OK);
        if let Some(ct) = stored.content_type.and_then(|s| s.parse().ok()) {
            resp.headers_mut()
                .insert(HeaderName::from_static("content-type"), ct);
        }
        return resp;
    }

    let resp = next.run(req).await;
    let (parts, body) = resp.into_parts();
    match body.collect().await {
        Ok(collected) => {
            let bytes = collected.to_bytes();
            if bytes.len() > MAX_CACHED_RESPONSE_BYTES {
                warn!(
                    "Idempotency fallback response too large to cache ({} bytes)",
                    bytes.len()
                );
                return Response::from_parts(parts, axum::body::Body::from(bytes));
            }
            let ct = parts
                .headers
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let stored = Stored {
                status: parts.status.as_u16(),
                content_type: ct,
                body: String::from_utf8_lossy(&bytes).to_string(),
            };
            {
                let mut store = FALLBACK_STORE.lock().await;
                store.insert(cache_key, stored);
            }
            Response::from_parts(parts, axum::body::Body::from(bytes))
        }
        Err(e) => {
            error!(
                "Failed to buffer response body for idempotency fallback: {}",
                e
            );
            Response::from_parts(parts, axum::body::Body::empty())
        }
    }
}
