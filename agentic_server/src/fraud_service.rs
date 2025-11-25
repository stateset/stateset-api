use crate::models::CheckoutSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FraudCase {
    pub session: CheckoutSession,
    pub status: FraudStatus,
    pub risk_score: Option<f32>,
    pub risk_factors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FraudStatus {
    Pending,
    UnderReview,
    Approved,
    Rejected,
}

#[derive(Clone)]
pub struct FraudService {
    cases: Arc<RwLock<HashMap<String, FraudCase>>>,
}

impl FraudService {
    pub fn new() -> Self {
        Self {
            cases: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn queue_for_review(&self, session: CheckoutSession) {
        let case = FraudCase {
            session: session.clone(),
            status: FraudStatus::Pending,
            risk_score: None,
            risk_factors: Vec::new(),
        };
        let mut cases = self.cases.write().unwrap();
        cases.insert(session.id.clone(), case);
    }

    pub fn get_pending_cases(&self) -> Vec<FraudCase> {
        let cases = self.cases.read().unwrap();
        cases
            .values()
            .filter(|c| c.status == FraudStatus::Pending)
            .cloned()
            .collect()
    }

    pub fn update_assessment(&self, session_id: &str, score: f32, factors: Vec<String>) {
        let mut cases = self.cases.write().unwrap();
        if let Some(case) = cases.get_mut(session_id) {
            case.risk_score = Some(score);
            case.risk_factors = factors;
            case.status = if score > 80.0 {
                FraudStatus::Rejected
            } else {
                FraudStatus::Approved
            };
        }
    }
}
