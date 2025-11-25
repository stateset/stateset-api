use crate::cache::InMemoryCache;
use crate::models::CheckoutSession;
use std::sync::Arc;
use tracing::error;

#[derive(Clone)]
pub struct RecoveryService {
    cache: Arc<InMemoryCache>,
}

impl RecoveryService {
    pub fn new(cache: Arc<InMemoryCache>) -> Self {
        Self { cache }
    }

    pub async fn get_abandoned_sessions(&self, older_than_minutes: i64) -> Vec<CheckoutSession> {
        let keys = self.cache.get_keys_by_prefix("checkout_session:").await;
        let mut abandoned = Vec::new();
        let cutoff = chrono::Utc::now() - chrono::Duration::minutes(older_than_minutes);

        for key in keys {
            if let Ok(Some(json)) = self.cache.get(&key).await {
                if let Ok(session) = serde_json::from_str::<CheckoutSession>(&json) {
                    // Check if active (not completed/canceled)
                    if session.status != crate::models::CheckoutSessionStatus::Completed 
                        && session.status != crate::models::CheckoutSessionStatus::Canceled 
                    {
                        // Check last update time
                        if let Ok(updated_at) = chrono::DateTime::parse_from_rfc3339(&session.updated_at) {
                            if updated_at < cutoff {
                                abandoned.push(session);
                            }
                        }
                    }
                } else {
                    error!("Failed to parse session for recovery: {}", key);
                }
            }
        }
        abandoned
    }
}
