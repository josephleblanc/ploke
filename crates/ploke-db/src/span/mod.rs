//! Source code location tracking and span management

mod locator;
mod tracker;

use std::path::PathBuf;

pub use locator::CodeLocation;
pub use tracker::SpanTracker;

/// Represents a span change between versions
#[derive(Debug)]
pub struct SpanChange {
    pub file: PathBuf,
    pub old_span: (usize, usize),
    pub new_span: (usize, usize),
    pub old_text: String,
    pub new_text: String,
}
