use std::path::PathBuf;

/// Non-fatal issues that should be surfaced but allow forward progress.
///
/// Examples include unlinked modules and orphan files.
#[cfg_attr(feature = "diagnostic", derive(miette::Diagnostic))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
