/*
// Target module for the actor and message types

pub(crate) struct IoManager {
    // request_receiver: mpsc::Receiver<IoManagerMessage>,
    // semaphore: Arc<Semaphore>,
    // roots: Option<Arc<Vec<PathBuf>>>,
}

pub(crate) enum IoManagerMessage {
    Request(IoRequest),
    Shutdown,
}

pub(crate) enum IoRequest {
    ReadSnippetBatch {
        // requests: Vec<EmbeddingData>,
        // responder: oneshot::Sender<Vec<Result<String, PlokeError>>>,
    },
    ScanChangeBatch {
        // requests: Vec<FileData>,
        // responder: oneshot::Sender<Result<Vec<Option<ChangedFileData>>, PlokeError>>,
    },
}

#[derive(Debug)]
pub(crate) struct OrderedRequest {
    // idx: usize,
    // request: EmbeddingData,
}

impl IoManager {
    // fn new(request_receiver: mpsc::Receiver<IoManagerMessage>) -> Self { ... }
    // fn new_with(request_receiver: mpsc::Receiver<IoManagerMessage>, semaphore_permits: usize, roots: Option<Vec<PathBuf>>) -> Self { ... }
    // async fn run(self) { ... }
    // async fn handle_request(&self, request: IoRequest) { ... }

    // async fn handle_read_snippet_batch(requests: Vec<EmbeddingData>, semaphore: Arc<Semaphore>) -> Vec<Result<String, PlokeError>> { ... }
    // async fn handle_read_snippet_batch_with_roots(requests: Vec<EmbeddingData>, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Vec<Result<String, PlokeError>> { ... }

    // async fn process_file(file_path: PathBuf, requests: Vec<OrderedRequest>, semaphore: Arc<Semaphore>) -> Vec<(usize, Result<String, PlokeError>)> { ... }
    // async fn process_file_with_roots(file_path: PathBuf, requests: Vec<OrderedRequest>, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Vec<(usize, Result<String, PlokeError>)> { ... }

    // async fn handle_scan_batch(requests: Vec<FileData>, semaphore: Arc<Semaphore>) -> Result<Vec<Option<ChangedFileData>>, PlokeError> { ... }
    // async fn handle_scan_batch_with_roots(requests: Vec<FileData>, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Result<Vec<Option<ChangedFileData>>, PlokeError> { ... }

    // async fn check_file_hash(file_data: FileData, semaphore: Arc<Semaphore>) -> Result<Option<ChangedFileData>, PlokeError> { ... }
    // async fn check_file_hash_with_roots(file_data: FileData, semaphore: Arc<Semaphore>, roots: Option<Arc<Vec<PathBuf>>>) -> Result<Option<ChangedFileData>, PlokeError> { ... }
}
*/
