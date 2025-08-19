use std::path::PathBuf;

#[derive(Debug, Clone, thiserror::Error)]
pub enum WarningError {
    #[error("Unlinked modules detected: {modules:?}")]
    UnlinkedModules {
        modules: Vec<String>,
        // backtrace: Backtrace,
    },

    #[error("Orphaned file: {path}")]
    OrphanFile { path: PathBuf },

    #[error("Unresolved reference to {name}")]
    UnresolvedRef {
        name: String,
        location: Option<String>,
    },

    #[error("ploke-db error: {0}")]
    PlokeDb(String),
}
