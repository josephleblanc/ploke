use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("Invalid Rust syntax: {0}")]
    SyntaxError(String), // Keeping string since syntax errors are message-based

    #[error("Duplicate module path")]
    DuplicateModulePath {
        path: Vec<String>,
        existing_id: String,
        conflicting_id: String,
        // backtrace: Backtrace,
    },

    #[error("Unresolved re-export")]
    UnresolvedReExport {
        import_id: String,
        target_path: Vec<String>,
    },

    #[error("Recursion limit exceeded")]
    RecursionLimit {
        start_node: String,
        depth: usize,
        limit: usize,
    },

    #[error("Path resolution failed for {path}")]
    PathResolution {
        path: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Database corruption detected: {0}")]
    DatabaseCorruption(String),

    #[error("I/O failure on {path:?}: {operation}: {source}")]
    FileOperation {
        operation: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Content changed for {path:?}")]
    ContentMismatch { path: PathBuf },

    #[error("Shutdown initiated")]
    ShutdownInitiated,
}
