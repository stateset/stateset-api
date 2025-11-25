use crate::product_catalog::ProductCatalogService;
use crate::neural::cognitive::CognitiveService;
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info};

pub struct PricingAgent {
    cognitive_service: Arc<CognitiveService>,
    product_catalog: Arc<ProductCatalogService>,
    interval: Duration,
}

impl PricingAgent {
    pub fn new(
        cognitive_service: Arc<CognitiveService>,
        product_catalog: Arc<ProductCatalogService>,
        interval_seconds: u64,
    ) -> Self {
        Self {
            cognitive_service,
            product_catalog,
            interval: Duration::from_secs(interval_seconds),
        }
    }

    pub async fn run(&self) {
        let mut interval = time::interval(self.interval);
        info!("PricingAgent started, running every {:?}", self.interval);

        loop {
            interval.tick().await;
            info!("PricingAgent: Analyzing prices...");
            if let Err(e) = self.optimize_prices().await {
                error!("PricingAgent error: {}", e);
            }
        }
    }

    async fn optimize_prices(&self) -> Result<(), anyhow::Error> {
        let products = self.product_catalog.get_all_products().await?;

        for product in products {
            // Mock demand and competitor data for prototype
            // In a real system, this would come from Analytics Service and Scrapers
            let demand_level = if product.inventory_quantity < 10 { "High" } else { "Moderate" };
            let competitor_price = (product.price as f64 * 1.1) as i64; // Competitor is slightly more expensive

            let system_prompt = "You are an autonomous Pricing Agent for an e-commerce platform. \    Your goal is to optimize revenue by adjusting prices based on stock levels, demand, and competitor pricing. \    Respond with a JSON object containing: \    - \"new_price\": the suggested price in cents (integer). \    - \"reason\": a short explanation.";

            let user_query = format!(
                "Product: {}\nCurrent Price: {}\nStock: {}\nDemand: {}\nCompetitor Price: {}\nSuggest a price.",
                product.name,
                product.price,
                product.inventory_quantity,
                demand_level,
                competitor_price
            );

            let analysis = self.cognitive_service.chat_completion(system_prompt, &user_query).await?;

            // Parse JSON output
            #[derive(serde::Deserialize)]
            struct PricingSuggestion {
                new_price: i64,
                reason: String,
            }

            // Helper to clean markdown code blocks if present
            let json_str = if let Some(start) = analysis.find('{') {
                if let Some(end) = analysis.rfind('}') {
                    &analysis[start..=end]
                } else {
                    &analysis
                }
            } else {
                &analysis
            };

            match serde_json::from_str::<PricingSuggestion>(json_str) {
                Ok(suggestion) => {
                    if suggestion.new_price != product.price {
                        info!(
                            "PricingAgent: Updating price for {} from {} to {}. Reason:வதற்காக {}",
                            product.name, product.price, suggestion.new_price, suggestion.reason
                        );
                        if let Err(e) = self.product_catalog.update_price(&product.id, suggestion.new_price) {
                            error!("Failed to update price for {}: {}", product.name, e);
                        }
                    } else {
                        info!("PricingAgent: Keeping price for {}. Reason: {}", product.name, suggestion.reason);
                    }
                }
                Err(e) => {
                    error!("PricingAgent: Failed to parse LLM response for {}: {}. Response: {}", product.name, e, analysis);
                }
            }
        }

        Ok(())
    }
}
