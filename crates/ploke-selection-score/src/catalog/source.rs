//! Source-reference metadata for catalog entries.

/// Inclusive line span in a source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSpan {
    pub start: u32,
    pub end: u32,
}

/// Source note and optional line span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRef {
    pub path: &'static str,
    pub span: Option<LineSpan>,
}
