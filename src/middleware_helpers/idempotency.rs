use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use bytes::Bytes;
use dashmap::DashMap;
use http_body_util::BodyExt as _;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct IdempotencyStore(Arc<DashMap<String, StoredResponse>>);

impl Default for IdempotencyStore {
    fn default() -> Self {
        Self(Arc::new(DashMap::new()))
    }
}

impl IdempotencyStore {
    pub fn new() -> Self { Self::default() }
    pub fn get(&self, key: &str, ttl: Duration) -> Option<StoredResponse> {
        if let Some(sr) = self.0.get(key) {
            if sr.stored_at.elapsed() < ttl { return Some(sr.clone()); }
        }
        None
    }
    pub fn insert(&self, key: &str, sr: StoredResponse) {
        self.0.insert(key.to_string(), sr);
    }
    pub fn cleanup(&self, ttl: Duration) {
        let now = Instant::now();
        self.0.retain(|_, sr| now.duration_since(sr.stored_at) < ttl);
    }
}

#[derive(Clone)]
pub struct StoredResponse {
    pub status: StatusCode,
    pub body: Bytes,
    pub content_type: Option<HeaderValue>,
    pub stored_at: Instant,
}

// Simple idempotency middleware: for mutating methods with Idempotency-Key header,
// reject repeated keys within TTL. This is a protective baseline; a full solution would
// persist the response and replay it.
pub async fn idempotency_middleware(mut req: Request, next: Next) -> Response {
    static TTL_SECS: u64 = 600; // 10 minutes
    static HEADER: &str = "idempotency-key";

    let method = req.method().clone();
    let is_mutating = matches!(method.as_str(), "POST" | "PUT" | "PATCH" | "DELETE");

    if !is_mutating { return next.run(req).await; }

    let Some(key) = req
        .headers()
        .get(HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string()) else {
        return next.run(req).await;
    };

    let store = req
        .extensions()
        .get::<IdempotencyStore>()
        .cloned()
        .unwrap_or_default();

    let ttl = Duration::from_secs(TTL_SECS);
    store.cleanup(ttl);

    // Replay previously stored response
    if let Some(stored) = store.get(&key, ttl) {
        let mut resp = Response::new(axum::body::Body::from(stored.body.clone()));
        *resp.status_mut() = stored.status;
        if let Some(ct) = stored.content_type.clone() {
            resp.headers_mut().insert(HeaderName::from_static("content-type"), ct);
        }
        return resp;
    }

    // Call next and capture response for storage
    let resp = next.run(req).await;
    let (parts, body) = resp.into_parts();
    // Try to buffer the body. If it fails, return original response without storing.
    match body.collect().await {
        Ok(collected) => {
            let bytes = collected.to_bytes();
            let ct = parts.headers.get("content-type").cloned();
            let stored = StoredResponse {
                status: parts.status,
                body: bytes.clone(),
                content_type: ct,
                stored_at: Instant::now(),
            };
            store.insert(&key, stored);
            let resp = Response::from_parts(parts, axum::body::Body::from(bytes));
            resp
        }
        Err(_) => Response::from_parts(parts, axum::body::Body::empty()),
    }
}
