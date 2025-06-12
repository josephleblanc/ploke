// src/backend.rs
use flume::{Receiver, Sender};
use reqwest;
use serde_json::{json, Value};
use std::env;
use tokio::time::{self, Duration};

use crate::app::{AppEvent, BackendRequest, BackendResponse};

/// Spawns a Tokio task that simulates your ploke backend.
/// It receives requests from the TUI and sends back responses.
pub async fn start_backend_listener(
    backend_rx: Receiver<BackendRequest>,
    app_event_tx: Sender<AppEvent>, // To send responses back to the App
) -> color_eyre::Result<()> {
    while let Ok(request) = backend_rx.recv_async().await {
        match request {
            BackendRequest::Query(query) => {
                let client = reqwest::Client::new();
                let api_key = env::var("OPENROUTER_API_KEY");

                if api_key.is_err() {
                    let response_text = "OPENROUTER_API_KEY not set. Please set the environment variable to use the LLM.".to_string();
                    if app_event_tx.send(AppEvent::BackendResponse(response_text)).is_err() {
                        break;
                    }
                    continue; // Skip to the next request
                }

                let api_key = api_key.unwrap();
                let api_url = "https://openrouter.ai/api/v1/chat/completions";

                let request_body = json!({
                    "model": "meta-llama/llama-3-8b-instruct",
                    "messages": [{"role": "user", "content": query}]
                });

                let response = client
                    .post(api_url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&request_body)
                    .send()
                    .await;

                match response {
                    Ok(res) => {
                        if res.status().is_success() {
                            match res.json::<Value>().await {
                                Ok(response_json) => {
                                    if let Some(content) = response_json["choices"][0]["message"]["content"].as_str() {
                                        if app_event_tx.send(AppEvent::BackendResponse(content.to_string())).is_err() {
                                            break;
                                        }
                                    } else {
                                        let error_message = "Failed to extract content from LLM response.".to_string();
                                        if app_event_tx.send(AppEvent::BackendResponse(error_message)).is_err() {
                                            break;
                                        }
                                    }
                                }
                                Err(err) => {
                                    let error_message = format!("Failed to parse LLM response JSON: {}", err);
                                    if app_event_tx.send(AppEvent::BackendResponse(error_message)).is_err() {
                                        break;
                                    }
                                }
                            }
                        } else {
                            let error_message = format!("LLM API request failed with status: {}", res.status());
                            if app_event_tx.send(AppEvent::BackendResponse(error_message)).is_err() {
                                break;
                            }
                        }
                    }
                    Err(err) => {
                        let error_message = format!("Failed to send request to LLM API: {}", err);
                        if app_event_tx.send(AppEvent::BackendResponse(error_message)).is_err() {
                            break;
                        }
                    }
                }
            }
            // Handle other backend request types as your project evolves
        }
    }
    Ok(())
}
