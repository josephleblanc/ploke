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
use serde::{Deserialize, Serialize};
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
pub(crate) struct Def {
    id: String,
    version: String,
    text: String,
}

impl Def {
    pub(crate) fn new(
        id: impl Into<String>,
        version: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            text: text.into(),
        }
    }

    fn parts(&self) -> [&str; 3] {
        [&self.id, &self.version, &self.text]
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn version(&self) -> &str {
        &self.version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    Named(Def),
    Inline(Def),
    Derived(Def),
}

impl Source {
    pub(crate) fn named(
        id: impl Into<String>,
        version: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self::Named(Def::new(id, version, text))
    }

    pub(crate) fn inline(
        id: impl Into<String>,
        version: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self::Inline(Def::new(id, version, text))
    }

    pub(crate) fn derived(
        id: impl Into<String>,
        version: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self::Derived(Def::new(id, version, text))
    }

    fn tag(&self) -> &'static str {
        match self {
            Self::Named(_) => "named",
            Self::Inline(_) => "inline",
            Self::Derived(_) => "derived",
        }
    }

    fn def(&self) -> &Def {
        match self {
            Self::Named(value) | Self::Inline(value) | Self::Derived(value) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GeneratorSourceKind {
    Named,
    Inline,
    Derived,
}

impl GeneratorSourceKind {
    fn of(source: &Source) -> Self {
        match source {
            Source::Named(_) => Self::Named,
            Source::Inline(_) => Self::Inline,
            Source::Derived(_) => Self::Derived,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GeneratorSurfaceVersion {
    pub(crate) projection_id: String,
    pub(crate) projection_hash: String,
    pub(crate) bounds_digest: String,
    pub(crate) source_kind: GeneratorSourceKind,
    pub(crate) source_id: String,
    pub(crate) source_version: String,
}

impl GeneratorSurfaceVersion {
    pub(crate) fn capture(bounds: &Bounds) -> Self {
        let source = bounds.source();
        let def = source.def();
        Self {
            projection_id: bounds.projection().id().to_string(),
            projection_hash: bounds.projection().hash().as_str().to_string(),
            bounds_digest: bounds.digest().as_str().to_string(),
            source_kind: GeneratorSourceKind::of(source),
            source_id: def.id().to_string(),
            source_version: def.version().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rule {
    source: Source,
    digest: Digest,
}

impl Rule {
    pub(crate) fn named(
        id: impl Into<String>,
        version: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        let source = Source::named(id, version, text);
        let digest = Self::compute_digest(&source);
        Self { source, digest }
    }

    pub(crate) fn inline(
        id: impl Into<String>,
        version: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        let source = Source::inline(id, version, text);
        let digest = Self::compute_digest(&source);
        Self { source, digest }
    }

    pub(crate) fn source(&self) -> &Source {
        &self.source
    }

    pub(crate) fn digest(&self) -> &Digest {
        &self.digest
    }

    fn compute_digest(source: &Source) -> Digest {
        let [id, version, text] = source.def().parts();
        Digest::of([
            source.tag().as_bytes(),
            id.as_bytes(),
            version.as_bytes(),
            text.as_bytes(),
        ])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Projection {
    id: String,
    hash: surface::Hash,
    artifact: surface::Ref,
    source: Source,
    rules: Vec<Rule>,
}

impl Projection {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn hash(&self) -> &surface::Hash {
        &self.hash
    }

    pub(crate) fn artifact(&self) -> &surface::Ref {
        &self.artifact
    }

    pub(crate) fn source(&self) -> &Source {
        &self.source
    }

    pub(crate) fn rules(&self) -> &[Rule] {
        &self.rules
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
pub(crate) struct Projector {
    id: String,
    source: Source,
    rules: Vec<Rule>,
}

impl Projector {
    pub(crate) fn new(
        id: impl Into<String>,
        source: Source,
        rules: impl IntoIterator<Item = Rule>,
    ) -> Self {
        Self {
            id: id.into(),
            source,
            rules: rules.into_iter().collect(),
        }
    }

    pub(crate) fn project(&self, graph: &graph::Projection) -> Projection {
        let digest = Self::compute_digest(&self.id, &self.source, &self.rules, graph);
        let hash = surface::Hash::new(digest.as_str());
        Projection {
            id: self.id.clone(),
            hash,
            artifact: graph.artifact().clone(),
            source: self.source.clone(),
            rules: self.rules.clone(),
        }
    }

    fn compute_digest(
        id: &str,
        source: &Source,
        rules: &[Rule],
        graph: &graph::Projection,
    ) -> Digest {
        let mut parts = vec![
            b"projection".to_vec(),
            id.as_bytes().to_vec(),
            format!("{:?}", graph.artifact().id()).into_bytes(),
            graph.artifact().hash().as_str().as_bytes().to_vec(),
        ];
        push_source(&mut parts, source);
        parts.extend(
            rules
                .iter()
                .map(|rule| rule.digest.as_str().as_bytes().to_vec()),
        );
        for span in graph.spans() {
            push_span(&mut parts, span);
        }
        for (parent, children) in graph.edges() {
            parts.push(b"edge".to_vec());
            push_target(&mut parts, parent);
            for child in children {
                push_target(&mut parts, child);
            }
        }
        Digest::of(parts)
    }
}

pub(crate) fn generator_projection(graph_projection: &graph::Projection) -> Projection {
    Projector::new(
        "prototype1:ploke-tui-tools",
        Source::derived(
            "prototype1:ploke-tui-tools",
            "v1",
            "backend-owned single-file edit surface bridge",
        ),
        [Rule::named(
            "prototype1:ploke-tui-tools",
            "v1",
            "crates/ploke-tui/src/tools/** plus documented rag tool files",
        )],
    )
    .project(graph_projection)
}

pub(crate) fn generator_bounds(
    graph_projection: &graph::Projection,
    graph_bounds: graph::Bounds,
) -> Result<Bounds, Error> {
    Bounds::new(generator_projection(graph_projection), graph_bounds)
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
    pub(crate) fn new(projection: Projection, graph: graph::Bounds) -> Result<Self, Error> {
        projection.check_artifact(graph.artifact())?;
        let rules = projection.rules.clone();
        let source = projection.source.clone();
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

    pub(crate) fn touches(
        &self,
        artifact: &surface::Artifact,
        targets: &[graph::Target],
        writes: &[WriteSnippetData],
    ) -> Result<ResolvedTouches, Error> {
        if targets.len() != writes.len() {
            return Err(Error::TargetWriteCountMismatch {
                targets: targets.len(),
                writes: writes.len(),
            });
        }
        let mut touches = Vec::with_capacity(writes.len());
        for (target, write) in targets.iter().cloned().zip(writes) {
            let material = MaterialSpan::from_write(target, write);
            let touch = self.touch(artifact, material, write.replacement.clone())?;
            touches.push(touch);
        }
        Ok(ResolvedTouches { touches })
    }

    fn compute_digest(
        projection: &Projection,
        rules: &[Rule],
        source: &Source,
        graph: &graph::Bounds,
    ) -> Digest {
        let mut parts = vec![
            b"bounds".to_vec(),
            projection.id.as_bytes().to_vec(),
            projection.hash.as_str().as_bytes().to_vec(),
            format!("{:?}", projection.artifact.id()).into_bytes(),
            projection.artifact.hash().as_str().as_bytes().to_vec(),
        ];
        push_source(&mut parts, source);
        parts.extend(
            rules
                .iter()
                .map(|rule| rule.digest.as_str().as_bytes().to_vec()),
        );
        for span in graph.spans() {
            push_span(&mut parts, span);
        }
        Digest::of(parts)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedTouches {
    touches: Vec<surface::Touch>,
}

impl ResolvedTouches {
    pub(crate) fn as_slice(&self) -> &[surface::Touch] {
        &self.touches
    }

    pub(crate) fn into_vec(self) -> Vec<surface::Touch> {
        self.touches
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
pub(crate) struct Apply {
    state: State,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum State {
    Reported {
        proposal: String,
        run: String,
        base: surface::Ref,
        after: surface::Ref,
        touches: Vec<surface::Touch>,
        authority: surface::GrantAuthority,
        check: surface::Check,
        writes: Vec<Write>,
    },
    Applied {
        proposal: String,
        run: String,
        authority: surface::GrantAuthority,
        delta: ArtifactDelta,
        writes: Vec<Write>,
    },
    Rejected {
        proposal: String,
        run: String,
        authority: surface::GrantAuthority,
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
        let authority = check.authority().clone();
        if writes.len() != proposal.touches.len() {
            return Ok(Self {
                state: State::Rejected {
                    proposal: proposal_id,
                    run: run_id,
                    authority,
                    reason: format!(
                        "apply returned {} write results for {} touches",
                        writes.len(),
                        proposal.touches.len()
                    ),
                    writes,
                },
            });
        }
        if writes
            .iter()
            .zip(proposal.touches.iter())
            .any(|(write, touch)| write.span != *touch.span() || write.after.is_err())
        {
            return Ok(Self {
                state: State::Rejected {
                    proposal: proposal_id,
                    run: run_id,
                    authority,
                    reason: "one or more touched spans were rejected".to_string(),
                    writes,
                },
            });
        }

        Ok(Self {
            state: State::Reported {
                proposal: proposal_id,
                run: run_id,
                base: proposal.base,
                after: proposal.after,
                touches: proposal.touches,
                authority,
                check,
                writes,
            },
        })
    }

    pub(crate) fn validate(self, after_artifact: &surface::Artifact) -> Result<Self, Error> {
        let (proposal, run, after, touches, authority, check, writes) = match self.state {
            State::Reported {
                proposal,
                run,
                base: _base,
                after,
                touches,
                authority,
                check,
                writes,
            } => (proposal, run, after, touches, authority, check, writes),
            state => return Ok(Self { state }),
        };

        if after_artifact.reference() != &after {
            return Err(Error::AfterArtifactMismatch {
                expected: after,
                actual: after_artifact.reference().clone(),
            });
        }

        for (touch, write) in touches.iter().zip(writes.iter()) {
            let expected = write.after.as_ref().expect("reported writes are applied");
            let actual = after_artifact
                .file_hash(touch.span().path())
                .ok_or_else(|| Error::MissingAfterHash(touch.span().path().clone()))?;
            if actual != expected {
                return Err(Error::AfterHashMismatch {
                    path: touch.span().path().clone(),
                    expected: expected.clone(),
                    actual: actual.clone(),
                });
            }
        }

        Ok(Self {
            state: State::Applied {
                proposal,
                run,
                authority,
                delta: harness::ArtifactDelta::from_check(check),
                writes,
            },
        })
    }

    pub(crate) fn is_reported(&self) -> bool {
        matches!(self.state, State::Reported { .. })
    }

    pub(crate) fn is_rejected(&self) -> bool {
        matches!(self.state, State::Rejected { .. })
    }

    pub(crate) fn is_applied(&self) -> bool {
        matches!(self.state, State::Applied { .. })
    }

    pub(crate) fn delta(&self) -> Option<&ArtifactDelta> {
        match &self.state {
            State::Applied { delta, .. } => Some(delta),
            State::Reported { .. } | State::Rejected { .. } => None,
        }
    }

    pub(crate) fn authority(&self) -> &surface::GrantAuthority {
        match &self.state {
            State::Reported { authority, .. }
            | State::Applied { authority, .. }
            | State::Rejected { authority, .. } => authority,
        }
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
    #[error("target/write count mismatch: targets={targets}, writes={writes}")]
    TargetWriteCountMismatch { targets: usize, writes: usize },
    #[error("auto-apply is not available through the eval adapter")]
    AutoApply,
    #[error("checked surface does not match proposal")]
    CheckMismatch,
    #[error(
        "after artifact did not match checked result: expected {expected:?}, actual {actual:?}"
    )]
    AfterArtifactMismatch {
        expected: surface::Ref,
        actual: surface::Ref,
    },
    #[error("after artifact has no hash for touched path: {0}")]
    MissingAfterHash(PathBuf),
    #[error("after artifact hash mismatch for {path}: expected {expected:?}, actual {actual:?}")]
    AfterHashMismatch {
        path: PathBuf,
        expected: surface::Hash,
        actual: surface::Hash,
    },
}

fn hash_from_tracking(hash: TrackingHash) -> surface::Hash {
    surface::Hash::new(hash.0.to_string())
}

fn push_source(parts: &mut Vec<Vec<u8>>, source: &Source) {
    let [id, version, text] = source.def().parts();
    parts.push(source.tag().as_bytes().to_vec());
    parts.push(id.as_bytes().to_vec());
    parts.push(version.as_bytes().to_vec());
    parts.push(text.as_bytes().to_vec());
}

fn push_target(parts: &mut Vec<Vec<u8>>, target: &graph::Target) {
    parts.push(target.path().display().to_string().into_bytes());
    parts.push(target.name().as_bytes().to_vec());
}

fn push_span(parts: &mut Vec<Vec<u8>>, span: &graph::Span) {
    parts.push(b"span".to_vec());
    push_target(parts, span.target());
    parts.push(span.path().display().to_string().into_bytes());
    parts.push(span.start().to_string().into_bytes());
    parts.push(span.end().to_string().into_bytes());
    parts.push(span.hash().as_str().as_bytes().to_vec());
}
