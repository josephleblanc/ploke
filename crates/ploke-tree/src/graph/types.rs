mod artifact;
mod evidence;
mod history;
mod operation;
mod runtime;
mod selection;
mod warning;

pub use artifact::*;
pub use evidence::*;
pub use history::*;
pub use operation::*;
pub use runtime::*;
pub use selection::*;
pub use warning::*;

/// Immutable read-side graph assembled from one loaded Prototype 1 run.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub history: HistoryIndex,
    pub authority: AuthorityIndex,
    pub artifacts: ArtifactIndex,
    pub runtimes: RuntimeIndex,
    pub candidates: CandidateIndex,
    pub selections: SelectionIndex,
    pub evidence: EvidenceIndex,
    pub warnings: Vec<GraphWarning>,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            history: HistoryIndex::default(),
            authority: AuthorityIndex::default(),
            artifacts: ArtifactIndex::default(),
            runtimes: RuntimeIndex::default(),
            candidates: CandidateIndex::default(),
            selections: SelectionIndex::default(),
            evidence: EvidenceIndex::default(),
            warnings: Vec::new(),
        }
    }
}
