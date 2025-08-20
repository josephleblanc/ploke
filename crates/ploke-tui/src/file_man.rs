use std::sync::Arc;

use ploke_db::TypedEmbedData;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::{fs, sync::mpsc};
use tracing::{error, info, warn};

use crate::error::ErrorSeverity;
use crate::{AppEvent, EventBus, system::SystemEvent};
use crate::{ErrorEvent, RagEvent};

pub struct FileManager {
    io_handle: ploke_io::IoManagerHandle,
    event_rx: broadcast::Receiver<AppEvent>,
    event_tx: broadcast::Sender<AppEvent>,
    context_tx: mpsc::Sender<RagEvent>,
    realtime_event_tx: broadcast::Sender<AppEvent>,
}

impl FileManager {
    /// Creates a new FileManager instance
    pub fn new(
        io_handle: ploke_io::IoManagerHandle,
        event_rx: broadcast::Receiver<AppEvent>,
        event_tx: broadcast::Sender<AppEvent>,
        context_tx: mpsc::Sender<RagEvent>,
        realtime_event_tx: broadcast::Sender<AppEvent>,
    ) -> Self {
        Self {
            io_handle,
            event_rx,
            event_tx,
            context_tx,
            realtime_event_tx,
        }
    }

    /// Main event loop for file operations
    pub async fn run(mut self) {
        while let Ok(event) = self.event_rx.recv().await {
            self.handle_event(event).await
        }
    }

    /// Processes incoming file-related events
    async fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::System(SystemEvent::SaveRequested(content)) => {
                let path = match std::env::current_dir() {
                    Ok(pwd_path) => pwd_path,
                    Err(e) => {
                        error!(
                            "Save faild, working directory invalid
                            Either cwd does not exist or insufficient permissions, prop error\n{}",
                            e.to_string()
                        );
                        // Surface to UI
                        let _ = self.event_tx.send(AppEvent::Error(ErrorEvent {
                            message: format!("Save failed: working directory invalid: {}", e),
                            severity: ErrorSeverity::Error,
                        }));
                        return;
                    }
                };
                match self.save_content(&path, &content).await {
                    Ok(final_path) => {
                        info!("Chat history saved to {}", final_path.display());
                    }
                    Err(e) => {
                        error!("Save failed: {}", e);
                        // Surface to UI
                        let _ = self.event_tx.send(AppEvent::Error(ErrorEvent {
                            message: format!("Save failed: {}", e),
                            severity: ErrorSeverity::Error,
                        }));
                    }
                }
            }
            AppEvent::System(SystemEvent::ReadQuery {
                file_name,
                query_name,
            }) => {
                let path = match std::env::current_dir() {
                    Ok(pwd_path) => pwd_path.join("query").join(file_name),
                    Err(e) => {
                        self.send_path_error(e);
                        return;
                    }
                };
                let query_content = match tokio::fs::read_to_string(path).await {
                    Ok(s) => s,
                    Err(e) => {
                        self.send_path_error(e);
                        return;
                    }
                };
                if let Err(e) = self
                    .realtime_event_tx
                    .send(AppEvent::System(SystemEvent::WriteQuery {
                        query_content,
                        query_name,
                    }))
                {
                    warn!(
                        "Failed to forward WriteQuery to realtime channel: {}",
                        e
                    );
                }
            }
            AppEvent::System(SystemEvent::ReadSnippet(ty_emb_data)) => {
                // tracing::info!(
                //     "Received ReadSnippet for type {}, next calling get_snippets_batch with ty_emb_data {:?}",
                //     ty_emb_data.ty.relation_str(),
                //     ty_emb_data.v
                // );
                // if let Ok(result) = self.io_handle.get_snippets_batch(ty_emb_data.v).await {
                //     let mut output = Vec::new();
                //     for snip_res in result {
                //         match snip_res {
                //             Ok(snippet) => {
                //                 tracing::trace!("Adding snippet to output: {}", snippet);
                //                 output.push(snippet);
                //             }
                //             Err(e) => {
                //                 error!("get_snippets_batch failed with: {}", e);
                //             }
                //         }
                //     }
                // tracing::info!("Finished reading snippets, collected output: {:?}", output);
                // match self
                //     .context_tx
                //     .send(RagEvent::ContextSnippets(id, output))
                //     .await
                // {
                //     Ok(_) => {
                //         tracing::trace!("Exiting send CodeSnippets with Ok");
                //         // self.event_tx
                //         //     .send(AppEvent::System(SystemEvent::CompleteReadSnip(output))).expect("Terrible things");
                //     }
                //     Err(e) => {
                //         tracing::trace!(
                //             "Err whiile trying to send CodeSnippets: {}",
                //             e.to_string()
                //         );
                //     }
                // };
                // }
            }
            other => warn!("FileManager received unexpected event: {:?}", other),
        }
    }

    fn send_path_error(&mut self, e: std::io::Error) {
        let message = e.to_string();
        warn!("Failed to load query from file {}", message);
        self.event_tx
            .send(AppEvent::Error(ErrorEvent {
                message,
                severity: ErrorSeverity::Warning,
            }))
            .expect("Invariant violated: ReadQuery event after AppEvent reader closed");
    }

    /// Saves content to disk atomically in a temp location then moves to final path
    async fn save_content(
        &self,
        dir: &std::path::Path,
        content: &[u8],
    ) -> Result<std::path::PathBuf, std::io::Error> {
        let final_path = dir.join(".ploke_history.md");
        let temp_path = dir.join(".ploke_history.md.tmp");

        info!("Saving chat history atomically to {}", final_path.display());

        // Write to temp file in the same directory
        let mut temp_file = fs::File::create(&temp_path).await?;
        temp_file.write_all(content).await?;
        temp_file.sync_all().await?;

        // On Unix, rename within the same directory is atomic and replaces the target.
        fs::rename(&temp_path, &final_path).await?;

        Ok(final_path)
    }

    /// Computes default save path for chat history
    fn default_history_path(&self) -> std::path::PathBuf {
        dirs::document_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".ploke.history")
    }
}

// --- Possible macro for DRY if ReadQuery pattern repeats ---
// use std::path::{Path, PathBuf};
//
// /// Convenience: `Ok(value)` if `op` succeeds, otherwise
// /// `self.send_path_error(err)` and `return`.
// macro_rules! try_or_return {
//     ($self:expr, $op:expr) => {
//         match $op {
//             Ok(v) => v,
//             Err(e) => {
//                 $self.send_path_error(e);
//                 return;
//             }
//         }
//     };
// }
//
// // usage --------------------------------------------------------------
// let path = try_or_return!(self, std::env::current_dir().map(|p|
// p.join("query").join(file)));
// let content = try_or_return!(self, tokio::fs::read_to_string(path).await);
