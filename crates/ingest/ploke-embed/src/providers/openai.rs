use crate::{config::OpenAIConfig, error::{EmbedError, OpenAIError}};



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

        let res = client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?; // Uses From<reqwest::Error>

        if !res.status().is_success() {
            return Err(OpenAIError::ApiError { 
                status: res.status().as_u16(), 
                body: res.text().await?
            }.into());
        }

        let response = res.json::<OpenAIEmbedResponse>().await?;
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

