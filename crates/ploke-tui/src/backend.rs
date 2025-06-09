// src/backend.rs
use flume::{Receiver, Sender};
use tokio::time::{self, Duration};

use crate::app::{BackendRequest, BackendResponse, AppEvent};

/// Spawns a Tokio task that simulates your ploke backend.
/// It receives requests from the TUI and sends back responses.
pub async fn start_backend_listener(
    backend_rx: Receiver<BackendRequest>,
    app_event_tx: Sender<AppEvent>, // To send responses back to the App
) -> color_eyre::Result<()> {
    while let Ok(request) = backend_rx.recv_async().await {
        match request {
            BackendRequest::Query(query) => {
                // Simulate a long-running LLM/RAG operation
                time::sleep(Duration::from_secs(2)).await;

                let response_text = format!("Processed query: '{}'. This is a simulated LLM response.", query);
                
                // Send the response back to the App
                if app_event_tx.send(AppEvent::BackendResponse(response_text)).is_err() {
                    // App channel closed, likely shutting down
                    break;
                }
            }
            // Handle other backend request types as your project evolves
        }
    }
    Ok(())
}
