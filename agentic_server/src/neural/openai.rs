use async_openai::{
    types::{CreateEmbeddingRequestArgs, EmbeddingInput},
    Client,
};
use tracing::error;

pub struct OpenAIService {
    client: Client<async_openai::config::OpenAIConfig>,
}

impl OpenAIService {
    pub fn new(api_key: &str) -> Self {
        let config = async_openai::config::OpenAIConfig::new().with_api_key(api_key);
        let client = Client::with_config(config);
        Self { client }
    }

    pub async fn get_embedding(&self, text: &str) -> Result<Vec<f32>, anyhow::Error> {
        let request = CreateEmbeddingRequestArgs::default()
            .model("text-embedding-3-small")
            .input(EmbeddingInput::String(text.to_string()))
            .build()?;

        let response = self.client.embeddings().create(request).await.map_err(|e| {
            error!("Failed to create embedding: {}", e);
            e
        })?;

        if let Some(data) = response.data.first() {
            Ok(data.embedding.clone())
        } else {
            Err(anyhow::anyhow!("No embedding data returned"))
        }
    }
}
