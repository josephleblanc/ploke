use std::io::Write;

use ploke_io::IoManagerHandle;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::{sync::broadcast, time::interval};
use tokio::time::Duration;
use tracing::{error, info, warn};

use crate::app_state::AppState;
use crate::system::SystemEvent;

pub struct FileManager {
    state: AppState,
    io_handle: IoManagerHandle,
    event_rx: broadcast::Receiver<crate::AppEvent>,
}

impl FileManager {
    /// Creates a new FileManager instance
    pub fn new(state: AppState, io_handle: IoManagerHandle, event_rx: broadcast::Receiver<crate::AppEvent>) -> Self {
        Self {
            state,
            io_handle,
            event_rx,
        }
    }

    /// Main event loop for file operations
    pub async fn run(mut self) {
        let mut autosave_interval = interval(Duration::from_secs(60 * 5)); // 5 minute autosave
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory");

        loop {
            tokio::select! {
                event = self.event_rx.recv() => self.handle_event(event).await,
                _ = autosave_interval.tick() => self.autosave(&temp_dir).await,
            }
        }
    }

    /// Processes incoming file-related events
    async fn handle_event(&mut self, event: Result<crate::AppEvent, tokio::sync::broadcast::error::RecvError>) {
        match event {
            Ok(crate::AppEvent::System(SystemEvent::SaveRequested)) => {
                self.save_chat_history(path, temp_dir)

            },
            Ok(other) => warn!("FileManager received unexpected event: {:?}", other),
            Err(_) => info!("FileManager event channel closed"),
        }
    }

    /// Performs automatic saving of chat history at configured intervals
    async fn autosave(&self, temp_dir: &tempfile::TempDir) {
        let save_path = self.default_history_path();
        if let Err(e) = self.save_chat_history(&save_path, temp_dir).await {
            error!("Autosave failed: {}", e);
        }
    }

    /// Saves chat history to disk using atomic write operations
    /// 
    /// 1. Write to temp file
    /// 2. Sync data to disk
    /// 3. Rename to final destination
    async fn save_chat_history(
        &self,
        path: &std::path::Path,
        temp_dir: &tempfile::TempDir,
    ) -> Result<(), std::io::Error> {
        let temp_path = temp_dir.path().join("history.md.tmp");
        let mut temp_file = fs::File::create(&temp_path).await?;

        // Serialize history to Markdown format
        let history_guard = self.state.chat.0.read().await;
        let content = history_guard.format_for_persistence();

        // Write contents to temp file
        temp_file.write_all(content.as_bytes()).await?;
        temp_file.sync_all().await?;  // Ensure data is flushed
        drop(temp_file);  // Close file before rename

        // Atomic rename operation
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

