use crate::config::CozoConfig;
use crate::local::LocalEmbedder;
use crate::providers::hugging_face::HuggingFaceBackend;
use crate::providers::openai::OpenAIBackend;
use ploke_core::EmbeddingNode;
use ploke_db::Database;
use ploke_io::IoManagerHandle;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{info_span, instrument};

use crate::{
    cancel_token::CancellationToken,
    error::EmbedError,
};

#[derive(Debug)]
pub struct EmbeddingProcessor {
    source: EmbeddingSource,
}

#[derive(Debug)]
pub enum EmbeddingSource {
    Local(LocalEmbedder),
    HuggingFace(HuggingFaceBackend),
    OpenAI(OpenAIBackend),
    Cozo(CozoBackend),
}

impl EmbeddingProcessor {
    pub fn new(source: EmbeddingSource) -> Self {
        Self { source }
    }

    pub async fn generate_embeddings(
        &self,
        snippets: Vec<String>,
    ) -> Result<Vec<Vec<f32>>, EmbedError> {
        match &self.source {
            EmbeddingSource::Local(backend) => {
                let text_slices: Vec<&str> = snippets.iter().map(|s| s.as_str()).collect();
                Ok(backend.embed_batch(&text_slices)?)
            }
            EmbeddingSource::HuggingFace(backend) => backend.compute_batch(snippets).await,
            EmbeddingSource::OpenAI(backend) => backend.compute_batch(snippets).await,
            EmbeddingSource::Cozo(backend) => backend.compute_batch(snippets).await,
        }
    }

    pub fn dimensions(&self) -> usize {
        match &self.source {
            EmbeddingSource::Local(backend) => backend.dimensions(),
            EmbeddingSource::HuggingFace(backend) => backend.dimensions,
            EmbeddingSource::OpenAI(backend) => backend.dimensions,
            EmbeddingSource::Cozo(backend) => backend.dimensions,
        }
    }
}






// Cozo placeholder backend
#[derive(Debug)]
pub struct CozoBackend {
    endpoint: String,
    dimensions: usize,
}

impl CozoBackend {
    pub fn new(_config: &CozoConfig) -> Self {
        Self {
            endpoint: "https://embedding.cozo.com".to_string(),
            dimensions: 512, // example dimensions
        }
    }

    pub async fn compute_batch(&self, _snippets: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        Err(EmbedError::NotImplemented("Cozo embeddings not implemented".to_string()))
    }
}

pub type IndexProgress = f64;
 // New state to track indexing
 #[derive(Debug, Clone)]
 pub struct IndexingStatus {
     pub status: IndexStatus,
     pub processed: usize,
     pub total: usize,
     pub current_file: Option<PathBuf>,
     pub errors: Vec<String>,
 }

impl IndexingStatus {
    fn calc_progress(&self) -> IndexProgress {
        if self.total == 0 {
            0.0
        } else {
            self.processed as f64 / self.total as f64
        }
    }
}

 #[derive(Debug, Clone, PartialEq)]
 pub enum IndexStatus {
     Idle,
     Running,
     Paused,
     Completed,
     Failed(String),
     Cancelled,
 }

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum IndexerCommand {
    Pause,
    Resume,
    Cancel,
}

#[derive(Debug)]
pub struct IndexerTask {
    pub db: Arc<Database>,
    pub io: IoManagerHandle,
    pub embedding_processor: EmbeddingProcessor,
    pub cancellation_token: CancellationToken,
    pub batch_size: usize,
}

impl IndexerTask {
    // async fn run(&self) -> Result<(), ploke_error::Error> {
    //     while let Some(batch) = self.next_batch().await? {
    //         process_batch(
    //             &self.db,
    //             &self.io,
    //             &self.embedding_processor,
    //             batch,
    //             |current, total| tracing::info!("Indexed {current}/{total}"),
    //         )
    //         .await?;
    //     }
    //     Ok(())
    // }

