use crate::{config::OpenAIConfig, error::{EmbedError, truncate_string}};

// OpenAI backend implementation
#[derive(Debug)]
pub struct OpenAIBackend {
    pub api_key: String,
    pub model: String,
    pub dimensions: usize,
}

impl OpenAIBackend {
    pub fn new(config: &OpenAIConfig) -> Self {
        Self {
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            dimensions: 1536, // text-embedding-ada-002 standard size
        }
    }

    pub async fn compute_batch(
        &self,
        snippets: Vec<String>
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let client = reqwest::Client::new();
        let request = OpenAIEmbedRequest {
            model: self.model.clone(),
            input: snippets,
        };
        let endpoint = "https://api.openai.com/v1/embeddings".to_string();

        let res = client
            .post(&endpoint)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?; // Uses From<reqwest::Error>

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_else(|_| "<unreadable>".into());
            return Err(EmbedError::HttpError {
                status,
                body,
                url: endpoint,
            });
        }

        let response = res.json::<OpenAIEmbedResponse>()
            .await
            .map_err(|e| EmbedError::Network(format!("Deserialization failed: {}", truncate_string(&e.to_string(), 60))))?;
        Ok(response.data.into_iter().map(|d| d.embedding).collect())
    }
}

// Request structs for openAI
#[derive(serde::Serialize)]
struct OpenAIEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(serde::Deserialize)]
struct OpenAIEmbedding {
    embedding: Vec<f32>,
}

#[derive(serde::Deserialize)]
struct OpenAIEmbedResponse {
    data: Vec<OpenAIEmbedding>,
}

