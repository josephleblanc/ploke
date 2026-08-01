//! Catalog entry types.

use super::source::SourceRef;

/// What kind of score-bearing object a mechanism exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MechanismKind {
    Formula,
    Predicate,
    Selector,
    Metric,
    Protocol,
    ComponentSet,
    Unresolved,
}

/// How directly the Rust helper is supported by the formal note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exactness {
    Faithful,
    Interpretive,
    ScopeLimited,
    NotFormalizable,
}

/// Static metadata for one implemented or tracked mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MechanismSpec {
    pub id: &'static str,
    pub paper_id: Option<&'static str>,
    pub name: &'static str,
    pub kind: MechanismKind,
    pub exactness: Exactness,
    pub source: [SourceRef; 1],
}
