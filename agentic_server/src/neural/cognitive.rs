use async_openai::{
    types::{
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use tracing::error;

#[derive(Clone)]
pub struct CognitiveService {
    client: Client<async_openai::config::OpenAIConfig>,
}

impl CognitiveService {
    pub fn new(api_key: &str) -> Self {
        let config = async_openai::config::OpenAIConfig::new().with_api_key(api_key);
        let client = Client::with_config(config);
        Self { client }
    }

    pub async fn chat_completion(&self, system_prompt: &str, user_query: &str) -> Result<String, anyhow::Error> {
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-4o-mini")
            .messages([
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system_prompt)
                    .build()?
                    .into(),
                ChatCompletionRequestUserMessageArgs::default()
                    .content(user_query)
                    .build()?
                    .into(),
            ])
            .build()?;

        let response = self.client.chat().create(request).await.map_err(|e| {
            error!("Failed to create chat completion: {}", e);
            e
        })?;

        if let Some(choice) = response.choices.first() {
            if let Some(content) = &choice.message.content {
                Ok(content.clone())
            } else {
                Err(anyhow::anyhow!("No content returned in chat completion"))
            }
        } else {
            Err(anyhow::anyhow!("No choices returned in chat completion"))
        }
    }
}
