use super::cognitive::CognitiveService;
use super::semantic_search::SemanticSearchService;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct ChatService {
    cognitive: Arc<CognitiveService>,
    search: Arc<SemanticSearchService>,
}

#[derive(Serialize, Deserialize)]
pub struct ChatResponse {
    pub response: String,
    pub relevant_products: Vec<super::semantic_search::SearchResult>,
}

impl ChatService {
    pub fn new(cognitive: Arc<CognitiveService>, search: Arc<SemanticSearchService>) -> Self {
        Self { cognitive, search }
    }

    pub async fn chat_with_inventory(&self, user_query: &str) -> Result<ChatResponse, anyhow::Error> {
        // 1. Retrieve relevant context
        let search_results = self.search.search(user_query, 5).await?;

        // 2. Format context for the LLM
        let mut context_str = String::from("Here are the relevant products found in the inventory:\n\n");
        for (i, result) in search_results.iter().enumerate() {
            context_str.push_str(&format!("{}. ID: {}\n", i + 1, result.id));
            context_str.push_str(&format!("   Relevance: {:.2}\n", result.score));
            context_str.push_str(&format!("   Details: {}\n\n", result.payload));
        }

        // 3. Construct System Prompt
        let system_prompt = format!(
            r#"You are an expert commerce assistant for the Stateset Neural Grid.
Your goal is to help customers find the perfect product or answer questions about inventory.

Use the following context to answer the user's question.
Context:
{}

Rules:
- Only recommend products from the context.
- If the context doesn't have the answer, admit it politely.
- Be concise and helpful."#,
            context_str
        );

        // 4. Generate Response
        let llm_response = self.cognitive.chat_completion(&system_prompt, user_query).await?;

        Ok(ChatResponse {
            response: llm_response,
            relevant_products: search_results,
        })
    }
}