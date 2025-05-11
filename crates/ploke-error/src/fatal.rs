// crates/error/src/fatal.rs
#[derive(Debug, thiserror::Error)]
pub enum FatalError {
    #[error("Invalid Rust syntax: {0}")]
    SyntaxError(String),
    #[error("Database corruption detected: {0}")]
    DatabaseCorruption(String),
    // ...
}
