//! Error types for UI-level operations.
//!
//! This module provides error types that are meant to be displayed to users,
//! with context about what went wrong and how to recover.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A UI-facing error type for operations that fail in user-visible ways.
///
/// This is a simplified version for serialization across the event system.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum UiError {
    /// A generic example error variant.
    ExampleError,
}

impl std::fmt::Display for UiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UiError::ExampleError => write!(f, "Example error occurred"),
        }
    }
}
