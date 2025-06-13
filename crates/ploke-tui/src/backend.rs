// src/backend.rs
use flume::{Receiver, Sender};
use reqwest;
use serde_json::{json, Value};
use std::env;
use tokio::time::{self, Duration};

use crate::app::{AppEvent, BackendRequest, BackendResponse};

/// Spawns a Tokio task that communicates with the OpenAI API.
pub async fn start_backend_listener(
    backend_rx: flume::Receiver<BackendRequest>,
    app_event_tx: flume::Sender<AppEvent>,
    config: Config,
) -> color_eyre::Result<()> {
    let client = Client::new();
    let api_key = config.openai_api_key
        .or_else(|| env::var("OPENAI_API_KEY").ok())
        .unwrap_or_else(|| {
            let _ = app_event_tx.send(AppEvent::BackendResponse {
                model: "SYSTEM".to_string(),
                content: "\nERROR: No OpenAI API key found\n\nConfigure your API key either:\n1. Via config file: ~/.config/ploke/config.toml\n2. Environment variable: OPENAI_API_KEY\n\nGet key from https://platform.openai.com/api-keys".to_string(),
            });
            String::new() // Will fail API calls but allows graceful shutdown
        });
    
    let model_name = &config.model;

    while let Ok(request) = backend_rx.recv_async().await {
        match request {
            BackendRequest::Query(query) => {
                let response = match client
                    .post("https://api.openai.com/v1/chat/completions")
                    .header("Authorization", format!("Bearer {}", api_key))
                    .json(&json!({
                        "model": model_name,
                        "messages": [{"role": "user", "content": query}],
                        "temperature": 0.7
                    }))
                    .send()
                    .await
                {
                    Ok(res) => res,
                    Err(e) => {
                        let _ = app_event_tx.send(AppEvent::BackendResponse {
                            model: "System".to_string(),
                            content: format!("API Error: {}", e),
                        });
                        continue;
                    }
                };

                if !response.status().is_success() {
                    let error_msg = format!("API Error: {}", response.status());
                    let _ = app_event_tx.send(AppEvent::BackendResponse {
                        model: "System".to_string(),
                        content: error_msg,
                    });
                    continue;
                }

                let response_json: serde_json::Value = match response.json().await {
                    Ok(json) => json,
                    Err(e) => {
                        let _ = app_event_tx.send(AppEvent::BackendResponse {
                            model: "System".to_string(),
                            content: format!("Parse Error: {}", e),
                        });
                        continue;
                    }
                };

                let content = response_json["choices"][0]["message"]["content"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                if app_event_tx
                    .send(AppEvent::BackendResponse {
                        model: model_name.to_string(),
                        content,
                    })
                    .is_err()
                {
                    break;
                }
            }
        }
    }
    Ok(())
}
