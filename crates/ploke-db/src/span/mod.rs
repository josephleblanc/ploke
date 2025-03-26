//! Source code location tracking and span management

mod tracker;
mod locator;

pub use tracker::SpanTracker;
pub use locator::CodeLocation;

/// Represents a span change between versions
#[derive(Debug)]
pub struct SpanChange {
    pub file: PathBuf,
    pub old_span: (usize, usize),
    pub new_span: (usize, usize),
    pub old_text: String,
    pub new_text: String,
}
