use thiserror::Error;
use reqwest::Client;
use serde::Serialize;

type EmbeddingResponse = Vec<Vec<f32>>;

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    inputs: &'a [&'a str],
}

#[derive(Error, Debug)]
pub enum HuggingFaceError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("API error: status {status}, body {body}")]
    Api { status: u16, body: String },
}

pub async fn get_embeddings_hf(
    snippets: &[&str],
    hf_token: &str,
) -> Result<EmbeddingResponse, HuggingFaceError> {
    let model = "sentence-transformers/all-MiniLM-L6-v2";
    let api_url = format!("https://api-inference.huggingface.co/models/{}", model);
    let client = Client::new();
    let request_body = EmbeddingRequest { inputs: snippets };

    let res = client
        .post(&api_url)
        .bearer_auth(hf_token)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| HuggingFaceError::Network(e.to_string()))?;

    if !res.status().is_success() {
        let status = res.status().as_u16();
        let error_body = res.text().await.unwrap_or_else(|_| String::new());
        return Err(HuggingFaceError::Api { status, body: error_body });
    }

    res.json::<Vec<Vec<f32>>>()
        .await
        .map_err(|e| HuggingFaceError::Network(e.to_string()))
}
