//! Source-reference metadata for catalog entries.

/// Source note and stable lookup key within that note.
///
/// Paper-backed entries use the arXiv id as their key, so source references
/// survive markdown line shifts and heading-title edits. Non-paper entries use
/// their mechanism id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRef {
    pub path: &'static str,
    pub section_key: &'static str,
}
