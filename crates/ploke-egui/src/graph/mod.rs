//! Core operator graph.
//!
//! The graph keeps the three Prototype 1 mappings visible together:
//!
//! - artifacts related by applied patch operations
//! - runtimes hydrated from artifacts
//! - operations performed by runtimes over target artifacts
//!
//! Records contribute evidence to this graph. They do not become the graph.
//!
//! This module owns semantic run facts for the operator UI. Layout caches,
//! egui widget graphs, diagnostics, and side panels are views over this object.
//! They should borrow from `Graph` when reading semantics and own only the
//! state that belongs to the view itself.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use ploke_records::branch::TreatmentBranchStatus;
use ploke_records::ids::{ArtifactId, BranchId, CandidateId, PatchId, RuntimeId, SchedulerNodeId};

type Map<K, V> = HashMap<K, V>;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    revision: u64,
    candidates: Map<SchedulerNodeId, Candidate>,
    artifacts: Map<ArtifactId, Artifact>,
    edges: Map<EdgeId, Edge>,
    runtimes: Map<RuntimeId, Runtime>,
    operations: Map<OperationId, Operation>,
    relations: Map<RelationId, Relation>,
    evidence: Map<EvidenceId, Evidence>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn artifacts(&self) -> impl Iterator<Item = &Artifact> {
        self.artifacts.values()
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn artifact_count(&self) -> usize {
        self.artifacts.len()
    }

    pub fn candidates(&self) -> impl Iterator<Item = &Candidate> {
        self.candidates.values()
    }

    pub fn candidate_count(&self) -> usize {
        self.candidates.len()
    }

    pub fn edges(&self) -> impl Iterator<Item = &Edge> {
        self.edges.values()
    }

    pub fn edge(&self, id: &EdgeId) -> Option<&Edge> {
        self.edges.get(id)
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn candidate_edge_count(&self) -> usize {
        self.edges
            .values()
            .filter(|edge| edge.kind() == &EdgeKind::CandidateTransition)
            .count()
    }

    pub fn artifact_edge_count(&self) -> usize {
        self.edges
            .values()
            .filter(|edge| matches!(edge.kind(), EdgeKind::ArtifactPatch { .. }))
            .count()
    }

    pub fn runtimes(&self) -> impl Iterator<Item = &Runtime> {
        self.runtimes.values()
    }

    pub fn runtime_count(&self) -> usize {
        self.runtimes.len()
    }

    pub fn operations(&self) -> impl Iterator<Item = &Operation> {
        self.operations.values()
    }

    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    pub fn relations(&self) -> impl Iterator<Item = &Relation> {
        self.relations.values()
    }

    pub fn relation_count(&self) -> usize {
        self.relations.len()
    }

    pub fn evidence(&self) -> impl Iterator<Item = &Evidence> {
        self.evidence.values()
    }

    pub fn evidence_count(&self) -> usize {
        self.evidence.len()
    }

    pub fn insert_artifact(&mut self, artifact: Artifact) -> bool {
        let inserted = self
            .artifacts
            .insert(artifact.id.clone(), artifact)
            .is_none();
        self.bump_if(inserted);
        inserted
    }

    pub fn insert_candidate(&mut self, candidate: Candidate) -> bool {
        let inserted = self
            .candidates
            .insert(candidate.id.clone(), candidate)
            .is_none();
        self.bump_if(inserted);
        inserted
    }

    pub fn insert_edge(&mut self, edge: Edge) -> Result<bool, GraphError> {
        self.validate_edge(&edge)?;

        if self.edges.get(&edge.id) == Some(&edge) {
            return Ok(false);
        }

        self.edges.insert(edge.id.clone(), edge);
        self.bump_if(true);
        Ok(true)
    }

    pub fn insert_patch_edge(
        &mut self,
        id: EdgeId,
        parent: ArtifactId,
        candidate: ArtifactId,
        patch_id: PatchId,
        evidence_id: EvidenceId,
        status: TreatmentBranchStatus,
    ) -> Result<bool, GraphError> {
        let edge = Edge::artifact_patch(id.clone(), parent, candidate)
            .with_status(status)
            .with_patch(PatchEvidence::new(patch_id, evidence_id.clone()))
            .with_evidence(evidence_id.clone());

        let changed = self.insert_edge(edge)?;
        if changed {
            self.insert_evidence(Evidence::new(evidence_id, Subject::Edge(id)));
        }

        Ok(changed)
    }

    pub fn insert_history_succession_edge(
        &mut self,
        id: EdgeId,
        parent: SchedulerNodeId,
        selected: SchedulerNodeId,
        block_height: u64,
        evidence_id: EvidenceId,
    ) -> Result<bool, GraphError> {
        let edge = Edge::history_succession(id.clone(), parent, selected, block_height)
            .with_status(TreatmentBranchStatus::Selected)
            .with_evidence(evidence_id.clone());

        let changed = self.insert_edge(edge)?;
        if changed {
            self.insert_evidence(Evidence::new(evidence_id, Subject::Edge(id)));
        }

        Ok(changed)
    }

    pub fn insert_runtime(&mut self, runtime: Runtime) -> bool {
        let inserted = self.runtimes.insert(runtime.id.clone(), runtime).is_none();
        self.bump_if(inserted);
        inserted
    }

    pub fn insert_operation(&mut self, operation: Operation) -> bool {
        let inserted = self
            .operations
            .insert(operation.id.clone(), operation)
            .is_none();
        self.bump_if(inserted);
        inserted
    }

    pub fn insert_relation(&mut self, relation: Relation) -> bool {
        let inserted = self
            .relations
            .insert(relation.id.clone(), relation)
            .is_none();
        self.bump_if(inserted);
        inserted
    }

    pub fn insert_evidence(&mut self, evidence: Evidence) -> bool {
        let inserted = self
            .evidence
            .insert(evidence.id.clone(), evidence)
            .is_none();
        self.bump_if(inserted);
        inserted
    }

    fn validate_edge(&self, edge: &Edge) -> Result<(), GraphError> {
        self.require_endpoint(edge.id(), Endpoint::Parent, edge.parent())?;
        self.require_endpoint(edge.id(), Endpoint::Candidate, edge.candidate())
    }

    fn require_endpoint(
        &self,
        edge: &EdgeId,
        endpoint: Endpoint,
        target: &EdgeEndpoint,
    ) -> Result<(), GraphError> {
        match target {
            EdgeEndpoint::Artifact(artifact) if self.artifacts.contains_key(artifact) => Ok(()),
            EdgeEndpoint::Candidate(candidate) if self.candidates.contains_key(candidate) => Ok(()),
            _ => Err(GraphError::MissingEndpoint {
                edge: edge.clone(),
                endpoint,
                target: target.clone(),
            }),
        }
    }

    fn bump_if(&mut self, changed: bool) {
        if changed {
            self.revision = self.revision.saturating_add(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoint {
    Parent,
    Candidate,
}

impl fmt::Display for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parent => f.write_str("parent"),
            Self::Candidate => f.write_str("candidate"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    MissingEndpoint {
        edge: EdgeId,
        endpoint: Endpoint,
        target: EdgeEndpoint,
    },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEndpoint {
                edge,
                endpoint,
                target,
            } => write!(
                f,
                "edge {edge} references missing {endpoint} endpoint {target:?}"
            ),
        }
    }
}

impl Error for GraphError {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Candidate {
    id: SchedulerNodeId,
    parent: Option<SchedulerNodeId>,
    branch_id: BranchId,
    candidate_id: CandidateId,
    generation: u32,
    status: TreatmentBranchStatus,
    ruling_epoch: Option<u64>,
    artifact_edge: Option<EdgeId>,
}

impl Candidate {
    pub fn new(
        id: SchedulerNodeId,
        branch_id: BranchId,
        candidate_id: CandidateId,
        generation: u32,
    ) -> Self {
        Self {
            id,
            parent: None,
            branch_id,
            candidate_id,
            generation,
            status: TreatmentBranchStatus::Synthesized,
            ruling_epoch: None,
            artifact_edge: None,
        }
    }

    pub fn with_parent(mut self, parent: Option<SchedulerNodeId>) -> Self {
        self.parent = parent;
        self
    }

    pub fn with_status(mut self, status: TreatmentBranchStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_ruling_epoch(mut self, block_height: u64) -> Self {
        self.ruling_epoch = Some(block_height);
        self
    }

    pub fn with_artifact_edge(mut self, edge: EdgeId) -> Self {
        self.artifact_edge = Some(edge);
        self
    }

    pub fn id(&self) -> &SchedulerNodeId {
        &self.id
    }

    pub fn parent(&self) -> Option<&SchedulerNodeId> {
        self.parent.as_ref()
    }

    pub fn branch_id(&self) -> &BranchId {
        &self.branch_id
    }

    pub fn candidate_id(&self) -> &CandidateId {
        &self.candidate_id
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn status(&self) -> TreatmentBranchStatus {
        self.status
    }

    pub fn ruling_epoch(&self) -> Option<u64> {
        self.ruling_epoch
    }

    pub fn artifact_edge(&self) -> Option<&EdgeId> {
        self.artifact_edge.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Artifact {
    id: ArtifactId,
}

impl Artifact {
    pub fn new(id: ArtifactId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &ArtifactId {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    id: EdgeId,
    parent: EdgeEndpoint,
    candidate: EdgeEndpoint,
    kind: EdgeKind,
    status: TreatmentBranchStatus,
    evidence: Vec<EvidenceId>,
}

impl Edge {
    pub fn candidate_transition(
        id: EdgeId,
        parent: SchedulerNodeId,
        candidate: SchedulerNodeId,
    ) -> Self {
        Self {
            id,
            parent: EdgeEndpoint::Candidate(parent),
            candidate: EdgeEndpoint::Candidate(candidate),
            kind: EdgeKind::CandidateTransition,
            status: TreatmentBranchStatus::Synthesized,
            evidence: Vec::new(),
        }
    }

    pub fn history_succession(
        id: EdgeId,
        parent: SchedulerNodeId,
        selected: SchedulerNodeId,
        block_height: u64,
    ) -> Self {
        Self {
            id,
            parent: EdgeEndpoint::Candidate(parent),
            candidate: EdgeEndpoint::Candidate(selected),
            kind: EdgeKind::HistorySuccession { block_height },
            status: TreatmentBranchStatus::Selected,
            evidence: Vec::new(),
        }
    }

    pub fn artifact_patch(id: EdgeId, parent: ArtifactId, candidate: ArtifactId) -> Self {
        Self {
            id,
            parent: EdgeEndpoint::Artifact(parent),
            candidate: EdgeEndpoint::Artifact(candidate),
            kind: EdgeKind::ArtifactPatch { patch: None },
            status: TreatmentBranchStatus::Synthesized,
            evidence: Vec::new(),
        }
    }

    pub fn with_status(mut self, status: TreatmentBranchStatus) -> Self {
        self.status = status;
        self
    }

    pub fn with_patch(mut self, patch: PatchEvidence) -> Self {
        if let EdgeKind::ArtifactPatch { patch: target } = &mut self.kind {
            *target = Some(patch);
        }
        self
    }

    pub fn with_evidence(mut self, evidence: EvidenceId) -> Self {
        self.evidence.push(evidence);
        self
    }

    pub fn id(&self) -> &EdgeId {
        &self.id
    }

    pub fn parent(&self) -> &EdgeEndpoint {
        &self.parent
    }

    pub fn candidate(&self) -> &EdgeEndpoint {
        &self.candidate
    }

    pub fn kind(&self) -> &EdgeKind {
        &self.kind
    }

    pub fn status(&self) -> TreatmentBranchStatus {
        self.status
    }

    pub fn patch(&self) -> Option<&PatchEvidence> {
        match &self.kind {
            EdgeKind::ArtifactPatch { patch } => patch.as_ref(),
            EdgeKind::CandidateTransition | EdgeKind::HistorySuccession { .. } => None,
        }
    }

    pub fn evidence(&self) -> &[EvidenceId] {
        &self.evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeEndpoint {
    Candidate(SchedulerNodeId),
    Artifact(ArtifactId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    CandidateTransition,
    HistorySuccession { block_height: u64 },
    ArtifactPatch { patch: Option<PatchEvidence> },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PatchEvidence {
    patch_id: PatchId,
    source: EvidenceId,
}

impl PatchEvidence {
    pub fn new(patch_id: PatchId, source: EvidenceId) -> Self {
        Self { patch_id, source }
    }

    pub fn patch_id(&self) -> &PatchId {
        &self.patch_id
    }

    pub fn source(&self) -> &EvidenceId {
        &self.source
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Runtime {
    id: RuntimeId,
}

impl Runtime {
    pub fn new(id: RuntimeId) -> Self {
        Self { id }
    }

    pub fn id(&self) -> &RuntimeId {
        &self.id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Operation {
    id: OperationId,
    generator: RuntimeId,
    target: ArtifactId,
}

impl Operation {
    pub fn new(id: OperationId, generator: RuntimeId, target: ArtifactId) -> Self {
        Self {
            id,
            generator,
            target,
        }
    }

    pub fn id(&self) -> &OperationId {
        &self.id
    }

    pub fn generator(&self) -> &RuntimeId {
        &self.generator
    }

    pub fn target(&self) -> &ArtifactId {
        &self.target
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Relation {
    id: RelationId,
    kind: RelationKind,
}

impl Relation {
    pub fn new(id: RelationId, kind: RelationKind) -> Self {
        Self { id, kind }
    }

    pub fn id(&self) -> &RelationId {
        &self.id
    }

    pub fn kind(&self) -> &RelationKind {
        &self.kind
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationKind {
    ArtifactDerived {
        base: ArtifactId,
        operation: OperationId,
        derived: ArtifactId,
    },
    RuntimeHydrated {
        artifact: ArtifactId,
        runtime: RuntimeId,
    },
    OperationProduced {
        operation: OperationId,
        artifact: ArtifactId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Evidence {
    id: EvidenceId,
    subject: Subject,
}

impl Evidence {
    pub fn new(id: EvidenceId, subject: Subject) -> Self {
        Self { id, subject }
    }

    pub fn id(&self) -> &EvidenceId {
        &self.id
    }

    pub fn subject(&self) -> &Subject {
        &self.subject
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    Candidate(SchedulerNodeId),
    Artifact(ArtifactId),
    Edge(EdgeId),
    Runtime(RuntimeId),
    Operation(OperationId),
    Relation(RelationId),
}

id_type!(EdgeId);
id_type!(OperationId);
id_type!(RelationId);
id_type!(EvidenceId);
