use crate::recovery_service::RecoveryService;
use crate::neural::cognitive::CognitiveService;
use crate::events::{Event, EventSender};
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info};

pub struct RecoveryAgent {
    cognitive_service: Arc<CognitiveService>,
    recovery_service: Arc<RecoveryService>,
    event_sender: Arc<EventSender>,
    interval: Duration,
}

impl RecoveryAgent {
    pub fn new(
        cognitive_service: Arc<CognitiveService>,
        recovery_service: Arc<RecoveryService>,
        event_sender: Arc<EventSender>,
        interval_seconds: u64,
    ) -> Self {
        Self {
            cognitive_service,
            recovery_service,
            event_sender,
            interval: Duration::from_secs(interval_seconds),
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(self.interval);
        info!("RecoveryAgent started, running every {:?}", self.interval);

        loop {
            interval.tick().await;
            info!("RecoveryAgent: Scanning for abandoned carts...");
            if let Err(e) = self.process_abandoned_carts().await {
                error!("RecoveryAgent error: {}", e);
            }
        }
    }

    async fn process_abandoned_carts(&self) -> Result<(), anyhow::Error> {
        // Find carts abandoned for more than 30 minutes
        let abandoned_sessions = self.recovery_service.get_abandoned_sessions(30).await;

        for session in abandoned_sessions {
            info!("RecoveryAgent: Processing abandoned session {}", session.id);

            // Check if we have customer info to contact
            if let Some(customer) = &session.customer {
                if let Some(email) = customer.shipping_address.as_ref().and_then(|a| a.email.as_ref()) {
                    
                    let items_desc = session.items.iter()
                        .map(|i| format!("{} x {}", i.quantity, i.title))
                        .collect::<Vec<_>>()
                        .join(", ");

                    let system_prompt = "You are a helpful Shopping Assistant. \n                    Write a short, friendly, and persuasive email subject and body to recover an abandoned cart. \n                    Offer a small incentive (e.g., free shipping) if appropriate. \n                    Keep it under 100 words.";

                    let user_query = format!(
                        "Customer Email: {}\nCart Items: {}\nTotal Value: {} {}\nGenerate a recovery message.",
                        email, items_desc, session.totals.grand_total.amount, session.totals.grand_total.currency
                    );

                    let recovery_message = self.cognitive_service.chat_completion(system_prompt, &user_query).await?;

                    info!("RecoveryAgent: Sending recovery message to {}", email);
                    
                    self.event_sender.send(Event::CartRecoveryMessageSent {
                        session_id: session.id.clone(),
                        email: email.clone(),
                        message: recovery_message,
                    }).await?;

                } else {
                    info!("RecoveryAgent: No email found for session {}, skipping.", session.id);
                }
            }
        }

        Ok(())
    }
}
