//// Coarse-grained classification for programmatic handling of errors.
//!
//! Typical mappings:
//! - Warning: non-fatal issues allowing forward progress
//! - Error: failures that should be handled or bubbled up
//! - Fatal: irrecoverable for the current operation
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
    Fatal,
}
