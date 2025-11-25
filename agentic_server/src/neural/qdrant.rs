use qdrant_client::prelude::*;
use qdrant_client::qdrant::{
    Condition, CreateCollection, Filter, PointStruct, ScoredPoint, SearchPoints, VectorParams,
    VectorsConfig,
};
use std::sync::Arc;
use tracing::info;

const COLLECTION_NAME: &str = "products";
const VECTOR_SIZE: u64 = 1536; // OpenAI text-embedding-3-small size

pub struct QdrantService {
    client: Arc<QdrantClient>,
}

impl QdrantService {
    pub async fn new(url: &str) -> Result<Self, anyhow::Error> {
        let client = Arc::new(QdrantClient::from_url(url).build()?);

        let service = Self { client };
        service.ensure_collection().await?;

        Ok(service)
    }

    async fn ensure_collection(&self) -> Result<(), anyhow::Error> {
        if !self.client.collection_exists(COLLECTION_NAME).await? {
            info!("Creating Qdrant collection: {}", COLLECTION_NAME);
            self.client
                .create_collection(&CreateCollection {
                    collection_name: COLLECTION_NAME.to_string(),
                    vectors_config: Some(VectorsConfig {
                        config: Some(qdrant_client::qdrant::vectors_config::Config::Params(VectorParams {
                            size: VECTOR_SIZE,
                            distance: Distance::Cosine.into(),
                            ..Default::default()
                        })),
                    }),
                    ..Default::default()
                })
                .await?;
        }
        Ok(())
    }

    pub async fn upsert_point(
        &self,
        id: uuid::Uuid,
        vector: Vec<f32>,
        payload: serde_json::Value,
    ) -> Result<(), anyhow::Error> {
        let payload: Payload = payload.try_into().map_err(|e| anyhow::anyhow!("Invalid payload: {}", e))?;
        
        let point = PointStruct::new(id.to_string(), vector, payload);

        self.client
            .upsert_points(COLLECTION_NAME, None, vec![point], None)
            .await?;

        Ok(())
    }

    pub async fn search(
        &self,
        vector: Vec<f32>,
        limit: u64,
        score_threshold: Option<f32>,
    ) -> Result<Vec<ScoredPoint>, anyhow::Error> {
        let search_result = self
            .client
            .search_points(&SearchPoints {
                collection_name: COLLECTION_NAME.to_string(),
                vector: vector,
                limit,
                score_threshold,
                with_payload: Some(true.into()),
                ..Default::default()
            })
            .await?;

        Ok(search_result.result)
    }
}