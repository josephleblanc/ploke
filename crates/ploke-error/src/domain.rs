//// Structured, non-fatal domain errors used across subsystems.
//!
//! Prefer DomainError over ad-hoc string variants. These map to
//! [`crate::Severity::Error`] by default, and applications can downgrade
//! or upgrade via an [`crate::policy::ErrorPolicy`].
#[cfg_attr(feature = "diagnostic", derive(miette::Diagnostic))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, thiserror::Error)]
pub enum DomainError {
    #[error("UI error: {message}")]
    Ui { message: String },

    #[error("Transform error: {message}")]
    Transform { message: String },

    #[error("Database error: {message}")]
    Db { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("RAG error: {message}")]
    Rag { message: String },

    #[error("Ingest error: {message}")]
    Ingest { message: String },
}
