#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;
use uuid::Uuid;

use crate::cli::prototype1_state::history::EvidenceRef;
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId};

use super::{diagnosis, graph};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Hash(String);

impl Hash {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Ref {
    id: ArtifactId,
    hash: Hash,
}

impl Ref {
    pub(crate) fn new(id: ArtifactId, hash: Hash) -> Self {
        Self { id, hash }
    }

    pub(crate) fn id(&self) -> &ArtifactId {
        &self.id
    }

    pub(crate) fn hash(&self) -> &Hash {
        &self.hash
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SurfacePolicyId(String);

impl SurfacePolicyId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Artifact {
    reference: Ref,
    files: BTreeMap<PathBuf, Hash>,
}

impl Artifact {
    pub(crate) fn new(reference: Ref, files: impl IntoIterator<Item = (PathBuf, Hash)>) -> Self {
        Self {
            reference,
            files: files.into_iter().collect(),
        }
    }

    pub(crate) fn reference(&self) -> &Ref {
        &self.reference
    }

    pub(crate) fn file_hash(&self, path: &Path) -> Option<&Hash> {
        self.files.get(path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Area {
    spans: Vec<graph::Span>,
}

impl Area {
    pub(crate) fn new(spans: impl IntoIterator<Item = graph::Span>) -> Self {
        Self {
            spans: spans.into_iter().collect(),
        }
    }

    fn contains(&self, span: &graph::Span) -> bool {
        self.spans.iter().any(|covered| {
            covered.path() == span.path()
                && covered.start() <= span.start()
                && covered.end() >= span.end()
                && covered.hash() == span.hash()
        })
    }

    fn check(&self, span: &graph::Span) -> Result<(), Error> {
        let covering = self.spans.iter().find(|allowed| {
            allowed.path() == span.path()
                && allowed.start() <= span.start()
                && allowed.end() >= span.end()
        });
        match covering {
            Some(allowed) if allowed.hash() == span.hash() => Ok(()),
            Some(allowed) => Err(Error::HashMismatch {
                path: span.path().clone(),
                expected: allowed.hash().clone(),
                actual: span.hash().clone(),
            }),
            None => Err(Error::OutsideMaterial(span.clone())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Touch {
    span: graph::Span,
    replacement: String,
}

impl Touch {
    pub(crate) fn new(span: graph::Span, replacement: impl Into<String>) -> Self {
        Self {
            span,
            replacement: replacement.into(),
        }
    }

    pub(crate) fn span(&self) -> &graph::Span {
        &self.span
    }

    pub(crate) fn replacement(&self) -> &str {
        &self.replacement
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrantAuthority {
    coordinate: Coordinate,
    policy: SurfacePolicyId,
}

impl GrantAuthority {
    pub(crate) fn new(
        coordinate: Coordinate,
        policy: SurfacePolicyId,
        artifact: &Ref,
    ) -> Result<Self, Error> {
        match &coordinate.target {
            OperationTarget::Artifact { artifact_id } if artifact_id == artifact.id() => {
                Ok(Self { coordinate, policy })
            }
            OperationTarget::Artifact { artifact_id } => Err(Error::CoordinateTargetMismatch {
                grant_artifact: artifact.id().clone(),
                coordinate_artifact: artifact_id.clone(),
            }),
            target => Err(Error::CoordinateTargetUnsupported {
                target: target.clone(),
            }),
        }
    }

    pub(crate) fn coordinate(&self) -> &Coordinate {
        &self.coordinate
    }

    pub(crate) fn policy(&self) -> &SurfacePolicyId {
        &self.policy
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MaterialScope {
    artifact: Ref,
    graph: graph::Bounds,
    write: Area,
    forbidden: Area,
}

impl MaterialScope {
    fn new(
        artifact: Ref,
        graph: graph::Bounds,
        write: Area,
        forbidden: Area,
    ) -> Result<Self, Error> {
        if graph.artifact() != &artifact {
            return Err(Error::ArtifactMismatch);
        }
        Ok(Self {
            artifact,
            graph,
            write,
            forbidden,
        })
    }

    fn artifact(&self) -> &Ref {
        &self.artifact
    }

    fn narrow(&self, graph: graph::Bounds) -> Result<Self, Error> {
        if graph.artifact() != &self.artifact {
            return Err(Error::ArtifactMismatch);
        }
        if !self.graph.covers(&graph) {
            return Err(Error::Widens);
        }
        Ok(Self {
            artifact: self.artifact.clone(),
            graph,
            write: self.write.clone(),
            forbidden: self.forbidden.clone(),
        })
    }

    fn check(&self, draft: &Draft<'_>) -> Result<(), Error> {
        if draft.base != &self.artifact {
            return Err(Error::ArtifactMismatch);
        }
        for touch in draft.touches {
            if !self.graph.contains(touch.span()) {
                return Err(Error::OutsideGraph(touch.span().target().clone()));
            }
            if self.forbidden.contains(touch.span()) {
                return Err(Error::Forbidden(touch.span().clone()));
            }
            self.write.check(touch.span())?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Grant {
    authority: GrantAuthority,
    material: MaterialScope,
}

impl Grant {
    pub(crate) fn for_coordinate(
        coordinate: Coordinate,
        policy: SurfacePolicyId,
        artifact: Ref,
        graph: graph::Bounds,
        write: Area,
    ) -> Result<Self, Error> {
        Self::for_coordinate_with_forbidden(
            coordinate,
            policy,
            artifact,
            graph,
            write,
            Area::new([]),
        )
    }

    pub(crate) fn for_coordinate_with_forbidden(
        coordinate: Coordinate,
        policy: SurfacePolicyId,
        artifact: Ref,
        graph: graph::Bounds,
        write: Area,
        forbidden: Area,
    ) -> Result<Self, Error> {
        let material = MaterialScope::new(artifact, graph, write, forbidden)?;
        let authority = GrantAuthority::new(coordinate, policy, material.artifact())?;
        Ok(Self {
            authority,
            material,
        })
    }

    fn for_objective_with_forbidden(
        objective: &EditObjective,
        artifact: Ref,
        graph: graph::Bounds,
        write: Area,
        forbidden: Area,
    ) -> Result<Self, Error> {
        let material = MaterialScope::new(artifact, graph, write, forbidden)?;
        let authority = objective.grant_authority(material.artifact())?;
        Ok(Self {
            authority,
            material,
        })
    }

    pub(crate) fn narrow(&self, graph: graph::Bounds) -> Result<Self, Error> {
        Ok(Self {
            authority: self.authority.clone(),
            material: self.material.narrow(graph)?,
        })
    }

    pub(crate) fn check(&self, draft: Draft<'_>) -> Result<Check, Error> {
        self.material.check(&draft)?;
        Ok(Check {
            authority: self.authority.clone(),
            proposal: draft.proposal.to_string(),
            base: draft.base.clone(),
            after: draft.after.clone(),
            touches: draft.touches.to_vec(),
        })
    }

    pub(crate) fn authority(&self) -> &GrantAuthority {
        &self.authority
    }

    pub(crate) fn coordinate(&self) -> &Coordinate {
        self.authority.coordinate()
    }

    pub(crate) fn policy(&self) -> &SurfacePolicyId {
        self.authority.policy()
    }
}

pub(crate) struct Draft<'a> {
    pub(crate) proposal: &'a str,
    pub(crate) base: &'a Ref,
    pub(crate) after: &'a Ref,
    pub(crate) touches: &'a [Touch],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Check {
    authority: GrantAuthority,
    proposal: String,
    base: Ref,
    after: Ref,
    touches: Vec<Touch>,
}

impl Check {
    pub(crate) fn matches(
        &self,
        proposal: &str,
        base: &Ref,
        after: &Ref,
        touches: &[Touch],
    ) -> bool {
        self.proposal == proposal
            && &self.base == base
            && &self.after == after
            && self.touches == touches
    }

    pub(crate) fn authority(&self) -> &GrantAuthority {
        &self.authority
    }

    pub(crate) fn coordinate(&self) -> &Coordinate {
        self.authority.coordinate()
    }

    pub(crate) fn policy(&self) -> &SurfacePolicyId {
        self.authority.policy()
    }

    pub(crate) fn into_parts(self) -> (Ref, Ref, Vec<Touch>) {
        (self.base, self.after, self.touches)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectiveKind {
    BroadRulingParent,
    ReduceKnownFailure {
        limiter: diagnosis::Limiter,
        failure_kind: diagnosis::FailureKind,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TargetMetric {
    InvalidEditSurfaceCandidates,
    OperationalAndProtocolScore,
    ChildAcceptanceRate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WritableIntent {
    BroadEditableSurface,
    ToolSurface,
    SemanticResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectiveConstraint {
    PreserveProtectedCore,
    ExactResolutionOnly,
    NoProcessAuthorityExpansion,
    NoPolicySurfaceMutation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SuccessCriterion {
    CandidateGenerationSucceeds,
    ProtectedCoreUntouched,
    EditSurfaceTestsPass,
    MetricImproves(TargetMetric),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectiveSpec {
    summary: String,
    kind: ObjectiveKind,
    target_metric: TargetMetric,
    writable_intent: WritableIntent,
    constraints: Vec<ObjectiveConstraint>,
    success_criteria: Vec<SuccessCriterion>,
    requested_candidates: Option<usize>,
}

impl ObjectiveSpec {
    pub(crate) fn new(
        summary: impl Into<String>,
        kind: ObjectiveKind,
        target_metric: TargetMetric,
        writable_intent: WritableIntent,
    ) -> Self {
        Self {
            summary: summary.into(),
            kind,
            target_metric,
            writable_intent,
            constraints: Vec::new(),
            success_criteria: Vec::new(),
            requested_candidates: None,
        }
    }

    pub(crate) fn with_constraints(
        mut self,
        constraints: impl IntoIterator<Item = ObjectiveConstraint>,
    ) -> Self {
        self.constraints = constraints.into_iter().collect();
        self
    }

    pub(crate) fn with_success_criteria(
        mut self,
        success_criteria: impl IntoIterator<Item = SuccessCriterion>,
    ) -> Self {
        self.success_criteria = success_criteria.into_iter().collect();
        self
    }

    pub(crate) fn with_requested_candidates(mut self, requested_candidates: usize) -> Self {
        self.requested_candidates = Some(requested_candidates);
        self
    }

    pub(crate) fn summary(&self) -> &str {
        &self.summary
    }

    pub(crate) fn kind(&self) -> ObjectiveKind {
        self.kind
    }

    pub(crate) fn target_metric(&self) -> TargetMetric {
        self.target_metric
    }

    pub(crate) fn writable_intent(&self) -> WritableIntent {
        self.writable_intent
    }

    pub(crate) fn constraints(&self) -> &[ObjectiveConstraint] {
        &self.constraints
    }

    pub(crate) fn success_criteria(&self) -> &[SuccessCriterion] {
        &self.success_criteria
    }

    pub(crate) fn requested_candidates(&self) -> Option<usize> {
        self.requested_candidates
    }

    fn surface_policy(&self) -> SurfacePolicyId {
        match self.writable_intent {
            WritableIntent::BroadEditableSurface => SurfacePolicyId::new("surface-policy:broad-v1"),
            WritableIntent::ToolSurface => SurfacePolicyId::new("surface-policy:tool-surface-v1"),
            WritableIntent::SemanticResolution => {
                SurfacePolicyId::new("surface-policy:semantic-resolution-v1")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditObjective {
    spec: ObjectiveSpec,
    diagnosis_refs: Vec<EvidenceRef>,
    context_refs: Vec<EvidenceRef>,
}

impl EditObjective {
    pub(crate) fn new(
        spec: ObjectiveSpec,
        diagnosis_refs: impl IntoIterator<Item = EvidenceRef>,
        context_refs: impl IntoIterator<Item = EvidenceRef>,
    ) -> Self {
        Self {
            spec,
            diagnosis_refs: diagnosis_refs.into_iter().collect(),
            context_refs: context_refs.into_iter().collect(),
        }
    }

    pub(crate) fn spec(&self) -> &ObjectiveSpec {
        &self.spec
    }

    pub(crate) fn intent(&self) -> &str {
        self.spec.summary()
    }

    pub(crate) fn diagnosis_refs(&self) -> &[EvidenceRef] {
        &self.diagnosis_refs
    }

    pub(crate) fn context_refs(&self) -> &[EvidenceRef] {
        &self.context_refs
    }

    fn grant_authority(&self, artifact: &Ref) -> Result<GrantAuthority, Error> {
        GrantAuthority::new(
            Coordinate {
                runtime_id: RuntimeId(Uuid::nil()),
                target: OperationTarget::Artifact {
                    artifact_id: artifact.id().clone(),
                },
            },
            self.spec.surface_policy(),
            artifact,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtectedCore {
    forbidden: Area,
}

impl ProtectedCore {
    pub(crate) fn new(spans: impl IntoIterator<Item = graph::Span>) -> Self {
        Self {
            forbidden: Area::new(spans),
        }
    }

    fn contains(&self, span: &graph::Span) -> bool {
        self.forbidden.contains(span)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditableSurface {
    objective: EditObjective,
    grant: Grant,
}

impl EditableSurface {
    pub(crate) fn broad(
        objective: EditObjective,
        artifact: Ref,
        graph: graph::Bounds,
        protected_core: ProtectedCore,
    ) -> Result<Self, Error> {
        let write = Area::new(
            graph
                .spans()
                .filter(|span| !protected_core.contains(span))
                .cloned(),
        );
        let grant = Grant::for_objective_with_forbidden(
            &objective,
            artifact,
            graph,
            write,
            protected_core.forbidden,
        )?;
        Ok(Self { objective, grant })
    }

    pub(crate) fn objective(&self) -> &EditObjective {
        &self.objective
    }

    pub(crate) fn grant(&self) -> &Grant {
        &self.grant
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SurfaceRequest {
    objective: EditObjective,
    artifact: Ref,
    graph: graph::Bounds,
    protected_core: ProtectedCore,
}

impl SurfaceRequest {
    pub(crate) fn broad(
        objective: EditObjective,
        artifact: Ref,
        graph: graph::Bounds,
        protected_core: ProtectedCore,
    ) -> Self {
        Self {
            objective,
            artifact,
            graph,
            protected_core,
        }
    }

    pub(crate) fn admit(self) -> Result<EditableSurface, Error> {
        EditableSurface::broad(
            self.objective,
            self.artifact,
            self.graph,
            self.protected_core,
        )
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("artifact identity did not match the granted surface")]
    ArtifactMismatch,
    #[error(
        "operation coordinate target artifact {coordinate_artifact} did not match granted artifact {grant_artifact}"
    )]
    CoordinateTargetMismatch {
        grant_artifact: ArtifactId,
        coordinate_artifact: ArtifactId,
    },
    #[error("surface grants only support artifact operation targets, got {target:?}")]
    CoordinateTargetUnsupported { target: OperationTarget },
    #[error("graph bounds would widen the granted surface")]
    Widens,
    #[error("target is outside graph bounds: {0:?}")]
    OutsideGraph(graph::Target),
    #[error("span is inside forbidden protected core: {0:?}")]
    Forbidden(graph::Span),
    #[error("span is outside material writable surface: {0:?}")]
    OutsideMaterial(graph::Span),
    #[error("expected file hash mismatch for {path}: expected {expected:?}, actual {actual:?}")]
    HashMismatch {
        path: PathBuf,
        expected: Hash,
        actual: Hash,
    },
}
