#![allow(dead_code)] // Phase 1 boundary; live adapters are wired in later phases.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use thiserror::Error;

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
}

impl Grant {
    pub(crate) fn new(artifact: Ref, graph: graph::Bounds, write: Area) -> Result<Self, Error> {
        if graph.artifact() != &artifact {
            return Err(Error::ArtifactMismatch);
        }
        Ok(Self {
            artifact,
            graph,
            write,
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

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("artifact identity did not match the granted surface")]
    ArtifactMismatch,
    #[error("graph bounds would widen the granted surface")]
    Widens,
    #[error("target is outside graph bounds: {0:?}")]
    OutsideGraph(graph::Target),
    #[error("span is outside material writable surface: {0:?}")]
    OutsideMaterial(graph::Span),
    #[error("expected file hash mismatch for {path}: expected {expected:?}, actual {actual:?}")]
    HashMismatch {
        path: PathBuf,
        expected: Hash,
        actual: Hash,
    },
}
