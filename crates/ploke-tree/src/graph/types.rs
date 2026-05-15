mod artifact;
mod child_plan;
mod evidence;
mod history;
mod operation;
mod runtime;
mod selection;
mod warning;

pub use artifact::*;
pub use child_plan::*;
pub use evidence::*;
pub use history::*;
pub use operation::*;
pub use runtime::*;
pub use selection::*;
pub use warning::*;

/// Immutable read-side graph assembled from one loaded Prototype 1 run.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    /// Scheduler/prototype node forest assembled from typed run records.
    ///
    /// This is the default UI topology when present. Lower-granularity artifact
    /// ids remain attached facts rather than the default canvas spine.
    pub forest: Option<crate::RunForest>,
    /// Sealed History is the primary ordering and authority spine.
    pub history: HistoryIndex,
    pub authority: AuthorityIndex,
    /// Recoverable checkout identities observed through History and evidence.
    pub artifacts: ArtifactIndex,
    /// Concrete hydrated executions observed through actors and passive records.
    pub runtimes: RuntimeIndex,
    /// Generative or compositional actions. Evidence may mention these before
    /// they are promoted to core History facts.
    pub operations: OperationIndex,
    /// Selection candidate universes and their set-scoped memberships.
    pub candidates: CandidateIndex,
    pub selections: SelectionIndex,
    /// Parent-published child plans that carry patch/surface details.
    pub child_plans: ChildPlanIndex,
    /// Typed attachments that explain graph objects without replacing History.
    pub evidence: EvidenceIndex,
    pub warnings: Vec<GraphWarning>,
}

impl Default for Graph {
    fn default() -> Self {
        Self {
            forest: None,
            history: HistoryIndex::default(),
            authority: AuthorityIndex::default(),
            artifacts: ArtifactIndex::default(),
            runtimes: RuntimeIndex::default(),
            operations: OperationIndex::default(),
            candidates: CandidateIndex::default(),
            selections: SelectionIndex::default(),
            child_plans: ChildPlanIndex::default(),
            evidence: EvidenceIndex::default(),
            warnings: Vec::new(),
        }
    }
}
