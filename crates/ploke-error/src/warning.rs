use super::*;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum WarningError {
    #[error("Unlinked modules detected")]
    UnlinkedModules {
        modules: Vec<String>,
        #[backtrace]
        backtrace: Backtrace,
    },
    
    #[error("Orphaned file: {path}")]
    OrphanFile {
        path: PathBuf,
        #[backtrace]
        backtrace: Backtrace,
    },
    
    #[error("Unresolved reference to {name}")]
    UnresolvedRef {
        name: String,
        location: Option<String>,
    },
}
