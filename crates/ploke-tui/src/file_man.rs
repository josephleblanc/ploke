use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::system::SystemEvent;

pub struct FileManager {
    io_handle: ploke_io::IoManagerHandle,
    event_rx: broadcast::Receiver<crate::AppEvent>,
}

impl FileManager {
    /// Creates a new FileManager instance
    pub fn new(
        io_handle: ploke_io::IoManagerHandle,
        event_rx: broadcast::Receiver<crate::AppEvent>,
    ) -> Self {
        Self {
            io_handle,
            event_rx,
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
            crate::AppEvent::System(SystemEvent::SaveRequested(content)) => {
                let path = self.default_history_path();
                if let Err(e) = self.save_content(&path, &content).await {
                    error!("Save failed: {}", e);
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
