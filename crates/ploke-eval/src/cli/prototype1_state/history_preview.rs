//! Read-only Prototype 1 History preview import.
//!
//! This module does not write sealed [`super::history`] blocks. It reads the
//! current Prototype 1 persistence surface, preserves source refs and payload
//! hashes, and emits a History-shaped preview that can be inspected before live
//! History writes are wired into the Crown handoff path.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use super::event::RecordedAt;
use super::history::{EntryKind, HistoryHash};
use super::journal::{
    ActiveCheckoutAdvancedEntry, BuildEntry, ChildArtifactCommittedEntry, CompletionEntry, Entry,
    JournalEntry, ParentStartedEntry, ReadyEntry, SpawnEntry, SuccessorHandoffEntry,
    prototype1_transition_journal_path,
};
use crate::cli::InspectOutputFormat;
use crate::intervention::{prototype1_branch_registry_path, prototype1_scheduler_path};
use crate::spec::PrepareError;

const SCHEMA_VERSION: &str = "prototype1-history-preview.v1";

/// Importer-facing access to persisted evidence.
///
/// This trait is intentionally narrow and consumer-shaped. It does not model a
/// database or generic storage backend; it names only the evidence reads the
/// preview importer currently performs.
pub(crate) trait EvidenceStore {
    type Error;

    fn transition_journal(&self) -> Result<Vec<Stored<JournalEntry>>, Self::Error>;

    fn documents(&self) -> Result<Vec<Document>, Self::Error>;
}

/// Filesystem-backed evidence store for the current campaign layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FsEvidenceStore {
    manifest_path: PathBuf,
    prototype_root: PathBuf,
}

impl FsEvidenceStore {
    pub(crate) fn new(manifest_path: impl Into<PathBuf>) -> Self {
        let manifest_path = manifest_path.into();
        let prototype_root = prototype_root(&manifest_path);
        Self {
            manifest_path,
            prototype_root,
        }
    }
}

impl EvidenceStore for FsEvidenceStore {
    type Error = PreviewError;

    fn transition_journal(&self) -> Result<Vec<Stored<JournalEntry>>, Self::Error> {
        let path = prototype1_transition_journal_path(&self.manifest_path);
        load_jsonl(&path, EvidenceClass::TransitionJournal)
    }

    fn documents(&self) -> Result<Vec<Document>, Self::Error> {
        let mut documents = Vec::new();

        self.push_document(
            &mut documents,
            EvidenceClass::Scheduler,
            prototype1_scheduler_path(&self.manifest_path),
        )?;
        self.push_document(
            &mut documents,
            EvidenceClass::BranchRegistry,
            prototype1_branch_registry_path(&self.manifest_path),
        )?;

        self.push_json_dir(
            &mut documents,
            EvidenceClass::Evaluation,
            self.prototype_root.join("evaluations"),
        )?;
        self.push_nested_json_dir(
            &mut documents,
            EvidenceClass::Invocation,
            self.prototype_root.join("nodes"),
            "invocations",
        )?;
        self.push_nested_json_dir(
            &mut documents,
            EvidenceClass::AttemptResult,
            self.prototype_root.join("nodes"),
            "results",
        )?;
        self.push_nested_json_dir(
            &mut documents,
            EvidenceClass::SuccessorReady,
            self.prototype_root.join("nodes"),
            "successor-ready",
        )?;
        self.push_nested_json_dir(
            &mut documents,
            EvidenceClass::SuccessorCompletion,
            self.prototype_root.join("nodes"),
            "successor-completion",
        )?;
        self.push_node_named_documents(&mut documents, EvidenceClass::NodeRecord, "node.json")?;
        self.push_node_named_documents(
            &mut documents,
            EvidenceClass::RunnerRequest,
            "runner-request.json",
        )?;
        self.push_node_named_documents(
            &mut documents,
            EvidenceClass::RunnerResult,
            "runner-result.json",
        )?;

        documents.sort_by(|left, right| {
            left.class
                .as_str()
                .cmp(right.class.as_str())
                .then_with(|| left.path.cmp(&right.path))
        });
        Ok(documents)
    }
}

impl FsEvidenceStore {
    fn push_document(
        &self,
        documents: &mut Vec<Document>,
        class: EvidenceClass,
        path: PathBuf,
    ) -> Result<(), PreviewError> {
        if !path.exists() {
            return Ok(());
        }
        documents.push(Document::load(class, path)?);
        Ok(())
    }

    fn push_json_dir(
        &self,
        documents: &mut Vec<Document>,
        class: EvidenceClass,
        dir: PathBuf,
    ) -> Result<(), PreviewError> {
        if !dir.exists() {
            return Ok(());
        }
        for entry in fs::read_dir(&dir).map_err(|source| PreviewError::ReadDir {
            path: dir.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| PreviewError::ReadDir {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                documents.push(Document::load(class, path)?);
            }
        }
        Ok(())
    }

    fn push_nested_json_dir(
        &self,
        documents: &mut Vec<Document>,
        class: EvidenceClass,
        root: PathBuf,
        dirname: &str,
    ) -> Result<(), PreviewError> {
        if !root.exists() {
            return Ok(());
        }
        for node in fs::read_dir(&root).map_err(|source| PreviewError::ReadDir {
            path: root.clone(),
            source,
        })? {
            let node = node.map_err(|source| PreviewError::ReadDir {
                path: root.clone(),
                source,
            })?;
            let dir = node.path().join(dirname);
            self.push_json_dir(documents, class, dir)?;
        }
        Ok(())
    }

    fn push_node_named_documents(
        &self,
        documents: &mut Vec<Document>,
        class: EvidenceClass,
        filename: &str,
    ) -> Result<(), PreviewError> {
        let nodes = self.prototype_root.join("nodes");
        if !nodes.exists() {
            return Ok(());
        }
        for node in fs::read_dir(&nodes).map_err(|source| PreviewError::ReadDir {
            path: nodes.clone(),
            source,
        })? {
            let node = node.map_err(|source| PreviewError::ReadDir {
                path: nodes.clone(),
                source,
            })?;
            self.push_document(documents, class, node.path().join(filename))?;
        }
        Ok(())
    }
}

/// Build a preview from the current filesystem-backed campaign records.
pub(crate) fn build(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<HistoryPreview, PreviewError> {
    let store = FsEvidenceStore::new(manifest_path);
    build_from_store(campaign_id, manifest_path, &store)
}

pub(crate) fn run(
    campaign_id: &str,
    manifest_path: &Path,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    let preview =
        build(campaign_id, manifest_path).map_err(|source| PrepareError::DatabaseSetup {
            phase: "build prototype1 history preview",
            detail: source.to_string(),
        })?;
    match format {
        InspectOutputFormat::Table => preview.print(),
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&preview).map_err(PrepareError::Serialize)?
            );
        }
    }
    Ok(())
}

