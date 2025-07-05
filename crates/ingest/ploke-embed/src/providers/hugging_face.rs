use std::time::Duration;

use serde::Serialize;

use crate::{config::HuggingFaceConfig, error::{EmbedError, HuggingFaceError}};

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

        let res = client
            .post(format!("https://api-inference.huggingface.co/models/{}", self.model))
            .bearer_auth(&self.token)
            .json(&request_body)
            .timeout(Duration::from_secs(30))  // Add timeout
            .send()
            .await?; // Uses From<reqwest::Error>

        if !res.status().is_success() {
            return Err(HuggingFaceError::ApiError { 
                status: res.status().as_u16(), 
                body: res.text().await?
            }.into());
        }

        res.json().await.map_err(Into::into)
    }
}
