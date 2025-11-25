use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReturnRequest {
    pub id: String,
    pub product_id: String,
    pub reason: String,
    pub comment: String,
    pub status: String, // "pending", "analyzed", "approved", "rejected"
    pub analysis: Option<String>,
}

#[derive(Clone)]
pub struct ReturnService {
    returns: Arc<RwLock<HashMap<String, ReturnRequest>>>,
}

impl ReturnService {
    pub fn new() -> Self {
        Self {
            returns: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn create_return(&self, product_id: String, reason: String, comment: String) -> ReturnRequest {
        let id = Uuid::new_v4().to_string();
        let request = ReturnRequest {
            id: id.clone(),
            product_id,
            reason,
            comment,
            status: "pending".to_string(),
            analysis: None,
        };

        let mut returns = self.returns.write().unwrap();
        returns.insert(id.clone(), request.clone());
        request
    }

    pub fn get_pending_returns(&self) -> Vec<ReturnRequest> {
        let returns = self.returns.read().unwrap();
        returns
            .values()
            .filter(|r| r.status == "pending")
            .cloned()
            .collect()
    }

    pub fn update_analysis(&self, id: &str, analysis: String, is_quality_issue: bool) -> Option<ReturnRequest> {
        let mut returns = self.returns.write().unwrap();
        if let Some(req) = returns.get_mut(id) {
            req.analysis = Some(analysis);
            req.status = "analyzed".to_string();
            // In a real system, we might auto-flag status based on is_quality_issue
            Some(req.clone())
        } else {
            None
        }
    }
}
