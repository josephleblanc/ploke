use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("Invalid Rust syntax: {0}")]
    SyntaxError(String),

    #[error("Duplicate module path")]
    DuplicateModulePath {
        path: Vec<String>,
        existing_id: String,
        conflicting_id: String,
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
        source: Option<Arc<dyn std::error::Error + Send + Sync + 'static>>,
    },

    #[error("Database corruption detected: {0}")]
    DatabaseCorruption(String),

    #[error("I/O failure on {path:?}: {operation}: {source}")]
    FileOperation {
        operation: &'static str,
        path: PathBuf,
        source: Arc<std::io::Error>,
    },

    #[error("Content changed for {path:?}")]
    ContentMismatch {
        name: String,
        id: uuid::Uuid,
        file_tracking_hash: uuid::Uuid,
        namespace: uuid::Uuid,
        path: PathBuf,
    },

    #[error("Shutdown initiated")]
    ShutdownInitiated,

    #[error("Invalid UTF-8 sequence in {path:?}: {source}")]
    Utf8 {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
    #[error("{msg}")]
    DefaultConfigDir { msg: &'static str },
}

// TODO: Cumbersome, make this better.
impl From<(std::io::Error, &'static str, PathBuf)> for FatalError {
    fn from((source, operation, path): (std::io::Error, &'static str, PathBuf)) -> Self {
        FatalError::FileOperation {
            operation,
            path,
            source: Arc::new(source),
        }
    }
}

impl Clone for FatalError {
    fn clone(&self) -> Self {
        match self {
            Self::SyntaxError(s) => Self::SyntaxError(s.clone()),
            Self::DuplicateModulePath {
                path,
                existing_id,
                conflicting_id,
            } => Self::DuplicateModulePath {
                path: path.clone(),
                existing_id: existing_id.clone(),
                conflicting_id: conflicting_id.clone(),
            },
            Self::UnresolvedReExport {
                import_id,
                target_path,
            } => Self::UnresolvedReExport {
                import_id: import_id.clone(),
                target_path: target_path.clone(),
            },
            Self::RecursionLimit {
                start_node,
                depth,
                limit,
            } => Self::RecursionLimit {
                start_node: start_node.clone(),
                depth: *depth,
                limit: *limit,
            },
            Self::PathResolution { path, source } => Self::PathResolution {
                path: path.clone(),
                source: source.clone(),
            },
            Self::DatabaseCorruption(s) => Self::DatabaseCorruption(s.clone()),
            Self::FileOperation {
                operation,
                path,
                source,
            } => Self::FileOperation {
                operation,
                path: path.clone(),
                source: Arc::clone(source),
            },
            Self::ContentMismatch {
                name,
                id,
                file_tracking_hash,
                namespace,
                path,
            } => Self::ContentMismatch {
                path: path.clone(),
                name: name.clone(),
                id: *id,
                file_tracking_hash: *file_tracking_hash,
                namespace: *namespace,
            },
            Self::ShutdownInitiated => Self::ShutdownInitiated,
            Self::Utf8 { path, source } => Self::Utf8 {
                path: path.clone(),
                source: source.clone(),
            },
            Self::DefaultConfigDir { msg } => Self::DefaultConfigDir { msg },
        }
    }
}
