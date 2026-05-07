#![allow(dead_code)] // Phase 2 boundary; live loop wiring is intentionally deferred.

//! Eval-owned adapter boundary for `ploke-tui` edit proposals and `ploke-db`
//! resolved material spans.
//!
//! This module wraps lower proposal/request/material shapes without giving them
//! authority. `ploke-eval` binds the projection to an Artifact, checks rule and
//! bounds digests, converts material spans into `surface::Touch` only after
//! hash validation, and admits apply evidence only as all-applied or rejected.

use std::path::{Path, PathBuf};

use ploke_core::{EmbeddingData, TrackingHash, WriteSnippetData};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

use super::{ArtifactDelta, graph, harness, surface};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Digest(String);

impl Digest {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn of(parts: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Self {
        let mut hasher = Sha256::new();
        for part in parts {
            hasher.update(part.as_ref());
            hasher.update([0]);
        }
        Self(format!("{:x}", hasher.finalize()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Named(String),
    Inline(String),
    Derived(String),
}

impl Source {
    fn tag(&self) -> &'static str {
        match self {
            Self::Named(_) => "named",
            Self::Inline(_) => "inline",
            Self::Derived(_) => "derived",
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Named(value) | Self::Inline(value) | Self::Derived(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rule {
    source: Source,
    digest: Digest,
}

impl Rule {
    pub(crate) fn named(name: impl Into<String>) -> Self {
        let source = Source::Named(name.into());
        let digest = Digest::of(["named".as_bytes(), source.label().as_bytes()]);
        Self { source, digest }
    }

    pub(crate) fn inline(source: impl Into<String>) -> Self {
        let source = Source::Inline(source.into());
        let digest = Digest::of(["inline".as_bytes(), source.label().as_bytes()]);
        Self { source, digest }
    }

    pub(crate) fn source(&self) -> &Source {
        &self.source
    }

    pub(crate) fn digest(&self) -> &Digest {
        &self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Projection {
    id: String,
    hash: surface::Hash,
    artifact: surface::Ref,
}

impl Projection {
    pub(crate) fn new(id: impl Into<String>, hash: surface::Hash, artifact: surface::Ref) -> Self {
        Self {
            id: id.into(),
            hash,
            artifact,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn hash(&self) -> &surface::Hash {
        &self.hash
    }

    pub(crate) fn artifact(&self) -> &surface::Ref {
        &self.artifact
    }

    fn check_artifact(&self, artifact: &surface::Ref) -> Result<(), Error> {
        if &self.artifact == artifact {
            Ok(())
        } else {
            Err(Error::StaleProjection)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Bounds {
    projection: Projection,
    rules: Vec<Rule>,
    source: Source,
    digest: Digest,
    graph: graph::Bounds,
}

impl Bounds {
    pub(crate) fn new(
        projection: Projection,
        rules: impl IntoIterator<Item = Rule>,
        source: Source,
        graph: graph::Bounds,
    ) -> Result<Self, Error> {
        projection.check_artifact(graph.artifact())?;
        let rules = rules.into_iter().collect::<Vec<_>>();
        let digest = Self::compute_digest(&projection, &rules, &source, &graph);
        Ok(Self {
            projection,
            rules,
            source,
            digest,
            graph,
        })
    }

    pub(crate) fn projection(&self) -> &Projection {
        &self.projection
    }

    pub(crate) fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub(crate) fn source(&self) -> &Source {
        &self.source
    }

    pub(crate) fn digest(&self) -> &Digest {
        &self.digest
    }

    pub(crate) fn graph(&self) -> &graph::Bounds {
        &self.graph
    }

    pub(crate) fn touch(
        &self,
        artifact: &surface::Artifact,
        material: MaterialSpan,
        replacement: impl Into<String>,
    ) -> Result<surface::Touch, Error> {
        self.projection.check_artifact(artifact.reference())?;
        let actual = artifact
            .file_hash(material.path())
            .ok_or_else(|| Error::MissingMaterialHash(material.path().to_path_buf()))?;
        if actual != material.expected_hash() {
            return Err(Error::ExpectedHashMismatch {
                path: material.path().to_path_buf(),
                expected: material.expected_hash().clone(),
                actual: actual.clone(),
            });
        }

        let span = material.into_span();
        if !self.graph.contains(&span) {
            return Err(Error::OutsideBounds(span.target().clone()));
        }
        Ok(surface::Touch::new(span, replacement))
    }

    fn compute_digest(
        projection: &Projection,
        rules: &[Rule],
        source: &Source,
        graph: &graph::Bounds,
    ) -> Digest {
        let mut parts = vec![
            projection.id.as_bytes().to_vec(),
            projection.hash.as_str().as_bytes().to_vec(),
            format!("{:?}", projection.artifact.id()).into_bytes(),
            projection.artifact.hash().as_str().as_bytes().to_vec(),
            source.tag().as_bytes().to_vec(),
            source.label().as_bytes().to_vec(),
        ];
        parts.extend(
            rules
                .iter()
                .map(|rule| rule.digest.as_str().as_bytes().to_vec()),
        );
        for span in graph.spans() {
            parts.push(span.target().name().as_bytes().to_vec());
            parts.push(span.target().path().display().to_string().into_bytes());
            parts.push(span.path().display().to_string().into_bytes());
            parts.push(span.start().to_string().into_bytes());
            parts.push(span.end().to_string().into_bytes());
            parts.push(span.hash().as_str().as_bytes().to_vec());
        }
        Digest::of(parts)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MaterialSource {
    DbExact { relation: String, canon: String },
    TuiSplice,
    TuiCanonical { node_type: String, canon: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MaterialSpan {
    target: graph::Target,
    path: PathBuf,
    start: usize,
    end: usize,
    expected_hash: surface::Hash,
    source: MaterialSource,
}

impl MaterialSpan {
    pub(crate) fn new(
        target: graph::Target,
        path: impl Into<PathBuf>,
        start: usize,
        end: usize,
        expected_hash: surface::Hash,
        source: MaterialSource,
    ) -> Self {
        Self {
            target,
            path: path.into(),
            start,
            end,
            expected_hash,
            source,
        }
    }

    pub(crate) fn from_embedding(
        target: graph::Target,
        relation: impl Into<String>,
        canon: impl Into<String>,
        data: &EmbeddingData,
    ) -> Self {
        Self::new(
            target,
            data.file_path.clone(),
            data.start_byte,
            data.end_byte,
            hash_from_tracking(data.file_tracking_hash),
            MaterialSource::DbExact {
                relation: relation.into(),
                canon: canon.into(),
            },
        )
    }

    pub(crate) fn from_write(target: graph::Target, data: &WriteSnippetData) -> Self {
        Self::new(
            target,
            data.file_path.clone(),
            data.start_byte,
            data.end_byte,
            hash_from_tracking(data.expected_file_hash),
            MaterialSource::TuiSplice,
        )
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn expected_hash(&self) -> &surface::Hash {
        &self.expected_hash
    }

    pub(crate) fn source(&self) -> &MaterialSource {
        &self.source
    }

    fn into_span(self) -> graph::Span {
        graph::Span::new(
            self.target,
            self.path,
            self.start,
            self.end,
            self.expected_hash,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LowerRequest {
    edits: Vec<LowerEdit>,
    confidence: Option<String>,
}

impl LowerRequest {
    pub(crate) fn edits(&self) -> &[LowerEdit] {
        &self.edits
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LowerEdit {
    Canonical {
        file: String,
        canon: String,
        node_type: String,
        replacement: String,
    },
    Splice {
        file_path: PathBuf,
        expected_hash: surface::Hash,
        start: usize,
        end: usize,
        replacement: String,
    },
    Patch {
        file: String,
        diff: String,
    },
}

impl From<&ploke_tui::rag::utils::ApplyCodeEditRequest> for LowerRequest {
    fn from(value: &ploke_tui::rag::utils::ApplyCodeEditRequest) -> Self {
        Self {
            confidence: value.confidence.map(|confidence| confidence.to_string()),
            edits: value.edits.iter().map(LowerEdit::from).collect(),
        }
    }
}

impl From<&ploke_tui::rag::utils::Edit> for LowerEdit {
    fn from(value: &ploke_tui::rag::utils::Edit) -> Self {
        match value {
            ploke_tui::rag::utils::Edit::Canonical {
                file,
                canon,
                node_type,
                code,
            } => Self::Canonical {
                file: file.clone(),
                canon: canon.clone(),
                node_type: node_type.relation_str().to_string(),
                replacement: code.clone(),
            },
            ploke_tui::rag::utils::Edit::Splice {
                file_path,
                expected_file_hash,
                start_byte,
                end_byte,
                replacement,
                ..
            } => Self::Splice {
                file_path: PathBuf::from(file_path),
                expected_hash: hash_from_tracking(*expected_file_hash),
                start: *start_byte as usize,
                end: *end_byte as usize,
                replacement: replacement.clone(),
            },
            ploke_tui::rag::utils::Edit::Patch { file, diff, .. } => Self::Patch {
                file: file.clone(),
                diff: diff.clone(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LowerProposal {
    id: String,
    status: String,
    semantic: bool,
    edit_count: usize,
}

impl From<&ploke_tui::app_state::core::EditProposal> for LowerProposal {
    fn from(value: &ploke_tui::app_state::core::EditProposal) -> Self {
        Self {
            id: value.proposal_id.to_string(),
            status: format!("{:?}", value.status),
            semantic: value.is_semantic,
            edit_count: value.edits.len() + value.edits_ns.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stage<'a> {
    pub(crate) proposal: &'a str,
    pub(crate) run: &'a str,
    pub(crate) base: &'a surface::Ref,
    pub(crate) after: surface::Ref,
    pub(crate) projection: &'a Projection,
    pub(crate) touches: Vec<surface::Touch>,
    pub(crate) auto_apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Proposal {
    id: String,
    run: String,
    base: surface::Ref,
    after: surface::Ref,
    touches: Vec<surface::Touch>,
}

impl Proposal {
    pub(crate) fn stage(input: Stage<'_>) -> Result<Self, Error> {
        if input.auto_apply {
            return Err(Error::AutoApply);
        }
        input.projection.check_artifact(input.base)?;
        Ok(Self {
            id: input.proposal.to_string(),
            run: input.run.to_string(),
            base: input.base.clone(),
            after: input.after,
            touches: input.touches,
        })
    }

    pub(crate) fn draft(&self) -> surface::Draft<'_> {
        surface::Draft {
            proposal: &self.id,
            base: &self.base,
            after: &self.after,
            touches: &self.touches,
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn run(&self) -> &str {
        &self.run
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Write {
    span: graph::Span,
    before: surface::Hash,
    after: Result<surface::Hash, String>,
}

impl Write {
    pub(crate) fn applied(touch: &surface::Touch, after: surface::Hash) -> Self {
        Self {
            span: touch.span().clone(),
            before: touch.span().hash().clone(),
            after: Ok(after),
        }
    }

    pub(crate) fn rejected(touch: &surface::Touch, reason: impl Into<String>) -> Self {
        Self {
            span: touch.span().clone(),
            before: touch.span().hash().clone(),
            after: Err(reason.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Apply {
    Applied {
        proposal: String,
        run: String,
        delta: ArtifactDelta,
        writes: Vec<Write>,
    },
    Rejected {
        proposal: String,
        run: String,
        reason: String,
        writes: Vec<Write>,
    },
}

impl Apply {
    pub(crate) fn from_results(
        proposal: Proposal,
        check: surface::Check,
        writes: Vec<Write>,
    ) -> Result<Self, Error> {
        if !check.matches(
            &proposal.id,
            &proposal.base,
            &proposal.after,
            &proposal.touches,
        ) {
            return Err(Error::CheckMismatch);
        }

        let proposal_id = proposal.id.clone();
        let run_id = proposal.run.clone();
        if writes.len() != proposal.touches.len() {
            return Ok(Self::Rejected {
                proposal: proposal_id,
                run: run_id,
                reason: format!(
                    "apply returned {} write results for {} touches",
                    writes.len(),
                    proposal.touches.len()
                ),
                writes,
            });
        }
        if writes
            .iter()
            .zip(proposal.touches.iter())
            .any(|(write, touch)| write.span != *touch.span() || write.after.is_err())
        {
            return Ok(Self::Rejected {
                proposal: proposal_id,
                run: run_id,
                reason: "one or more touched spans were rejected".to_string(),
                writes,
            });
        }

        Ok(Self::Applied {
            proposal: proposal_id,
            run: run_id,
            delta: harness::ArtifactDelta::from_check(check),
            writes,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("projection is stale for the target artifact")]
    StaleProjection,
    #[error("material file has no hash in artifact: {0}")]
    MissingMaterialHash(PathBuf),
    #[error("expected file hash mismatch for {path}: expected {expected:?}, actual {actual:?}")]
    ExpectedHashMismatch {
        path: PathBuf,
        expected: surface::Hash,
        actual: surface::Hash,
    },
    #[error("canonical target is outside adapter bounds: {0:?}")]
    OutsideBounds(graph::Target),
    #[error("auto-apply is not available through the eval adapter")]
    AutoApply,
    #[error("checked surface does not match proposal")]
    CheckMismatch,
}

fn hash_from_tracking(hash: TrackingHash) -> surface::Hash {
    surface::Hash::new(hash.0.to_string())
}
