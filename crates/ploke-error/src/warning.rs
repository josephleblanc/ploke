use std::path::PathBuf;
use std::backtrace::Backtrace;
use std::backtrace::Backtrace;

#[derive(Debug, thiserror::Error)]
pub enum WarningError {
    #[error("Unlinked modules detected: {modules:?}")]
    UnlinkedModules {
        modules: Vec<String>,
        backtrace: Backtrace,
    },

    #[error("Orphaned file: {path}")]
    OrphanFile { path: PathBuf },

    #[error("Unresolved reference to {name}")]
    UnresolvedRef {
        name: String,
        location: Option<String>,
    },
}
