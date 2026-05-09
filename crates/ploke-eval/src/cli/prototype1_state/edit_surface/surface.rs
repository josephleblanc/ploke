#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::cli::prototype1_state::history::EvidenceRef;
use crate::loop_graph::ArtifactId;

use super::graph;

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
pub(crate) struct Grant {
    artifact: Ref,
    graph: graph::Bounds,
    write: Area,
    forbidden: Area,
}

impl Grant {
    pub(crate) fn new(artifact: Ref, graph: graph::Bounds, write: Area) -> Result<Self, Error> {
        Self::with_forbidden(artifact, graph, write, Area::new([]))
    }

    pub(crate) fn with_forbidden(
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

    pub(crate) fn narrow(&self, graph: graph::Bounds) -> Result<Self, Error> {
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

    pub(crate) fn check(&self, draft: Draft<'_>) -> Result<Check, Error> {
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
        Ok(Check {
            proposal: draft.proposal.to_string(),
            base: draft.base.clone(),
            after: draft.after.clone(),
            touches: draft.touches.to_vec(),
        })
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

    pub(crate) fn into_parts(self) -> (Ref, Ref, Vec<Touch>) {
        (self.base, self.after, self.touches)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditObjective {
    intent: String,
    diagnosis_refs: Vec<EvidenceRef>,
    context_refs: Vec<EvidenceRef>,
}

impl EditObjective {
    pub(crate) fn new(
        intent: impl Into<String>,
        diagnosis_refs: impl IntoIterator<Item = EvidenceRef>,
        context_refs: impl IntoIterator<Item = EvidenceRef>,
    ) -> Self {
        Self {
            intent: intent.into(),
            diagnosis_refs: diagnosis_refs.into_iter().collect(),
            context_refs: context_refs.into_iter().collect(),
        }
    }

    pub(crate) fn intent(&self) -> &str {
        &self.intent
    }

    pub(crate) fn diagnosis_refs(&self) -> &[EvidenceRef] {
        &self.diagnosis_refs
    }

    pub(crate) fn context_refs(&self) -> &[EvidenceRef] {
        &self.context_refs
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
        let grant = Grant::with_forbidden(artifact, graph, write, protected_core.forbidden)?;
        Ok(Self { objective, grant })
    }

    pub(crate) fn objective(&self) -> &EditObjective {
        &self.objective
    }

    pub(crate) fn grant(&self) -> &Grant {
        &self.grant
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("artifact identity did not match the granted surface")]
    ArtifactMismatch,
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
