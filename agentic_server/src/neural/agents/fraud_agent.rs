use crate::fraud_service::FraudService;
use crate::neural::cognitive::CognitiveService;
use crate::events::{Event, EventSender};
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info};

pub struct FraudAgent {
    cognitive_service: Arc<CognitiveService>,
    fraud_service: Arc<FraudService>,
    event_sender: Arc<EventSender>,
    interval: Duration,
}

impl FraudAgent {
    pub fn new(
        cognitive_service: Arc<CognitiveService>,
        fraud_service: Arc<FraudService>,
        event_sender: Arc<EventSender>,
        interval_seconds: u64,
    ) -> Self {
        Self {
            cognitive_service,
            fraud_service,
            event_sender,
            interval: Duration::from_secs(interval_seconds),
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(self.interval);
        info!("FraudAgent started, running every {:?}", self.interval);

        loop {
            interval.tick().await;
            info!("FraudAgent: Scanning for pending fraud cases...");
            if let Err(e) = self.process_cases().await {
                error!("FraudAgent error: {}", e);
            }
        }
    }

    async fn process_cases(&self) -> Result<(), anyhow::Error> {
        let pending = self.fraud_service.get_pending_cases();

        for case in pending {
            info!("FraudAgent: Analyzing session {}", case.session.id);

            let customer_info = serde_json::to_string_pretty(&case.session.customer).unwrap_or_default();
            let totals_info = serde_json::to_string_pretty(&case.session.totals).unwrap_or_default();

            let system_prompt = "You are an expert Fraud Detection Agent for an e-commerce platform. \n            Analyze the transaction details for potential fraud indicators (e.g., high value, mismatched addresses, suspicious email). \n            Respond with a JSON object containing: \n            - \"risk_score\": a number between 0 and 100 (100 is confirmed fraud). \n            - \"risk_factors\": a list of strings explaining the risk factors found.";

            let user_query = format!(
                "Customer Details:\n{}\n\nTransaction Totals:\n{}",
                customer_info,
                totals_info
            );

            let analysis = self.cognitive_service.chat_completion(system_prompt, &user_query).await?;
            
            // Simple parsing of the JSON response (assuming LLM returns valid JSON or close to it)
            // For robustness, we might want to use a proper JSON parser and error handling or structured output if supported.
            // Here we'll try to extract JSON if it's wrapped in markdown blocks.
            let json_str = if let Some(start) = analysis.find('{') {
                if let Some(end) = analysis.rfind('}') {
                    &analysis[start..=end]
                } else {
                    &analysis
                }
            } else {
                &analysis
            };

            #[derive(serde::Deserialize)]
            struct FraudAnalysis {
                risk_score: f32,
                risk_factors: Vec<String>,
            }

            let result: FraudAnalysis = match serde_json::from_str(json_str) {
                Ok(r) => r,
                Err(_) => {
                    // Fallback if parsing fails
                    FraudAnalysis {
                        risk_score: 50.0,
                        risk_factors: vec!["Manual review required (LLM output parsing failed)".to_string()],
                    }
                }
            };

            info!("FraudAgent: Risk Score {} for session {}", result.risk_score, case.session.id);

            self.fraud_service.update_assessment(&case.session.id, result.risk_score, result.risk_factors.clone());

            // Emit event if high risk
            if result.risk_score > 75.0 {
                 // self.event_sender.send(Event::FraudAlert ... ) - need to add this event type
                 // For now, just log
                 info!("FraudAgent: HIGH RISK ALERT for session {}", case.session.id);
            }
        }

        Ok(())
    }
}
