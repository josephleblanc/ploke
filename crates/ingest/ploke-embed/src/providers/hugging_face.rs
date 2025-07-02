// In your remote embedding module

use reqwest::Client;
use serde::Serialize;

// The structure of the response from the feature-extraction task is a nested array.
// For example: [[-0.1, 0.2, ...], [-0.3, 0.4, ...]]
type EmbeddingResponse = Vec<Vec<f32>>;

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    inputs: &'a [&'a str],
}

pub async fn get_embeddings_hf(
    snippets: &[&str],
    hf_token: &str,
) -> Result<EmbeddingResponse, reqwest::Error> {
    // Using a popular, high-performance sentence-transformer model
    let model = "sentence-transformers/all-MiniLM-L6-v2";
    let api_url = format!("https://api-inference.huggingface.co/models/{}", model);

    let client = Client::new();

    let request_body = EmbeddingRequest { inputs: snippets };

    let res = client
        .post(&api_url)
        .bearer_auth(hf_token)
        .json(&request_body)
        .send()
        .await?;

    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().await?;
        // If the model is loading, it returns a 503 error. You need to handle this.
        eprintln!("API Error: Status {}, Body: {}", status, error_body);
        // A robust implementation would include retry logic for 503s.
        return Err(reqwest::Error::from(status));
    }

    let response_body: EmbeddingResponse = res.json().await?;

    Ok(response_body)
}
