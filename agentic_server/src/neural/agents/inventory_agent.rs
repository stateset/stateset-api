use crate::product_catalog::ProductCatalogService;
use crate::neural::cognitive::CognitiveService;
use crate::neural::semantic_search::SemanticSearchService;
use crate::events::{Event, EventSender};
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info};

pub struct InventoryAgent {
    cognitive_service: Arc<CognitiveService>,
    semantic_search_service: Arc<SemanticSearchService>,
    product_catalog: Arc<ProductCatalogService>,
    event_sender: Arc<EventSender>,
    interval: Duration,
}

impl InventoryAgent {
    pub fn new(
        cognitive_service: Arc<CognitiveService>,
        semantic_search_service: Arc<SemanticSearchService>,
        product_catalog: Arc<ProductCatalogService>,
        event_sender: Arc<EventSender>,
        interval_seconds: u64,
    ) -> Self {
        Self {
            cognitive_service,
            semantic_search_service,
            product_catalog,
            event_sender,
            interval: Duration::from_secs(interval_seconds),
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(self.interval);
        info!("InventoryAgent started, running every {:?}", self.interval);

        loop {
            interval.tick().await;
            info!("InventoryAgent: Checking inventory...");
            if let Err(e) = self.check_inventory().await {
                error!("InventoryAgent error: {}", e);
            }
        }
    }

    async fn check_inventory(&self) -> Result<(), anyhow::Error> {
        let products = self.product_catalog.get_all_products().await?;

        for product in products {
            // Mocked sales trend and lead time for now
            let sales_trend = "trending up 20% DoD";
            let supplier_lead_time = "2 weeks";

            let system_prompt = r###"You are an autonomous Inventory Management Agent for Stateset Neural Grid. Your goal is to decide if a product needs to be reordered based on inventory data, sales trends, and supplier lead times. Respond ONLY with "REORDER" or "NO_REORDER" followed by a brief justification."###;

            let user_query = format!(
                "Product: {}\nCurrent Stock: {}\nSales Trend: {}\nSupplier Lead Time: {}\nShould this product be reordered?",
                product.name,
                product.inventory_quantity,
                sales_trend,
                supplier_lead_time
            );

            let llm_response = self.cognitive_service.chat_completion(system_prompt, &user_query).await?;

            if llm_response.trim().starts_with("REORDER") {
                info!("InventoryAgent: REORDER needed for {}. Justification: {}", product.name, llm_response);
                // In a real scenario, this would create a draft PO
                self.event_sender.send(Event::PurchaseOrderDrafted { 
                    product_id: product.id.to_string(), 
                    quantity: 100, // Mock quantity
                    reason: llm_response.clone(),
                }).await?;
            } else {
                info!("InventoryAgent: NO_REORDER for {}. Justification: {}", product.name, llm_response);
            }
        }

        Ok(())
    }
}