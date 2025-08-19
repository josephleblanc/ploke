use super::*;
use ploke_core::{WriteResult, WriteSnippetData};

/// A handle to the IoManager actor.
/// Used by other parts of the application to send requests.
#[derive(Clone, Debug)]
pub struct IoManagerHandle {
    /// Channel sender to send requests to the IoManager
    pub(crate) request_sender: mpsc::Sender<IoManagerMessage>,
    #[cfg(feature = "watcher")]
    pub(crate) events_tx: Option<tokio::sync::broadcast::Sender<FileChangeEvent>>,
}

impl Default for IoManagerHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl IoManagerHandle {
    /// Spawns the IoManager and returns a handle to it.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);

        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build Tokio runtime"); // TODO: Add proper error handling

            rt.block_on(async {
                let manager = IoManager::new(rx);
                manager.run().await;
            });
        });

        Self {
            request_sender: tx,
            #[cfg(feature = "watcher")]
            events_tx: None,
        }
    }

    /// Create a builder to configure IoManager before starting it.
    pub fn builder() -> IoManagerBuilder {
        IoManagerBuilder::default()
    }

    /// Asynchronously requests a batch of code snippets.
    pub async fn get_snippets_batch(
        &self,
        requests: Vec<EmbeddingData>,
    ) -> Result<Vec<Result<String, PlokeError>>, errors::RecvError> {
        use tracing::{span, Level};
        let tracing_span = span!(Level::TRACE, "get_snippets_batch");
        let _enter = tracing_span.enter();

        let (responder, response_rx) = oneshot::channel();
        let request = IoRequest::ReadSnippetBatch {
            requests,
            responder,
        };
        self.request_sender
            .send(IoManagerMessage::Request(request))
            .await
            .map_err(|_| RecvError::SendError)?;
        response_rx.await.map_err(|_| RecvError::RecvError)
    }

    /// Asynchronously requests a batch of file hash checks.
    pub async fn scan_changes_batch(
        &self,
        requests: Vec<FileData>,
    ) -> Result<Result<Vec<Option<ChangedFileData>>, PlokeError>, IoError> {
        let (responder, response_rx) = oneshot::channel();
        let request = IoRequest::ScanChangeBatch {
            requests,
            responder,
        };
        self.request_sender
            .send(IoManagerMessage::Request(request))
            .await
            .map_err(|_| RecvError::SendError)
            .map_err(IoError::from)?;
        response_rx
            .await
            .map_err(|_| RecvError::RecvError)
            .map_err(IoError::from)
    }

    /// Subscribe to file change events (requires `watcher` feature and enabled watcher).
    #[cfg(feature = "watcher")]
    pub fn subscribe_file_events(&self) -> tokio::sync::broadcast::Receiver<FileChangeEvent> {
        self.events_tx
            .as_ref()
            .expect("Watcher not enabled; use IoManagerBuilder::enable_watcher(true)")
            .subscribe()
    }

    /// Write a batch of snippets to files atomically.
    /// Returns per-request results. Channel errors are mapped to IoError.
    pub async fn write_snippets_batch(
        &self,
        requests: Vec<WriteSnippetData>,
    ) -> Result<Vec<Result<WriteResult, PlokeError>>, IoError> {
        let (responder, response_rx) = oneshot::channel();
        let request = IoRequest::WriteSnippetBatch { requests, responder };
        self.request_sender
            .send(IoManagerMessage::Request(request))
            .await
            .map_err(|_| RecvError::SendError)
            .map_err(IoError::from)?;
        response_rx
            .await
            .map_err(|_| RecvError::RecvError)
            .map_err(IoError::from)
    }

    /// Sends a shutdown signal to the IoManager.
    pub async fn shutdown(&self) {
        let _ = self.request_sender.send(IoManagerMessage::Shutdown).await;
    }
}