fn build_from_store<S>(
    campaign_id: &str,
    manifest_path: &Path,
    store: &S,
) -> Result<HistoryPreview, PreviewError>
where
    S: EvidenceStore<Error = PreviewError>,
{
    let journal = store.transition_journal()?;
    let documents = store.documents()?;
    let prototype_root = prototype_root(manifest_path);

    let mut diagnostics = Vec::new();
    let mut entries = Vec::new();
    for stored in &journal {
        entries.push(preview_entry(stored, &mut diagnostics)?);
    }
    let index = EvidenceIndex::from_documents(&documents);
    entries.extend(document_entries(&documents, &index, &mut diagnostics));
    entries.sort_by(|left, right| {
        left.block_height
            .cmp(&right.block_height)
            .then_with(|| left.occurred_at_ms.cmp(&right.occurred_at_ms))
            .then_with(|| left.source.ref_id.cmp(&right.source.ref_id))
    });

    let blocks = provisional_blocks(&entries);
    let sources = source_summary(&journal, &documents);
    let deferred = deferred_documents(&documents);

    Ok(HistoryPreview {
        schema_version: SCHEMA_VERSION,
        generated_at: Utc::now().to_rfc3339(),
        campaign_id: campaign_id.to_string(),
        manifest_path: manifest_path.to_path_buf(),
        prototype_root,
        sources,
        blocks,
        entries,
        deferred,
        diagnostics,
    })
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HistoryPreview {
    schema_version: &'static str,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    sources: Vec<SourceSummary>,
    blocks: Vec<PreviewBlock>,
    entries: Vec<PreviewEntry>,
    deferred: Vec<DeferredEvidence>,
    diagnostics: Vec<PreviewDiagnostic>,
}

impl HistoryPreview {
    fn print(&self) {
        println!("prototype1 history preview");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", self.schema_version);
        println!("generated_at: {}", self.generated_at);
        println!("campaign_id: {}", self.campaign_id);
        println!("manifest: {}", self.manifest_path.display());
        println!("prototype_root: {}", self.prototype_root.display());
        println!("blocks: {}", self.blocks.len());
        println!("entries: {}", self.entries.len());
        println!("deferred: {}", self.deferred.len());
        println!("diagnostics: {}", self.diagnostics.len());
        println!();

        println!("sources");
        println!("{}", "-".repeat(40));
        for source in &self.sources {
            println!(
                "{} count={} treatment={}",
                source.class, source.count, source.treatment
            );
        }
        println!();

        println!("provisional blocks");
        println!("{}", "-".repeat(40));
        for block in &self.blocks {
            println!(
                "height={} entries={} generations={:?} status={}",
                block.block_height,
                block.entry_count,
                block.imported_from_generations,
                block.authority_status
            );
        }
        println!();

        println!("entries by kind");
        println!("{}", "-".repeat(40));
        for (kind, count) in entry_kind_counts(&self.entries) {
            println!("{kind}: {count}");
        }
        println!();

        println!("diagnostics");
        println!("{}", "-".repeat(40));
        if self.diagnostics.is_empty() {
            println!("(none)");
        } else {
            for diagnostic in &self.diagnostics {
                if let Some(source_ref) = diagnostic.source_ref.as_ref() {
                    println!(
                        "{} [{}]: {}",
                        diagnostic.severity, source_ref, diagnostic.message
                    );
                } else {
                    println!("{}: {}", diagnostic.severity, diagnostic.message);
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PreviewBlock {
    lineage_id: String,
    block_height: u64,
    entry_count: usize,
    imported_from_generations: Vec<u32>,
    authority_status: &'static str,
    notes: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewEntry {
    entry_kind: EntryKind,
    subject: String,
    executor: String,
    observer: String,
    recorder: String,
    proposer: String,
    procedure_or_policy: String,
    occurred_at_ms: Option<i64>,
    recorded_at_ms: Option<i64>,
    block_height: u64,
    generation: Option<u32>,
    source: EvidencePointer,
    payload_ref: String,
    payload_hash: HistoryHash,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
    authority: AuthorityPreview,
}

#[derive(Debug, Clone, Serialize)]
struct AuthorityPreview {
    status: &'static str,
    notes: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
struct SourceSummary {
    class: &'static str,
    treatment: &'static str,
    count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct DeferredEvidence {
    class: EvidenceClass,
    source: EvidencePointer,
    treatment: &'static str,
    reason: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct PreviewDiagnostic {
    severity: &'static str,
    source_ref: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct EvidencePointer {
    class: EvidenceClass,
    ref_id: String,
    path: PathBuf,
    line: Option<usize>,
    hash: HistoryHash,
}

impl EvidencePointer {
    pub(crate) fn ref_id(&self) -> &str {
        &self.ref_id
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn hash(&self) -> &HistoryHash {
        &self.hash
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Stored<T> {
    pointer: EvidencePointer,
    item: T,
}

impl<T> Stored<T> {
    pub(crate) fn pointer(&self) -> &EvidencePointer {
        &self.pointer
    }

    pub(crate) fn item(&self) -> &T {
        &self.item
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Document {
    class: EvidenceClass,
    path: PathBuf,
    pointer: EvidencePointer,
    value: Option<Value>,
}

impl Document {
    fn load(class: EvidenceClass, path: PathBuf) -> Result<Self, PreviewError> {
        let bytes = fs::read(&path).map_err(|source| PreviewError::Read {
            path: path.clone(),
            source,
        })?;
        let pointer = EvidencePointer {
            class,
            ref_id: format!("file:{}", path.display()),
            path: path.clone(),
            line: None,
            hash: HistoryHash::of_bytes(&bytes),
        };
        let value = serde_json::from_slice(&bytes).ok();
        Ok(Self {
            class,
            path,
            pointer,
            value,
        })
    }

    pub(crate) fn class(&self) -> EvidenceClass {
        self.class
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn pointer(&self) -> &EvidencePointer {
        &self.pointer
    }

    pub(crate) fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }
}

#[derive(Debug, Default)]
struct EvidenceIndex {
    node_generation: BTreeMap<String, u32>,
    branch_generation: BTreeMap<String, u32>,
    branch_node: BTreeMap<String, String>,
}

impl EvidenceIndex {
    fn from_documents(documents: &[Document]) -> Self {
        let mut index = Self::default();
        for document in documents
            .iter()
            .filter(|document| document.class == EvidenceClass::NodeRecord)
        {
            let Some(value) = document.value.as_ref() else {
                continue;
            };
            let Some(node_id) = str_field(value, "node_id") else {
                continue;
            };
            let generation = u32_field(value, "generation");
            if let Some(generation) = generation {
                index
                    .node_generation
                    .insert(node_id.to_string(), generation);
            }
            if let Some(branch_id) = str_field(value, "branch_id") {
                if let Some(generation) = generation {
                    index
                        .branch_generation
                        .insert(branch_id.to_string(), generation);
                }
                index
                    .branch_node
                    .insert(branch_id.to_string(), node_id.to_string());
            }
        }
        index
    }

    fn generation_for(&self, value: &Value) -> Option<u32> {
        u32_field(value, "generation")
            .or_else(|| {
                str_field(value, "node_id")
                    .and_then(|node_id| self.node_generation.get(node_id).copied())
            })
            .or_else(|| {
                str_field(value, "branch_id")
                    .and_then(|branch_id| self.branch_generation.get(branch_id).copied())
            })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceClass {
    TransitionJournal,
    Evaluation,
    Invocation,
    AttemptResult,
    SuccessorReady,
    SuccessorCompletion,
    Scheduler,
    BranchRegistry,
    NodeRecord,
    RunnerRequest,
    RunnerResult,
}

impl EvidenceClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::TransitionJournal => "transition_journal",
            Self::Evaluation => "evaluation",
            Self::Invocation => "invocation",
            Self::AttemptResult => "attempt_result",
            Self::SuccessorReady => "successor_ready",
            Self::SuccessorCompletion => "successor_completion",
            Self::Scheduler => "scheduler",
            Self::BranchRegistry => "branch_registry",
            Self::NodeRecord => "node_record",
            Self::RunnerRequest => "runner_request",
            Self::RunnerResult => "runner_result",
        }
    }

    fn treatment(self) -> &'static str {
        match self {
            Self::TransitionJournal => "admitted_preview",
            Self::Evaluation => "admitted_preview_raw",
            Self::Invocation => "admitted_preview_raw",
            Self::AttemptResult => "admitted_preview_raw",
            Self::SuccessorReady => "admitted_preview_raw_or_ingress",
            Self::SuccessorCompletion => "admitted_preview_raw",
            Self::Scheduler => "projection_only",
            Self::BranchRegistry => "projection_plus_evidence_refs",
            Self::NodeRecord => "projection_or_degraded_evidence",
            Self::RunnerRequest => "admitted_preview_raw",
            Self::RunnerResult => "projection_unless_attempt_result_missing",
        }
    }
}

fn load_jsonl<T>(path: &Path, class: EvidenceClass) -> Result<Vec<Stored<T>>, PreviewError>
where
    T: serde::de::DeserializeOwned,
{
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path).map_err(|source| PreviewError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut stored = Vec::new();

    for (line_index, line) in reader.lines().enumerate() {
        let line = line.map_err(|source| PreviewError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let line_number = line_index + 1;
        let item = serde_json::from_str(trimmed).map_err(|source| PreviewError::ParseLine {
            path: path.to_path_buf(),
            line_number,
            source,
        })?;
        stored.push(Stored {
            pointer: EvidencePointer {
                class,
                ref_id: format!("file:{}#L{}", path.display(), line_number),
                path: path.to_path_buf(),
                line: Some(line_number),
                hash: HistoryHash::of_bytes(trimmed.as_bytes()),
            },
            item,
        });
    }

    Ok(stored)
}

fn preview_entry(
    stored: &Stored<JournalEntry>,
    diagnostics: &mut Vec<PreviewDiagnostic>,
) -> Result<PreviewEntry, PreviewError> {
    let projection = project_journal_entry(&stored.item);
    let payload_hash =
        HistoryHash::of_domain_json("prototype1.history_preview.entry.v1", &stored.item)?;
    if !projection.missing.is_empty() {
        diagnostics.push(PreviewDiagnostic {
            severity: "warning",
            source_ref: Some(stored.pointer.ref_id.clone()),
            message: projection.missing.join("; "),
        });
    }

    Ok(PreviewEntry {
        entry_kind: projection.entry_kind,
        subject: projection.subject,
        executor: projection.executor,
        observer: projection.observer,
        recorder: "transition-journal.jsonl".to_string(),
        proposer: "history-preview-import".to_string(),
        procedure_or_policy: projection.procedure_or_policy,
        occurred_at_ms: projection.recorded_at.map(|value| value.0),
        recorded_at_ms: projection.recorded_at.map(|value| value.0),
        block_height: projection.generation.unwrap_or_default() as u64,
        generation: projection.generation,
        source: stored.pointer.clone(),
        payload_ref: stored.pointer.ref_id.clone(),
        payload_hash,
        input_refs: projection.input_refs,
        output_refs: projection.output_refs,
        authority: AuthorityPreview {
            status: "degraded_pre_history",
            notes: vec![
                "source predates sealed History blocks",
                "entry is a preview projection, not an admitted History entry",
            ],
        },
    })
}

#[derive(Debug)]
struct JournalProjection {
    entry_kind: EntryKind,
    subject: String,
    executor: String,
    observer: String,
    procedure_or_policy: String,
    generation: Option<u32>,
    recorded_at: Option<RecordedAt>,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
    missing: Vec<String>,
}

fn project_journal_entry(entry: &JournalEntry) -> JournalProjection {
    match entry {
        JournalEntry::ParentStarted(entry) => parent_started(entry),
        JournalEntry::ChildArtifactCommitted(entry) => child_artifact_committed(entry),
        JournalEntry::ActiveCheckoutAdvanced(entry) => active_checkout_advanced(entry),
        JournalEntry::SuccessorHandoff(entry) => successor_handoff(entry),
        JournalEntry::Successor(entry) => successor_record(entry),
        JournalEntry::MaterializeBranch(entry) => materialize_branch(entry),
        JournalEntry::BuildChild(entry) => build_child(entry),
        JournalEntry::SpawnChild(entry) => spawn_child(entry),
        JournalEntry::Child(entry) => child_record(entry),
        JournalEntry::ChildReady(entry) => child_ready(entry),
        JournalEntry::ObserveChild(entry) => observe_child(entry),
    }
}

fn parent_started(entry: &ParentStartedEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!("parent:{}", entry.parent_identity.parent_id),
        executor: format!("process:pid:{}", entry.pid),
        observer: format!("parent:{}", entry.parent_identity.parent_id),
        procedure_or_policy: "prototype1.parent.started".to_string(),
        generation: Some(entry.parent_identity.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![format!("repo_root:{}", entry.repo_root.display())],
        output_refs: vec![format!(
            "parent_identity:{}",
            entry.parent_identity.parent_id
        )],
        missing: vec!["runtime_id is absent from ParentStartedEntry".to_string()],
    }
}

fn child_artifact_committed(entry: &ChildArtifactCommittedEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!("artifact:committed:{}", entry.node_id),
        executor: parent_actor(entry.parent_identity.as_ref()),
        observer: parent_actor(entry.parent_identity.as_ref()),
        procedure_or_policy: "prototype1.artifact.committed".to_string(),
        generation: Some(entry.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![
            format!("branch:{}", entry.child_branch),
            format!("target_relpath:{}", entry.target_relpath.display()),
        ],
        output_refs: vec![
            format!("target_commit:{}", entry.target_commit),
            format!("identity_commit:{}", entry.identity_commit),
        ],
        missing: Vec::new(),
    }
}

fn active_checkout_advanced(entry: &ActiveCheckoutAdvancedEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!("checkout:{}", entry.active_parent_root.display()),
        executor: parent_actor(entry.previous_parent_identity.as_ref()),
        observer: parent_actor(entry.previous_parent_identity.as_ref()),
        procedure_or_policy: "prototype1.checkout.advanced".to_string(),
        generation: Some(entry.selected_parent_identity.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![format!("selected_branch:{}", entry.selected_branch)],
        output_refs: vec![
            format!("installed_commit:{}", entry.installed_commit),
            format!(
                "selected_parent:{}",
                entry.selected_parent_identity.parent_id
            ),
        ],
        missing: Vec::new(),
    }
}

fn successor_handoff(entry: &SuccessorHandoffEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!("successor:{}", entry.runtime_id),
        executor: format!("runtime:{}", entry.runtime_id),
        observer: "previous_parent".to_string(),
        procedure_or_policy: "prototype1.successor.handoff".to_string(),
        generation: None,
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![
            format!("invocation:{}", entry.invocation_path.display()),
            format!("binary:{}", entry.binary_path.display()),
        ],
        output_refs: vec![format!("ready:{}", entry.ready_path.display())],
        missing: vec!["generation is absent from SuccessorHandoffEntry".to_string()],
    }
}

fn successor_record(entry: &super::successor::Record) -> JournalProjection {
    let (entry_kind, procedure_or_policy, output_refs) = match &entry.state {
        super::successor::State::Selected { .. } => (
            EntryKind::Decision,
            "prototype1.successor.selected",
            vec!["selection_decision:inline".to_string()],
        ),
        super::successor::State::Spawned {
            invocation_path,
            ready_path,
            ..
        } => (
            EntryKind::Transition,
            "prototype1.successor.spawned",
            vec![
                format!("invocation:{}", invocation_path.display()),
                format!("ready:{}", ready_path.display()),
            ],
        ),
        super::successor::State::Checkout {
            selected_branch,
            installed_commit,
            ..
        } => (
            EntryKind::Transition,
            "prototype1.successor.checkout",
            vec![
                format!("selected_branch:{selected_branch}"),
                format!(
                    "installed_commit:{}",
                    installed_commit.as_deref().unwrap_or("unknown")
                ),
            ],
        ),
        super::successor::State::Ready { ready_path, .. } => (
            EntryKind::Transition,
            "prototype1.successor.ready",
            vec![format!("ready:{}", ready_path.display())],
        ),
        super::successor::State::TimedOut { ready_path, .. } => (
            EntryKind::Observation,
            "prototype1.successor.timed_out",
            vec![format!("ready:{}", ready_path.display())],
        ),
        super::successor::State::ExitedBeforeReady { .. } => (
            EntryKind::Observation,
            "prototype1.successor.exited_before_ready",
            Vec::new(),
        ),
        super::successor::State::Completed {
            completion_path,
            trace_path,
            ..
        } => {
            let mut refs = vec![format!("completion:{}", completion_path.display())];
            if let Some(trace_path) = trace_path {
                refs.push(format!("trace:{}", trace_path.display()));
            }
            (
                EntryKind::ProcedureRun,
                "prototype1.successor.completed",
                refs,
            )
        }
    };
    JournalProjection {
        entry_kind,
        subject: format!("successor:{}", entry.node_id),
        executor: entry
            .runtime_id
            .map(|runtime_id| format!("runtime:{runtime_id}"))
            .unwrap_or_else(|| "parent_policy".to_string()),
        observer: "transition-journal".to_string(),
        procedure_or_policy: procedure_or_policy.to_string(),
        generation: None,
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![format!("node:{}", entry.node_id)],
        output_refs,
        missing: vec!["generation is absent from successor::Record".to_string()],
    }
}

fn materialize_branch(entry: &Entry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!(
            "surface:{}:{}",
            entry.refs.node_id,
            entry.paths.target_relpath.display()
        ),
        executor: format!("parent_node:{}", entry.refs.node_id),
        observer: format!("parent_node:{}", entry.refs.node_id),
        procedure_or_policy: format!("prototype1.materialize.{:?}", entry.phase).to_lowercase(),
        generation: Some(entry.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: refs_from_transition(&entry.refs, &entry.hashes.source.0),
        output_refs: vec![
            format!("current_hash:{}", entry.hashes.current),
            format!("proposed_hash:{}", entry.hashes.proposed),
            format!("workspace:{}", entry.paths.workspace_root.display()),
        ],
        missing: Vec::new(),
    }
}

fn build_child(entry: &BuildEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::ProcedureRun,
        subject: format!("child_binary:{}", entry.refs.node_id),
        executor: format!("parent_node:{}", entry.refs.node_id),
        observer: format!("parent_node:{}", entry.refs.node_id),
        procedure_or_policy: format!("prototype1.build.{:?}", entry.phase).to_lowercase(),
        generation: Some(entry.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: refs_from_transition(&entry.refs, &entry.hashes.current.0),
        output_refs: vec![
            format!("binary:{}", entry.paths.binary_path.display()),
            format!("result:{:?}", entry.result),
        ],
        missing: Vec::new(),
    }
}

fn spawn_child(entry: &SpawnEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!("child_runtime:{}", entry.runtime_id),
        executor: format!("parent_node:{}", entry.refs.node_id),
        observer: format!("parent_node:{}", entry.refs.node_id),
        procedure_or_policy: format!("prototype1.spawn.{:?}", entry.phase).to_lowercase(),
        generation: Some(entry.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![
            format!("binary:{}", entry.paths.binary_path.display()),
            format!("argv:{:?}", entry.argv),
        ],
        output_refs: vec![
            format!("runtime:{}", entry.runtime_id),
            format!("child_pid:{:?}", entry.child_pid),
            format!("result:{:?}", entry.result),
        ],
        missing: Vec::new(),
    }
}

fn child_record(entry: &super::child::Record) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!("child_runtime:{}", entry.runtime_id()),
        executor: format!("runtime:{}", entry.runtime_id()),
        observer: format!("runtime:{}", entry.runtime_id()),
        procedure_or_policy: format!("prototype1.{}", entry.entry_kind().replace(':', ".")),
        generation: None,
        recorded_at: None,
        input_refs: Vec::new(),
        output_refs: entry
            .result_path(entry.runtime_id())
            .map(|path| vec![format!("runner_result:{}", path.display())])
            .unwrap_or_default(),
        missing: vec![
            "child::Record fields needed for generation/recorded_at are private to child module"
                .to_string(),
        ],
    }
}

fn child_ready(entry: &ReadyEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Transition,
        subject: format!("child_runtime:{}", entry.runtime_id),
        executor: format!("runtime:{}", entry.runtime_id),
        observer: format!("runtime:{}", entry.runtime_id),
        procedure_or_policy: "prototype1.child.ready".to_string(),
        generation: Some(entry.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: refs_from_transition(&entry.refs, "ready"),
        output_refs: vec![format!("pid:{}", entry.pid)],
        missing: Vec::new(),
    }
}

fn observe_child(entry: &CompletionEntry) -> JournalProjection {
    JournalProjection {
        entry_kind: EntryKind::Observation,
        subject: format!("child_runtime:{}", entry.runtime_id),
        executor: format!("parent_node:{}", entry.refs.node_id),
        observer: format!("parent_node:{}", entry.refs.node_id),
        procedure_or_policy: format!("prototype1.observe_child.{:?}", entry.phase).to_lowercase(),
        generation: Some(entry.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![format!(
            "runner_result:{}",
            entry.runner_result_path.display()
        )],
        output_refs: vec![format!("result:{:?}", entry.result)],
        missing: Vec::new(),
    }
}

fn refs_from_transition(refs: &super::event::Refs, source_hash: &str) -> Vec<String> {
    vec![
        format!("campaign:{}", refs.campaign_id),
        format!("node:{}", refs.node_id),
        format!("instance:{}", refs.instance_id),
        format!("source_state:{}", refs.source_state_id),
        format!("branch:{}", refs.branch_id),
        format!("candidate:{}", refs.candidate_id),
        format!("spec:{}", refs.spec_id),
        format!("source_hash:{source_hash}"),
    ]
}

fn parent_actor(identity: Option<&super::identity::ParentIdentity>) -> String {
    identity
        .map(|identity| format!("parent:{}", identity.parent_id))
        .unwrap_or_else(|| "parent:unknown".to_string())
}

fn document_entries(
    documents: &[Document],
    index: &EvidenceIndex,
    diagnostics: &mut Vec<PreviewDiagnostic>,
) -> Vec<PreviewEntry> {
    let mut entries = Vec::new();
    for document in documents {
        if raw_document_import(document.class).is_none() {
            continue;
        }
        let projection = project_document(document, index);
        if projection.value_parse_failed {
            diagnostics.push(PreviewDiagnostic {
                severity: "warning",
                source_ref: Some(document.pointer.ref_id.clone()),
                message: format!(
                    "{} has no parseable JSON object; preserving only raw evidence ref",
                    document.class.as_str()
                ),
            });
        }
        if !projection.missing.is_empty() {
            diagnostics.push(PreviewDiagnostic {
                severity: "info",
                source_ref: Some(document.pointer.ref_id.clone()),
                message: projection.missing.join("; "),
            });
        }
        entries.push(PreviewEntry {
            entry_kind: projection.entry_kind,
            subject: projection.subject,
            executor: projection.executor,
            observer: projection.observer,
            recorder: document.class.as_str().to_string(),
            proposer: "history-preview-import".to_string(),
            procedure_or_policy: projection.procedure_or_policy,
            occurred_at_ms: projection.occurred_at_ms,
            recorded_at_ms: projection.recorded_at_ms,
            block_height: projection.generation.unwrap_or_default() as u64,
            generation: projection.generation,
            source: document.pointer.clone(),
            payload_ref: document.pointer.ref_id.clone(),
            payload_hash: document.pointer.hash.clone(),
            input_refs: projection.input_refs,
            output_refs: projection.output_refs,
            authority: AuthorityPreview {
                status: projection.authority_status,
                notes: projection.authority_notes,
            },
        });
    }
    entries
}

#[derive(Debug)]
struct DocumentProjection {
    entry_kind: EntryKind,
    subject: String,
    executor: String,
    observer: String,
    procedure_or_policy: String,
    generation: Option<u32>,
    occurred_at_ms: Option<i64>,
    recorded_at_ms: Option<i64>,
    input_refs: Vec<String>,
    output_refs: Vec<String>,
    authority_status: &'static str,
    authority_notes: Vec<&'static str>,
    missing: Vec<String>,
    value_parse_failed: bool,
}

fn project_document(document: &Document, index: &EvidenceIndex) -> DocumentProjection {
    let Some(value) = document.value.as_ref().filter(|value| value.is_object()) else {
        let (entry_kind, subject_prefix, procedure) = raw_document_import(document.class)
            .expect("caller filters importable document classes");
        return DocumentProjection {
            entry_kind,
            subject: format!("{subject_prefix}:{}", document.path.display()),
            executor: "unknown_from_raw_import".to_string(),
            observer: "history-preview-import".to_string(),
            procedure_or_policy: procedure.to_string(),
            generation: None,
            occurred_at_ms: None,
            recorded_at_ms: None,
            input_refs: vec![document.pointer.ref_id.clone()],
            output_refs: Vec::new(),
            authority_status: "raw_degraded_pre_history",
            authority_notes: vec![
                "source has not yet been normalized into typed History fields",
                "payload hash and evidence ref are preserved for later import",
            ],
            missing: Vec::new(),
            value_parse_failed: true,
        };
    };

    let mut missing = Vec::new();
    let generation = index.generation_for(value);
    if generation.is_none() {
        missing.push("generation could not be inferred from document or node index".to_string());
    }
    let occurred_at_ms = timestamp_ms(value, &["recorded_at", "created_at", "updated_at"]);
    let recorded_at_ms = timestamp_ms(value, &["recorded_at", "updated_at", "created_at"]);
    let node_id = str_field(value, "node_id")
        .map(ToOwned::to_owned)
        .or_else(|| node_id_from_path(&document.path));
    let runtime_id = str_field(value, "runtime_id")
        .map(ToOwned::to_owned)
        .or_else(|| runtime_id_from_path(document));
    let branch_id = str_field(value, "branch_id").map(ToOwned::to_owned);

    let mut input_refs = vec![document.pointer.ref_id.clone()];
    let mut output_refs = Vec::new();
    push_ref(&mut input_refs, "node", node_id.as_deref());
    push_ref(&mut input_refs, "branch", branch_id.as_deref());
    push_ref(&mut input_refs, "runtime", runtime_id.as_deref());
    push_json_ref(&mut input_refs, value, "journal_path", "journal");
    push_json_ref(&mut input_refs, value, "source_state_id", "source_state");
    push_json_ref(&mut input_refs, value, "base_artifact_id", "base_artifact");
    push_json_ref(&mut input_refs, value, "patch_id", "patch");
    push_json_ref(
        &mut input_refs,
        value,
        "baseline_campaign_id",
        "baseline_campaign",
    );
    push_json_ref(
        &mut input_refs,
        value,
        "treatment_campaign_id",
        "treatment_campaign",
    );
    push_json_ref(
        &mut input_refs,
        value,
        "treatment_campaign_manifest",
        "treatment_manifest",
    );
    push_compared_instance_refs(&mut input_refs, value);

    push_json_ref(
        &mut output_refs,
        value,
        "evaluation_artifact_path",
        "evaluation",
    );
    push_json_ref(
        &mut output_refs,
        value,
        "runner_result_path",
        "runner_result",
    );
    push_json_ref(&mut output_refs, value, "binary_path", "binary");
    push_json_ref(&mut output_refs, value, "target_relpath", "target_relpath");
    push_json_ref(&mut output_refs, value, "status", "status");
    push_json_ref(&mut output_refs, value, "disposition", "disposition");
    push_json_ref(
        &mut output_refs,
        value,
        "overall_disposition",
        "overall_disposition",
    );
    push_json_ref(&mut output_refs, value, "exit_code", "exit_code");
    push_json_ref(&mut output_refs, value, "role", "role");

    match document.class {
        EvidenceClass::Evaluation => DocumentProjection {
            entry_kind: EntryKind::Judgment,
            subject: branch_id
                .as_deref()
                .map(|branch_id| format!("evaluation:{branch_id}"))
                .unwrap_or_else(|| format!("evaluation:{}", file_stem(&document.path))),
            executor: "prototype1-evaluation-procedure".to_string(),
            observer: "history-preview-import".to_string(),
            procedure_or_policy: "prototype1.evaluation.report".to_string(),
            generation,
            occurred_at_ms,
            recorded_at_ms,
            input_refs,
            output_refs,
            authority_status: "degraded_pre_history",
            authority_notes: vec![
                "evaluation artifact predates sealed History blocks",
                "branch judgment is imported with source payload hash",
            ],
            missing,
            value_parse_failed: false,
        },
        EvidenceClass::Invocation => DocumentProjection {
            entry_kind: EntryKind::Transition,
            subject: runtime_id
                .as_deref()
                .map(|runtime_id| format!("runtime:{runtime_id}"))
                .unwrap_or_else(|| format!("invocation:{}", file_stem(&document.path))),
            executor: runtime_id
                .as_deref()
                .map(|runtime_id| format!("runtime:{runtime_id}"))
                .unwrap_or_else(|| "runtime:unknown".to_string()),
            observer: "history-preview-import".to_string(),
            procedure_or_policy: "prototype1.runtime.invocation".to_string(),
            generation,
            occurred_at_ms,
            recorded_at_ms,
            input_refs,
            output_refs,
            authority_status: "degraded_pre_history",
            authority_notes: vec![
                "attempt-scoped invocation contract is evidence, not Crown authority",
            ],
            missing,
            value_parse_failed: false,
        },
        EvidenceClass::AttemptResult | EvidenceClass::RunnerResult => {
            let is_runner_projection = document.class == EvidenceClass::RunnerResult;
            DocumentProjection {
                entry_kind: if is_runner_projection {
                    EntryKind::Observation
                } else {
                    EntryKind::ProcedureRun
                },
                subject: runtime_id
                    .as_deref()
                    .map(|runtime_id| format!("attempt_result:{runtime_id}"))
                    .or_else(|| node_id.as_deref().map(|node_id| format!("node:{node_id}")))
                    .unwrap_or_else(|| format!("attempt_result:{}", file_stem(&document.path))),
                executor: runtime_id
                    .as_deref()
                    .map(|runtime_id| format!("runtime:{runtime_id}"))
                    .unwrap_or_else(|| "runtime:unknown".to_string()),
                observer: if is_runner_projection {
                    "scheduler_latest_result".to_string()
                } else {
                    "history-preview-import".to_string()
                },
                procedure_or_policy: if is_runner_projection {
                    "prototype1.runner_result.latest_projection".to_string()
                } else {
                    "prototype1.attempt.result".to_string()
                },
                generation,
                occurred_at_ms,
                recorded_at_ms,
                input_refs,
                output_refs,
                authority_status: if is_runner_projection {
                    "projection_degraded_pre_history"
                } else {
                    "degraded_pre_history"
                },
                authority_notes: if is_runner_projection {
                    vec![
                        "latest runner-result copy is mutable projection",
                        "prefer attempt-scoped nodes/*/results/<runtime-id>.json when present",
                    ]
                } else {
                    vec![
                        "attempt-scoped result predates sealed History blocks",
                        "payload hash and source ref are preserved",
                    ]
                },
                missing,
                value_parse_failed: false,
            }
        }
        EvidenceClass::SuccessorReady | EvidenceClass::SuccessorCompletion => DocumentProjection {
            entry_kind: if document.class == EvidenceClass::SuccessorReady {
                EntryKind::Observation
            } else {
                EntryKind::ProcedureRun
            },
            subject: runtime_id
                .as_deref()
                .map(|runtime_id| format!("successor:{runtime_id}"))
                .or_else(|| {
                    node_id
                        .as_deref()
                        .map(|node_id| format!("successor:{node_id}"))
                })
                .unwrap_or_else(|| format!("successor:{}", file_stem(&document.path))),
            executor: runtime_id
                .as_deref()
                .map(|runtime_id| format!("runtime:{runtime_id}"))
                .unwrap_or_else(|| "successor:unknown".to_string()),
            observer: "history-preview-import".to_string(),
            procedure_or_policy: if document.class == EvidenceClass::SuccessorReady {
                "prototype1.successor.ready_file".to_string()
            } else {
                "prototype1.successor.completion_file".to_string()
            },
            generation,
            occurred_at_ms,
            recorded_at_ms,
            input_refs,
            output_refs,
            authority_status: "degraded_pre_history_or_ingress",
            authority_notes: vec![
                "successor evidence may belong to ingress depending on Crown lock timing",
            ],
            missing,
            value_parse_failed: false,
        },
        EvidenceClass::RunnerRequest => DocumentProjection {
            entry_kind: EntryKind::ProcedureRun,
            subject: node_id
                .as_deref()
                .map(|node_id| format!("runner_request:{node_id}"))
                .unwrap_or_else(|| format!("runner_request:{}", file_stem(&document.path))),
            executor: "parent_policy".to_string(),
            observer: "history-preview-import".to_string(),
            procedure_or_policy: "prototype1.runner.request".to_string(),
            generation,
            occurred_at_ms,
            recorded_at_ms,
            input_refs,
            output_refs,
            authority_status: "degraded_pre_history",
            authority_notes: vec!["node execution plan is evidence before sealed History"],
            missing,
            value_parse_failed: false,
        },
        EvidenceClass::NodeRecord => DocumentProjection {
            entry_kind: EntryKind::Projection,
            subject: node_id
                .as_deref()
                .map(|node_id| format!("node:{node_id}"))
                .unwrap_or_else(|| format!("node_record:{}", file_stem(&document.path))),
            executor: "scheduler".to_string(),
            observer: "history-preview-import".to_string(),
            procedure_or_policy: "prototype1.node_record.projection".to_string(),
            generation,
            occurred_at_ms,
            recorded_at_ms,
            input_refs,
            output_refs,
            authority_status: "projection_degraded_pre_history",
            authority_notes: vec![
                "node record is a scheduler mirror and must not override journal evidence",
            ],
            missing,
            value_parse_failed: false,
        },
        EvidenceClass::TransitionJournal
        | EvidenceClass::Scheduler
        | EvidenceClass::BranchRegistry => {
            unreachable!("caller filters non-document-entry classes")
        }
    }
}

fn raw_document_import(class: EvidenceClass) -> Option<(EntryKind, &'static str, &'static str)> {
    match class {
        EvidenceClass::Evaluation => Some((
            EntryKind::Judgment,
            "evaluation",
            "prototype1.evaluation.raw",
        )),
        EvidenceClass::Invocation => Some((
            EntryKind::Transition,
            "invocation",
            "prototype1.invocation.raw",
        )),
        EvidenceClass::AttemptResult => Some((
            EntryKind::ProcedureRun,
            "attempt_result",
            "prototype1.attempt_result.raw",
        )),
        EvidenceClass::SuccessorReady => Some((
            EntryKind::Observation,
            "successor_ready",
            "prototype1.successor_ready.raw",
        )),
        EvidenceClass::SuccessorCompletion => Some((
            EntryKind::ProcedureRun,
            "successor_completion",
            "prototype1.successor_completion.raw",
        )),
        EvidenceClass::RunnerRequest => Some((
            EntryKind::ProcedureRun,
            "runner_request",
            "prototype1.runner_request.raw",
        )),
        EvidenceClass::RunnerResult => Some((
            EntryKind::Observation,
            "runner_result",
            "prototype1.runner_result.raw_degraded",
        )),
        EvidenceClass::NodeRecord => Some((
            EntryKind::Projection,
            "node_record",
            "prototype1.node_record.raw_degraded",
        )),
        EvidenceClass::TransitionJournal
        | EvidenceClass::Scheduler
        | EvidenceClass::BranchRegistry => None,
    }
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
}

fn u32_field(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok())
}

fn timestamp_ms(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        str_field(value, key).and_then(|timestamp| {
            DateTime::parse_from_rfc3339(timestamp)
                .map(|datetime| datetime.timestamp_millis())
                .ok()
        })
    })
}

fn push_ref(refs: &mut Vec<String>, label: &str, value: Option<&str>) {
    if let Some(value) = value {
        refs.push(format!("{label}:{value}"));
    }
}

fn push_json_ref(refs: &mut Vec<String>, value: &Value, key: &str, label: &str) {
    match value.get(key) {
        Some(Value::String(text)) if !text.is_empty() => refs.push(format!("{label}:{text}")),
        Some(Value::Number(number)) => refs.push(format!("{label}:{number}")),
        Some(Value::Bool(flag)) => refs.push(format!("{label}:{flag}")),
        _ => {}
    }
}

fn push_compared_instance_refs(refs: &mut Vec<String>, value: &Value) {
    let Some(instances) = value.get("compared_instances").and_then(Value::as_array) else {
        return;
    };
    for instance in instances {
        push_json_ref(refs, instance, "instance_id", "instance");
        push_json_ref(refs, instance, "baseline_record_path", "baseline_record");
        push_json_ref(refs, instance, "treatment_record_path", "treatment_record");
        if let Some(evaluation) = instance.get("evaluation") {
            push_json_ref(refs, evaluation, "disposition", "evaluation_disposition");
        }
        push_json_ref(refs, instance, "status", "compared_instance_status");
    }
}

fn runtime_id_from_path(document: &Document) -> Option<String> {
    match document.class {
        EvidenceClass::Invocation
        | EvidenceClass::AttemptResult
        | EvidenceClass::SuccessorReady
        | EvidenceClass::SuccessorCompletion => document
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.is_empty())
            .map(ToOwned::to_owned),
        _ => None,
    }
}

fn node_id_from_path(path: &Path) -> Option<String> {
    let mut previous_was_nodes = false;
    for component in path.components() {
        let text = component.as_os_str().to_str()?;
        if previous_was_nodes {
            return Some(text.to_string());
        }
        previous_was_nodes = text == "nodes";
    }
    None
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("unknown")
        .to_string()
}

fn deferred_documents(documents: &[Document]) -> Vec<DeferredEvidence> {
    documents
        .iter()
        .filter(|document| raw_document_import(document.class).is_none())
        .map(|document| DeferredEvidence {
            class: document.class,
            source: document.pointer.clone(),
            treatment: document.class.treatment(),
            reason: match document.class {
                EvidenceClass::Scheduler => "mutable scheduler state is projection-only",
                EvidenceClass::BranchRegistry => {
                    "branch registry is a mutable catalog; selected refs need typed import"
                }
                EvidenceClass::TransitionJournal => "transition journal is imported line-by-line",
                _ => "deferred by current import policy",
            },
        })
        .collect()
}

fn provisional_blocks(entries: &[PreviewEntry]) -> Vec<PreviewBlock> {
    let mut heights = Vec::<u64>::new();
    for entry in entries {
        if !heights.contains(&entry.block_height) {
            heights.push(entry.block_height);
        }
    }
    heights.sort_unstable();
    heights
        .into_iter()
        .map(|height| {
            let matching = entries
                .iter()
                .filter(|entry| entry.block_height == height)
                .collect::<Vec<_>>();
            let mut generations = matching
                .iter()
                .filter_map(|entry| entry.generation)
                .collect::<Vec<_>>();
            generations.sort_unstable();
            generations.dedup();
            PreviewBlock {
                lineage_id: "prototype1-preview-lineage".to_string(),
                block_height: height,
                entry_count: matching.len(),
                imported_from_generations: generations,
                authority_status: "provisional_unsealed",
                notes: vec![
                    "preview block groups existing records by observed generation",
                    "not sealed by Crown<Locked>",
                ],
            }
        })
        .collect()
}

fn source_summary(journal: &[Stored<JournalEntry>], documents: &[Document]) -> Vec<SourceSummary> {
    let mut summaries = vec![SourceSummary {
        class: EvidenceClass::TransitionJournal.as_str(),
        treatment: EvidenceClass::TransitionJournal.treatment(),
        count: journal.len(),
    }];
    for class in [
        EvidenceClass::Evaluation,
        EvidenceClass::Invocation,
        EvidenceClass::AttemptResult,
        EvidenceClass::SuccessorReady,
        EvidenceClass::SuccessorCompletion,
        EvidenceClass::Scheduler,
        EvidenceClass::BranchRegistry,
        EvidenceClass::NodeRecord,
        EvidenceClass::RunnerRequest,
        EvidenceClass::RunnerResult,
    ] {
        let count = documents
            .iter()
            .filter(|document| document.class == class)
            .count();
        if count > 0 {
            summaries.push(SourceSummary {
                class: class.as_str(),
                treatment: class.treatment(),
                count,
            });
        }
    }
    summaries
}

fn entry_kind_counts(entries: &[PreviewEntry]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for entry in entries {
        *counts
            .entry(entry_kind_label(&entry.entry_kind))
            .or_insert(0) += 1;
    }
    counts
}

fn entry_kind_label(kind: &EntryKind) -> &'static str {
    match kind {
        EntryKind::Observation => "observation",
        EntryKind::ProcedureRun => "procedure_run",
        EntryKind::Judgment => "judgment",
        EntryKind::Decision => "decision",
        EntryKind::Transition => "transition",
        EntryKind::Projection => "projection",
    }
}

fn prototype_root(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

#[derive(Debug, Error)]
pub(crate) enum PreviewError {
    #[error("failed to read '{path}'")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read directory '{path}'")]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse JSONL record in '{path}' at line {line_number}")]
    ParseLine {
        path: PathBuf,
        line_number: usize,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to serialize History preview")]
    Serialize(#[from] serde_json::Error),

    #[error("failed to hash History preview value")]
    Hash(#[from] super::history::HistoryError),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::cli::prototype1_state::identity::{PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentity};

    fn identity() -> ParentIdentity {
        ParentIdentity {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: "campaign-a".to_string(),
            parent_id: "parent-0".to_string(),
            node_id: "node-0".to_string(),
            generation: 0,
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: "branch-0".to_string(),
            artifact_branch: Some("prototype1-parent-0".to_string()),
            created_at: "2026-04-28T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn preview_imports_transition_journal_lines_with_hashes() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let journal_path = tmp.path().join("prototype1/transition-journal.jsonl");
        fs::create_dir_all(journal_path.parent().unwrap()).expect("journal dir");
        let entry = JournalEntry::ParentStarted(ParentStartedEntry {
            recorded_at: RecordedAt(100),
            campaign_id: "campaign-a".to_string(),
            parent_identity: identity(),
            repo_root: tmp.path().join("repo"),
            handoff_runtime_id: None,
            pid: 42,
        });
        fs::write(
            &journal_path,
            format!("{}\n", serde_json::to_string(&entry).expect("entry json")),
        )
        .expect("journal");

        let preview = build("campaign-a", &manifest).expect("preview");

        assert_eq!(preview.sources[0].class, "transition_journal");
        assert_eq!(preview.sources[0].count, 1);
        assert_eq!(preview.entries.len(), 1);
        assert_eq!(preview.entries[0].subject, "parent:parent-0");
        assert_eq!(preview.entries[0].source.line, Some(1));
        assert_eq!(preview.blocks[0].block_height, 0);
    }

    #[test]
    fn preview_catalogs_adjacent_json_evidence() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let eval_dir = tmp.path().join("prototype1/evaluations");
        fs::create_dir_all(&eval_dir).expect("eval dir");
        fs::write(eval_dir.join("branch-a.json"), "{\"ok\":true}").expect("eval");
        fs::write(
            tmp.path().join("prototype1/scheduler.json"),
            "{\"projection\":true}",
        )
        .expect("scheduler");

        let preview = build("campaign-a", &manifest).expect("preview");

        assert!(
            preview
                .sources
                .iter()
                .any(|source| source.class == "evaluation" && source.count == 1)
        );
        assert!(
            preview
                .entries
                .iter()
                .any(|entry| entry.subject == "evaluation:branch-a")
        );
        assert!(
            preview
                .deferred
                .iter()
                .any(|item| item.class == EvidenceClass::Scheduler)
        );
    }

    #[test]
    fn preview_places_documents_with_node_index_generation() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let node_dir = tmp.path().join("prototype1/nodes/node-a");
        fs::create_dir_all(node_dir.join("invocations")).expect("node dirs");
        fs::write(
            node_dir.join("node.json"),
            serde_json::json!({
                "node_id": "node-a",
                "generation": 2,
                "branch_id": "branch-a",
                "status": "completed",
                "created_at": "2026-04-28T00:00:00Z"
            })
            .to_string(),
        )
        .expect("node record");
        fs::write(
            node_dir.join("invocations/runtime-a.json"),
            serde_json::json!({
                "role": "child",
                "node_id": "node-a",
                "runtime_id": "runtime-a",
                "created_at": "2026-04-28T00:01:00Z"
            })
            .to_string(),
        )
        .expect("invocation");

        let preview = build("campaign-a", &manifest).expect("preview");
        let invocation = preview
            .entries
            .iter()
            .find(|entry| entry.subject == "runtime:runtime-a")
            .expect("invocation entry");

        assert_eq!(invocation.generation, Some(2));
        assert_eq!(invocation.block_height, 2);
        assert_eq!(invocation.executor, "runtime:runtime-a");
    }
}
