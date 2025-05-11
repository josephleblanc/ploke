use super::*;
// crates/error/src/warning.rs
#[derive(Debug, thiserror::Error)]
pub enum WarningError {
    #[error("File not in module tree: {0}")]
    OrphanFile(PathBuf),
    #[error("Unresolved reference: {0}")]
    UnresolvedRef(String),
    // ...
}
