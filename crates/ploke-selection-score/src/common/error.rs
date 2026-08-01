//! Error type shared by small scoring helpers.

/// Errors from scoring helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreError {
    Empty,
    LengthMismatch,
    NonFinite,
    ZeroTotal,
}
