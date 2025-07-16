use std::sync::Arc;

use ploke_db::TypedEmbedData;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::{fs, sync::mpsc};
use tracing::{error, info, warn};

use crate::ploke_rag::RagEvent;
use crate::{AppEvent, EventBus, system::SystemEvent};

pub struct FileManager {
    io_handle: ploke_io::IoManagerHandle,
    event_rx: broadcast::Receiver<crate::AppEvent>,
    event_tx: broadcast::Sender<crate::AppEvent>,
    context_tx: mpsc::Sender<RagEvent>,
}

impl FileManager {
    /// Creates a new FileManager instance
    pub fn new(
        io_handle: ploke_io::IoManagerHandle,
        event_rx: broadcast::Receiver<crate::AppEvent>,
        event_tx: broadcast::Sender<crate::AppEvent>,
        context_tx: mpsc::Sender<RagEvent>,
    ) -> Self {
        Self {
            io_handle,
            event_rx,
            event_tx,
            context_tx,
        }
    }

    /// Main event loop for file operations
    pub async fn run(mut self) {
        while let Ok(event) = self.event_rx.recv().await {
            self.handle_event(event).await
        }
    }

    /// Processes incoming file-related events
    async fn handle_event(&mut self, event: crate::AppEvent) {
        match event {
            AppEvent::System(SystemEvent::SaveRequested(content)) => {
                let path = self.default_history_path();
                if let Err(e) = self.save_content(&path, &content).await {
                    error!("Save failed: {}", e);
                }
            }
            AppEvent::System(SystemEvent::ReadSnippet(ty_emb_data)) => {
                tracing::info!(
                    "Received ReadSnippet for type {}, next calling get_snippets_batch with ty_emb_data {:?}",
                    ty_emb_data.ty.relation_str(),
                    ty_emb_data.v
                );
                if let Ok(result) = self.io_handle.get_snippets_batch(ty_emb_data.v).await {
                    let mut output = Vec::new();
                    for snip_res in result {
                        match snip_res {
                            Ok(snippet) => {
                                tracing::trace!("Adding snippet to output: {}", snippet);
                                output.push(snippet);
                            }
                            Err(e) => {
                                error!("get_snippets_batch failed with: {}", e);
                            }
                        }
                    }
                    tracing::info!("Finished reading snippets, collected output: {:?}", output);
                    match self
                        .context_tx
                        .send(RagEvent::ContextSnippets(output.clone()))
                        .await
                    {
                        Ok(_) => {
                            tracing::trace!("Exiting send CodeSnippets with Ok");
                            self.event_tx
                                .send(AppEvent::System(SystemEvent::CompleteReadSnip(output))).expect("Terrible things");
                        }
                        Err(e) => {
                            tracing::trace!(
                                "Err whiile trying to send CodeSnippets: {}",
                                e.to_string()
                            );
                        }
                    };
                }
            }
            other => warn!("FileManager received unexpected event: {:?}", other),
        }
    }

    /// Saves content to disk atomically in a temp location then moves to final path
    async fn save_content(
        &self,
        path: &std::path::Path,
        content: &[u8],
    ) -> Result<(), std::io::Error> {
        let temp_path = path.with_extension("tmp");
        let mut temp_file = fs::File::create(&temp_path).await?;
        temp_file.write_all(content).await?;
        temp_file.sync_all().await?;
        fs::rename(&temp_path, path).await?;
        info!("Chat history saved to {}", path.display());
        Ok(())
    }

    /// Computes default save path for chat history
    fn default_history_path(&self) -> std::path::PathBuf {
        dirs::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("ploke_history.md")
    }
}