    pub async fn run(
        &self,
        progress_tx: broadcast::Sender<IndexingStatus>,
        mut control_rx: mpsc::Receiver<IndexerCommand>
    ) -> Result<(), EmbedError> {
        let total = self.db.count_pending_embeddings()?;
        let mut state = IndexingStatus {
            status: IndexStatus::Running,
            processed: 0,
            total,
            current_file: None,
            errors: Vec::new(),
        };

        progress_tx.send(state.clone())?;
        
        while let Some(batch) = self.next_batch().await? {
            // Check for control commands
            if let Ok(cmd) = control_rx.try_recv() {
                match cmd {
                    IndexerCommand::Pause => state.status = IndexStatus::Paused,
                    IndexerCommand::Resume => state.status = IndexStatus::Running,
                    IndexerCommand::Cancel => {
                        state.status = IndexStatus::Cancelled;
                        break;
                    }
                }
                progress_tx.send(state.clone())?;
            }
            
            if state.status != IndexStatus::Running {
                continue;
            }
            
            state.current_file = batch.first().map(|n| n.path.clone());
            progress_tx.send(state.clone())?;
            
            let batch_len = batch.len();
            match process_batch(
                &self.db,
                &self.io,
                &self.embedding_processor,
                batch,
                |current, total| tracing::info!("Indexed {current}/{total}"),
            ).await {
                Ok(_) => state.processed += batch_len,
                Err(e) => state.errors.push(e.to_string()),
            }
            
            progress_tx.send(state.clone())?;
        }
        
        state.status = if state.processed >= state.total {
            IndexStatus::Completed
        } else {
            IndexStatus::Cancelled
        };
        progress_tx.send(state)?;
        Ok(())
    }
    async fn next_batch(&self) -> Result<Option<Vec<EmbeddingNode>>, EmbedError> {
        static LAST_ID: tokio::sync::Mutex<Option<uuid::Uuid>> =
            tokio::sync::Mutex::const_new(None);

        let mut last_id_guard = LAST_ID.lock().await;
        let last_id = last_id_guard.take();

        let batch = self
            .db
            .get_nodes_for_embedding(self.batch_size, last_id)
            .map_err(EmbedError::PlokeCore)?;

        *last_id_guard = batch.last().map(|node| node.id);

        if self.cancellation_token.is_cancelled() {
            return Err(EmbedError::Cancelled("Processing cancelled".into()));
        }

        match batch.is_empty() {
            true => Ok(None),
            false => Ok(Some(batch)),
        }
    }

}

#[instrument(skip_all, fields(batch_size = nodes.len()))]
pub async fn process_batch(
    db: &Database,
    io_manager: &IoManagerHandle,
    embedding_processor: &EmbeddingProcessor,
    nodes: Vec<EmbeddingNode>,
    report_progress: impl Fn(usize, usize) + Send + Sync,
) -> Result<(), EmbedError> {
    let ctx_span = info_span!("process_batch");
    let _guard = ctx_span.enter();

    let total_nodes = nodes.len();
    let snippet_results =
        io_manager
            .get_snippets_batch(nodes.clone())
            .await
            .map_err(|arg0: ploke_io::RecvError| {
                EmbedError::SnippetFetch(ploke_io::IoError::Recv(arg0))
            })?;

    let mut valid_snippets = Vec::new();
    let mut valid_nodes = Vec::new();
    let mut valid_indices = Vec::new();

    for (i, (node, snippet_result)) in nodes.into_iter().zip(snippet_results).enumerate() {
        report_progress(i, total_nodes);
        match snippet_result {
            Ok(snippet) => {
                valid_snippets.push(snippet);
                valid_nodes.push(node);
                valid_indices.push(i);
            }
            Err(e) => tracing::warn!("Snippet error: {:?}", e),
        }
    }

    let embeddings = embedding_processor
        .generate_embeddings(valid_snippets)
        .await?;

    let dims = embedding_processor.dimensions();
    for embedding in &embeddings {
        if embedding.len() != dims {
            return Err(EmbedError::DimensionMismatch {
                expected: dims,
                actual: embedding.len(),
            });
        }
    }

    let updates = valid_nodes
        .into_iter()
        .zip(embeddings)
        .map(|(node, embedding)| (node.id, embedding))
        .collect();

    db.update_embeddings_batch(updates).await?;

    report_progress(total_nodes, total_nodes);
    Ok(())
}

use tokio::sync::{broadcast, mpsc};

