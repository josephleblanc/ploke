mod artifact;
mod child_plan;
mod evidence;
mod history;
mod operation;
mod parent_create;
mod runtime;
mod selection;
mod warning;

pub use artifact::*;
pub use child_plan::*;
pub use evidence::*;
pub use history::*;
pub use operation::*;
pub use parent_create::*;
pub use runtime::*;
pub use selection::*;
pub use warning::*;

use ploke_records::ids::ArtifactId;
use ploke_records::invocation::{InvocationRecord, Role};

use crate::RunAttemptEvidence;

/// Immutable read-side graph assembled from one loaded Prototype 1 run.
#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    /// Scheduler/prototype node forest assembled from typed run records.
    ///
    /// This remains available for process/schedule drilldown and future
    /// step-through projections. The default `ploke-egui` canvas is now the
    /// borrowed artifact-first `artifact_tree()` projection instead.
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

impl Graph {
    /// Loaded run-attempt records carried through the graph boundary.
    pub fn run_attempts(&self) -> Option<&RunAttemptEvidence> {
        self.forest
            .as_ref()
            .and_then(|forest| forest.passive_evidence.run_attempts.as_ref())
    }

    /// Invocation records loaded from `nodes/<node>/invocations/<runtime>.json`.
    pub fn invocations(&self) -> impl Iterator<Item = (&str, &InvocationRecord)> {
        self.run_attempts().into_iter().flat_map(|attempts| {
            attempts
                .invocations
                .iter()
                .map(|(path, invocation)| (path.as_str(), invocation))
        })
    }

    /// Invocation records whose persisted role is `child`.
    pub fn child_invocations(&self) -> impl Iterator<Item = (&str, &InvocationRecord)> {
        self.invocations()
            .filter(|(_, invocation)| invocation.role == Role::Child)
    }

    /// Child invocation records whose node or request produced `artifact_id`.
    pub fn child_invocations_for_artifact<'a>(
        &'a self,
        artifact_id: &'a ArtifactId,
    ) -> impl Iterator<Item = (&'a str, &'a InvocationRecord)> + 'a {
        self.child_invocations().filter(move |(_, invocation)| {
            invocation
                .node
                .as_ref()
                .and_then(|node| node.derived_artifact_id.as_ref())
                .is_some_and(|derived| derived == artifact_id)
                || invocation
                    .request
                    .as_ref()
                    .and_then(|request| request.derived_artifact_id.as_ref())
                    .is_some_and(|derived| derived == artifact_id)
        })
    }
}
