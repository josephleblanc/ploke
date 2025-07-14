use std::time::Duration;

use serde::Serialize;

use crate::{config::HuggingFaceConfig, error::{EmbedError, truncate_string}};

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    inputs: &'a [&'a str],
}

// HuggingFace backend implementation
#[derive(Debug)]
pub struct HuggingFaceBackend {
    pub token: String,
    pub model: String,
    pub dimensions: usize,
}

impl HuggingFaceBackend {
    pub fn new(config: &HuggingFaceConfig) -> Self {
        Self {
            token: config.api_key.clone(),
            model: config.model.clone(),
            dimensions: config.dimensions,
        }
    }

    pub async fn compute_batch(
        &self,
        snippets: Vec<String>
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        let client = reqwest::Client::new();
        let inputs: Vec<&str> = snippets.iter().map(|s| s.as_str()).collect();
        let request_body = EmbeddingRequest { inputs: &inputs };
        let endpoint = format!("https://api-inference.huggingface.co/models/{}", self.model);

        let res = client
            .post(&endpoint)
            .bearer_auth(&self.token)
            .json(&request_body)
            .timeout(Duration::from_secs(30))  // Add timeout
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

        res.json::<Vec<Vec<f32>>>()
            .await
            .map_err(|e| EmbedError::Network(format!("Deserialization failed: {}", truncate_string(&e.to_string(), 60))))
    }
}
