use crate::return_service::ReturnService;
use crate::neural::cognitive::CognitiveService;
use crate::events::{Event, EventSender};
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info};

pub struct ReturnAgent {
    cognitive_service: Arc<CognitiveService>,
    return_service: Arc<ReturnService>,
    event_sender: Arc<EventSender>,
    interval: Duration,
}

impl ReturnAgent {
    pub fn new(
        cognitive_service: Arc<CognitiveService>,
        return_service: Arc<ReturnService>,
        event_sender: Arc<EventSender>,
        interval_seconds: u64,
    ) -> Self {
        Self {
            cognitive_service,
            return_service,
            event_sender,
            interval: Duration::from_secs(interval_seconds),
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(self.interval);
        info!("ReturnAgent started, running every {:?}", self.interval);

        loop {
            interval.tick().await;
            info!("ReturnAgent: Scanning for pending returns...");
            if let Err(e) = self.process_returns().await {
                error!("ReturnAgent error: {}", e);
            }
        }
    }

    async fn process_returns(&self) -> Result<(), anyhow::Error> {
        let pending = self.return_service.get_pending_returns();

        for request in pending {
            info!("ReturnAgent: Analyzing return {}", request.id);

            let system_prompt = "You are a Quality Control Agent for a retailer. \n            Analyze the customer's return reason and comment. \n            Determine if this indicates a product quality defect (e.g., broken, not working, poor material) vs a preference issue (e.g., wrong size, changed mind). \n            Respond with: QUALITY_ISSUE or PREFERENCE_ISSUE followed by a short explanation.";

            let user_query = format!(
                "Product ID: {}\nReason: {}\nComment: {}",
                request.product_id,
                request.reason,
                request.comment
            );

            let analysis = self.cognitive_service.chat_completion(system_prompt, &user_query).await?;
            
            let is_quality_issue = analysis.contains("QUALITY_ISSUE");

            self.return_service.update_analysis(&request.id, analysis.clone(), is_quality_issue);

            if is_quality_issue {
                info!("ReturnAgent: Quality Alert Triggered for return {}", request.id);
                self.event_sender.send(Event::QualityAlert {
                    product_id: request.product_id.clone(),
                    return_id: request.id.clone(),
                    reason: analysis,
                }).await?;
            } else {
                info!("ReturnAgent: Return {} classified as preference issue.", request.id);
            }
        }

        Ok(())
    }
}
