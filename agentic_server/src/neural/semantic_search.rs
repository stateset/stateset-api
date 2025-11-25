use super::openai::OpenAIService;
use super::qdrant::QdrantService;
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::PointId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct SemanticSearchService {
    openai: Arc<OpenAIService>,
    qdrant: Arc<QdrantService>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchResult {
    pub id: String,
    pub score: f32,
    pub payload: serde_json::Value,
}

impl SemanticSearchService {
    pub fn new(openai: Arc<OpenAIService>, qdrant: Arc<QdrantService>) -> Self {
        Self { openai, qdrant }
    }

    pub async fn index_product(
        &self,
        id: Uuid,
        text_description: &str,
        metadata: serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let vector = self.openai.get_embedding(text_description).await?;
        self.qdrant.upsert_point(id, vector, metadata).await?;
        Ok(())
    }

    pub async fn search(&self, query: &str, limit: u64) -> Result<Vec<SearchResult>, anyhow::Error> {
        let vector = self.openai.get_embedding(query).await?;
        let points = self.qdrant.search(vector, limit, Some(0.7)).await?;

        let results = points
            .into_iter()
            .map(|p| SearchResult {
                id: p.id.map(point_id_to_string).unwrap_or_default(),
                score: p.score,
                payload: serde_json::to_value(p.payload).unwrap_or_default(),
            })
            .collect();

        Ok(results)
    }
}

fn point_id_to_string(id: PointId) -> String {
    match id.point_id_options {
        Some(PointIdOptions::Num(num)) => num.to_string(),
        Some(PointIdOptions::Uuid(uuid)) => uuid,
        None => String::new(),
    }
}