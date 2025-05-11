use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum WarningError {
    #[error("Unlinked modules detected: {modules:?}")]
    UnlinkedModules {
        modules: Vec<String>,
    },
    
    #[error("Orphaned file: {path}")]
    OrphanFile {
        path: PathBuf,
    },
    
    #[error("Unresolved reference to {name}")]
    UnresolvedRef {
        name: String,
        location: Option<String>,
    },
}
