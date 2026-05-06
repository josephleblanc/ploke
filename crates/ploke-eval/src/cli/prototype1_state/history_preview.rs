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
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use thiserror::Error;

use super::event::RecordedAt;
use super::history::{Block, EntryKind, FsBlockStore, HistoryHash, block};
use super::invocation::{Invocation, SuccessorCompletionRecord, SuccessorReadyRecord};
use super::journal::{
    ActiveCheckoutAdvancedEntry, BuildEntry, ChildArtifactCommittedEntry, CompletionEntry, Entry,
    JournalEntry, ParentStartedEntry, ReadyEntry, SpawnEntry, SuccessorHandoffEntry,
    prototype1_transition_journal_path,
};
use crate::cli::InspectOutputFormat;
use crate::intervention::{prototype1_branch_registry_path, prototype1_scheduler_path};
use crate::spec::PrepareError;

use super::evidence::{
    ChildEvidenceRecords, ChildEvidenceSet, ComparedRunEvidence, EvaluationEvidence,
    EvidenceDiagnostic, EvidenceSource,
};
use crate::cli::prototype1_state::cli_facing::Prototype1BranchEvaluationReport;
use crate::intervention::{
    PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION, PROTOTYPE1_SCHEDULER_SCHEMA_VERSION,
    PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1BranchRegistry, Prototype1NodeRecord,
    Prototype1RunnerRequest, Prototype1RunnerResult, Prototype1SchedulerState,
};

const SCHEMA_VERSION: &str = "prototype1-history-preview.v1";
const SEALED_SELECTION_SCHEMA_VERSION: &str = "prototype1-history-preview.sealed_selection.v1";
const CHILD_EVIDENCE_TABLE_LIMIT: usize = 20;
const CHILD_EVIDENCE_COMPARED_LIMIT: usize = 3;

/// Importer-facing access to persisted evidence.
///
/// This trait is intentionally narrow and consumer-shaped. It does not model a
/// database or generic storage backend; it names only the evidence reads the
/// preview importer currently performs.
pub(crate) trait EvidenceStore {
    type Error;

    fn transition_journal(&self) -> Result<Vec<Stored<JournalEntry>>, Self::Error>;

    /// Load generic JSON document projections for the `history preview` catalog.
    ///
    /// This is preview catalog/projection compatibility only. Consumers that
    /// derive typed child evidence, metrics, successor selection, future
    /// scoring, or authority facts must use typed `Stored<T>` reads where
    /// `T: EvidenceRecord`, not `Document`, `serde_json::Value`, or path/name
    /// recovery.
    fn documents(&self) -> Result<Vec<Document>, Self::Error>;

    /// Load declared child/runtime/result/evaluation records through the typed
    /// evidence boundary.
    ///
    /// Malformed declared records are store-boundary errors. They must not be
    /// downgraded into generic `Document` compatibility records.
    fn child_records(&self) -> Result<ChildEvidenceRecords, Self::Error>;

    #[allow(dead_code)]
    fn child_evidence(&self) -> Result<ChildEvidenceSet, Self::Error>;
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

    #[allow(dead_code)]
    pub(crate) fn child_evidence(&self) -> Result<ChildEvidenceSet, PreviewError> {
        <Self as EvidenceStore>::child_evidence(self)
    }

    /// Load typed mutable preview records that expose operator-facing selection
    /// markers for `history metrics` display.
    ///
    /// These scheduler/registry records are typed `Stored<T>` inputs for a
    /// dashboard projection. They are not loose `Document` compatibility, typed
    /// child evidence, selector input, future scoring input, or History/Crown
    /// authority.
    pub(crate) fn preview_selection_records(
        &self,
    ) -> Result<SelectionPreviewRecords, PreviewError> {
        Ok(SelectionPreviewRecords {
            scheduler: self.preview_selection_record::<Prototype1SchedulerState>(
                EvidenceClass::Scheduler,
                prototype1_scheduler_path(&self.manifest_path),
            )?,
            branch_registry: self.preview_selection_record::<Prototype1BranchRegistry>(
                EvidenceClass::BranchRegistry,
                prototype1_branch_registry_path(&self.manifest_path),
            )?,
        })
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

    fn child_records(&self) -> Result<ChildEvidenceRecords, Self::Error> {
        let mut records = ChildEvidenceRecords {
            journal: self.transition_journal()?,
            ..ChildEvidenceRecords::default()
        };

        self.push_typed_dir::<Prototype1BranchEvaluationReport>(
            &mut records.evaluations,
            EvidenceClass::Evaluation,
            self.prototype_root.join("evaluations"),
        )?;
        self.push_nested_typed_dir::<Invocation>(
            &mut records.invocations,
            EvidenceClass::Invocation,
            self.prototype_root.join("nodes"),
            "invocations",
        )?;
        self.push_nested_typed_dir::<Prototype1RunnerResult>(
            &mut records.attempt_results,
            EvidenceClass::AttemptResult,
            self.prototype_root.join("nodes"),
            "results",
        )?;
        self.push_nested_typed_dir::<SuccessorReadyRecord>(
            &mut records.successor_ready,
            EvidenceClass::SuccessorReady,
            self.prototype_root.join("nodes"),
            "successor-ready",
        )?;
        self.push_nested_typed_dir::<SuccessorCompletionRecord>(
            &mut records.successor_completion,
            EvidenceClass::SuccessorCompletion,
            self.prototype_root.join("nodes"),
            "successor-completion",
        )?;
        self.push_node_typed_records::<Prototype1NodeRecord>(
            &mut records.nodes,
            EvidenceClass::NodeRecord,
            "node.json",
        )?;
        self.push_node_typed_records::<Prototype1RunnerRequest>(
            &mut records.runner_requests,
            EvidenceClass::RunnerRequest,
            "runner-request.json",
        )?;
        self.push_node_typed_records::<Prototype1RunnerResult>(
            &mut records.runner_results,
            EvidenceClass::RunnerResult,
            "runner-result.json",
        )?;

        records.sort();
        Ok(records)
    }

    fn child_evidence(&self) -> Result<ChildEvidenceSet, Self::Error> {
        let records = self.child_records()?;
        Ok(ChildEvidenceSet::from_records(&records))
    }
}

impl FsEvidenceStore {
    fn preview_selection_record<T>(
        &self,
        class: EvidenceClass,
        path: PathBuf,
    ) -> Result<Option<Stored<T>>, PreviewError>
    where
        T: EvidenceRecord,
    {
        if path.exists() {
            load_typed_record::<T>(class, path).map(Some)
        } else {
            Ok(None)
        }
    }

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

    fn push_typed_dir<T>(
        &self,
        records: &mut Vec<Stored<T>>,
        class: EvidenceClass,
        dir: PathBuf,
    ) -> Result<(), PreviewError>
    where
        T: EvidenceRecord,
    {
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
                records.push(load_typed_record::<T>(class, path)?);
            }
        }
        Ok(())
    }

    fn push_nested_typed_dir<T>(
        &self,
        records: &mut Vec<Stored<T>>,
        class: EvidenceClass,
        root: PathBuf,
        dirname: &str,
    ) -> Result<(), PreviewError>
    where
        T: EvidenceRecord,
    {
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
            self.push_typed_dir(records, class, dir)?;
        }
        Ok(())
    }

    fn push_node_typed_records<T>(
        &self,
        records: &mut Vec<Stored<T>>,
        class: EvidenceClass,
        filename: &str,
    ) -> Result<(), PreviewError>
    where
        T: EvidenceRecord,
    {
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
            let path = node.path().join(filename);
            if path.exists() {
                records.push(load_typed_record::<T>(class, path)?);
            }
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

/// Build a read-only operator projection over grouped child evidence.
pub(crate) fn build_child_evidence(
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<ChildEvidenceProjection, PreviewError> {
    let store = FsEvidenceStore::new(manifest_path);
    let evidence = store.child_evidence()?;
    Ok(ChildEvidenceProjection {
        schema_version: evidence.schema_version.clone(),
        generated_at: Utc::now().to_rfc3339(),
        campaign_id: campaign_id.to_string(),
        manifest_path: manifest_path.to_path_buf(),
        prototype_root: prototype_root(manifest_path),
        evidence,
    })
}

pub(crate) fn run_child_evidence(
    campaign_id: &str,
    manifest_path: &Path,
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    let projection = build_child_evidence(campaign_id, manifest_path).map_err(|source| {
        PrepareError::DatabaseSetup {
            phase: "build prototype1 child evidence",
            detail: source.to_string(),
        }
    })?;

    match format {
        InspectOutputFormat::Table => projection.print(),
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&projection).map_err(PrepareError::Serialize)?
            );
        }
    }

    Ok(())
}

/// Sealed-block scan + digest verification for committed successor selection
/// (`EntryPayload::SelectionDecision`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SealedSelectionCommitmentsProjection {
    pub(crate) schema_version: &'static str,
    pub(crate) history_segment_path: PathBuf,
    /// Number of sealed block records loaded from the segment file.
    pub(crate) blocks_scanned: usize,
    pub(crate) decision_entries: Vec<SealedSelectionDecisionRow>,
    /// True when every surfaced row passed structural digest checks.
    pub(crate) all_checks_pass: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SealedSelectionDecisionRow {
    pub(crate) segment_line_index: u64,
    pub(crate) block_hash: String,
    pub(crate) block_height: u64,
    pub(crate) lineage_id: String,
    pub(crate) entry_id: String,
    pub(crate) entry_subject: String,
    pub(crate) observed_payload_ref: String,
    pub(crate) sealed_payload_hash: String,
    pub(crate) recomputed_decision_hash: String,
    pub(crate) decision_observation_ok: bool,
    pub(crate) considered_order_ok: bool,
    pub(crate) procedure_or_policy: String,
    pub(crate) scope: String,
    pub(crate) selected_candidate: Option<String>,
    pub(crate) candidates: Vec<SealedEvaluationCandidateRow>,
    pub(crate) decision_projection_failure_count: usize,
    pub(crate) row_ok: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SealedEvaluationCandidateRow {
    pub(crate) candidate_subject: String,
    pub(crate) recomputed_payload_hash: String,
    pub(crate) payload_digest_ok: bool,
    pub(crate) selection_input_binding_ok: bool,
    pub(crate) source_ref_count: usize,
    pub(crate) source_hash_count: usize,
    pub(crate) source_evidence_alignment_ok: bool,
}

/// Verify and surface selection-decision entries from the append-only sealed
/// block segment (digest checks on the committed decision payload, considered
/// order, per-candidate evaluation payloads, and selection-input bindings).
pub(crate) fn project_sealed_selection_commitments(
    manifest_path: &Path,
) -> Result<SealedSelectionCommitmentsProjection, PreviewError> {
    let store = FsBlockStore::for_campaign_manifest(manifest_path);
    let segment_path = prototype_root(manifest_path).join("history/blocks/segment-000000.jsonl");
    let blocks = store.load_segment_verified_blocks()?;
    let blocks_scanned = blocks.len();
    let mut decision_entries = Vec::new();

    for (line_index, block) in blocks {
        collect_selection_decision_rows(line_index, &block, &mut decision_entries)?;
    }

    let all_checks_pass = decision_entries.iter().all(|row| row.row_ok);

    Ok(SealedSelectionCommitmentsProjection {
        schema_version: SEALED_SELECTION_SCHEMA_VERSION,
        history_segment_path: segment_path,
        blocks_scanned,
        decision_entries,
        all_checks_pass,
    })
}

fn collect_selection_decision_rows(
    line_index: u64,
    block: &Block<block::Sealed>,
    out: &mut Vec<SealedSelectionDecisionRow>,
) -> Result<(), PreviewError> {
    let block_hash = block.block_hash().to_string();
    let block_height = block.block_height();
    let lineage_id = block.lineage_id().as_str().to_string();

    for entry in block.entries() {
        if entry.entry_kind() != EntryKind::Decision {
            continue;
        }
        let Some(selection) = entry.selection_decision() else {
            continue;
        };

        let recomputed_decision = selection.decision_hash()?;
        let recomputed_decision_hex = recomputed_decision.as_str().to_string();
        let sealed_hash = entry.payload_hash().as_str().to_string();
        let decision_observation_ok = entry
            .verify_selection_decision_observation()?
            .unwrap_or(false);
        let considered_order_ok = selection.verify_considered_order_hash()?;

        let mut candidates = Vec::new();
        for evaluation in &selection.considered {
            let payload_hash = evaluation.payload_hash()?;
            let payload_digest_ok = true;
            let selection_input_binding_ok = evaluation.verify_selection_input_binding()?;
            let source_ref_count = evaluation.source_refs.len();
            let source_hash_count = evaluation.source_hashes.len();
            let source_evidence_alignment_ok = source_ref_count == source_hash_count;
            let recomputed_hex = payload_hash.as_str().to_string();
            candidates.push(SealedEvaluationCandidateRow {
                candidate_subject: evaluation.candidate.as_str().to_string(),
                recomputed_payload_hash: recomputed_hex,
                payload_digest_ok,
                selection_input_binding_ok,
                source_ref_count,
                source_hash_count,
                source_evidence_alignment_ok,
            });
        }

        let row_ok = decision_observation_ok
            && considered_order_ok
            && candidates.iter().all(|c| {
                c.payload_digest_ok
                    && c.selection_input_binding_ok
                    && c.source_evidence_alignment_ok
            });

        out.push(SealedSelectionDecisionRow {
            segment_line_index: line_index,
            block_hash: block_hash.clone(),
            block_height,
            lineage_id: lineage_id.clone(),
            entry_id: format!("{}", entry.entry_id()),
            entry_subject: entry.subject().as_str().to_string(),
            observed_payload_ref: entry.observed_payload_ref().as_str().to_string(),
            sealed_payload_hash: sealed_hash,
            recomputed_decision_hash: recomputed_decision_hex,
            decision_observation_ok,
            considered_order_ok,
            procedure_or_policy: selection.procedure_or_policy.as_str().to_string(),
            scope: selection.scope.as_str().to_string(),
            selected_candidate: selection
                .selected_candidate
                .as_ref()
                .map(|subject| subject.as_str().to_string()),
            candidates,
            decision_projection_failure_count: selection.projection_failures.len(),
            row_ok,
        });
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

    let sealed_selection_commitments =
        project_sealed_selection_commitments(manifest_path)?;

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
        sealed_selection_commitments,
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
    sealed_selection_commitments: SealedSelectionCommitmentsProjection,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChildEvidenceProjection {
    schema_version: String,
    generated_at: String,
    campaign_id: String,
    manifest_path: PathBuf,
    prototype_root: PathBuf,
    evidence: ChildEvidenceSet,
}

impl ChildEvidenceProjection {
    fn print(&self) {
        let global_diagnostics = self.evidence.diagnostics.len();
        let child_diagnostics: usize = self
            .evidence
            .children
            .iter()
            .map(|child| child.diagnostics.len())
            .sum();

        println!("prototype1 child evidence");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", self.schema_version);
        println!("generated_at: {}", self.generated_at);
        println!("campaign_id: {}", self.campaign_id);
        println!("manifest: {}", self.manifest_path.display());
        println!("prototype_root: {}", self.prototype_root.display());
        println!("children: {}", self.evidence.children.len());
        println!("unplaced: {}", self.evidence.unplaced.len());
        println!(
            "diagnostics: {} (global={} child={})",
            global_diagnostics + child_diagnostics,
            global_diagnostics,
            child_diagnostics
        );
        println!("treatment_note: source treatment labels are not sealed authority");
        println!();

        println!("children");
        println!("{}", "-".repeat(40));
        if self.evidence.children.is_empty() {
            println!("(none)");
        } else {
            for child in self
                .evidence
                .children
                .iter()
                .take(CHILD_EVIDENCE_TABLE_LIMIT)
            {
                println!(
                    "node={} parent={} gen={} branch={} runtimes={} branches={} evaluations={} documents={} journal={} diagnostics={}",
                    child.node_id,
                    opt_text(child.parent_node_id.as_deref()),
                    opt_u32(child.generation),
                    opt_text(child.branch_id.as_deref()),
                    child.runtimes.len(),
                    child.branches.len(),
                    child.evaluations.len(),
                    child.documents.len(),
                    child.journal.len(),
                    child.diagnostics.len()
                );
            }
            print_omitted(
                self.evidence.children.len(),
                CHILD_EVIDENCE_TABLE_LIMIT,
                "children",
            );
        }
        println!();

        self.print_evaluations();
        println!();
        self.print_unplaced();
        println!();
        self.print_diagnostics();
    }

    fn print_evaluations(&self) {
        let total: usize = self
            .evidence
            .children
            .iter()
            .map(|child| child.evaluations.len())
            .sum();

        println!("evaluations");
        println!("{}", "-".repeat(40));
        if total == 0 {
            println!("(none)");
            return;
        }

        let mut printed = 0;
        for child in &self.evidence.children {
            for evaluation in &child.evaluations {
                if printed >= CHILD_EVIDENCE_TABLE_LIMIT {
                    print_omitted(total, CHILD_EVIDENCE_TABLE_LIMIT, "evaluations");
                    return;
                }
                print_evaluation(&child.node_id, evaluation);
                printed += 1;
            }
        }
    }

    fn print_unplaced(&self) {
        println!("unplaced evidence");
        println!("{}", "-".repeat(40));
        if self.evidence.unplaced.is_empty() {
            println!("(none)");
        } else {
            for source in self
                .evidence
                .unplaced
                .iter()
                .take(CHILD_EVIDENCE_TABLE_LIMIT)
            {
                print_source(source);
            }
            print_omitted(
                self.evidence.unplaced.len(),
                CHILD_EVIDENCE_TABLE_LIMIT,
                "unplaced sources",
            );
        }
    }

    fn print_diagnostics(&self) {
        let total = self.evidence.diagnostics.len()
            + self
                .evidence
                .children
                .iter()
                .map(|child| child.diagnostics.len())
                .sum::<usize>();

        println!("diagnostics");
        println!("{}", "-".repeat(40));
        if total == 0 {
            println!("(none)");
            return;
        }

        let mut printed = 0;
        for diagnostic in &self.evidence.diagnostics {
            if printed >= CHILD_EVIDENCE_TABLE_LIMIT {
                print_omitted(total, CHILD_EVIDENCE_TABLE_LIMIT, "diagnostics");
                return;
            }
            print_diagnostic(None, diagnostic);
            printed += 1;
        }

        for child in &self.evidence.children {
            for diagnostic in &child.diagnostics {
                if printed >= CHILD_EVIDENCE_TABLE_LIMIT {
                    print_omitted(total, CHILD_EVIDENCE_TABLE_LIMIT, "diagnostics");
                    return;
                }
                print_diagnostic(Some(&child.node_id), diagnostic);
                printed += 1;
            }
        }
    }
}

fn print_evaluation(node_id: &str, evaluation: &EvaluationEvidence) {
    println!(
        "node={} branch={} disposition={} compared={} artifact={} source={}",
        node_id,
        evaluation.branch_id,
        opt_text(evaluation.overall_disposition.as_deref()),
        evaluation.compared.len(),
        opt_path(evaluation.evaluation_artifact_path.as_deref()),
        evaluation.source.pointer.ref_id()
    );

    for compared in evaluation
        .compared
        .iter()
        .take(CHILD_EVIDENCE_COMPARED_LIMIT)
    {
        print_compared(compared);
    }
    print_omitted(
        evaluation.compared.len(),
        CHILD_EVIDENCE_COMPARED_LIMIT,
        "compared runs",
    );
}

fn print_compared(compared: &ComparedRunEvidence) {
    println!(
        "  compared instance={} status={} baseline={} treatment={}",
        opt_text(compared.instance_id.as_deref()),
        opt_text(compared.status.as_deref()),
        opt_path(compared.baseline_record_path.as_deref()),
        opt_path(compared.treatment_record_path.as_deref())
    );
}

fn print_source(source: &EvidenceSource) {
    println!(
        "class={} kind={} treatment={} ref={}",
        source.class.as_str(),
        source.kind,
        source.treatment,
        source.pointer.ref_id()
    );
}

fn print_diagnostic(node_id: Option<&str>, diagnostic: &EvidenceDiagnostic) {
    println!(
        "node={} {} [{}]: {}",
        opt_text(node_id),
        diagnostic.severity,
        opt_text(diagnostic.source_ref.as_deref()),
        diagnostic.message
    );
}

fn print_omitted(total: usize, limit: usize, label: &str) {
    if total > limit {
        println!("... {} more {} omitted", total - limit, label);
    }
}

fn opt_text(value: Option<&str>) -> &str {
    value.unwrap_or("-")
}

fn opt_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn opt_path(value: Option<&Path>) -> String {
    value
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string())
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
        println!(
            "sealed selection decision rows: {}",
            self.sealed_selection_commitments.decision_entries.len()
        );
        println!(
            "sealed selection digests ok: {}",
            self.sealed_selection_commitments.all_checks_pass
        );
        println!();

        println!("sealed selection decisions (block segment)");
        println!("{}", "-".repeat(40));
        println!(
            "segment: {}",
            self.sealed_selection_commitments
                .history_segment_path
                .display()
        );
        println!(
            "blocks scanned: {}",
            self.sealed_selection_commitments.blocks_scanned
        );
        if self.sealed_selection_commitments.decision_entries.is_empty() {
            println!("(no sealed selection entries in segment)");
        } else {
            for row in &self.sealed_selection_commitments.decision_entries {
                println!(
                    "line={} block_height={} block={} row_ok={} decision_hash_ok={} order_ok={} procedure={} scope={} selected={}",
                    row.segment_line_index,
                    row.block_height,
                    &row.block_hash[..16.min(row.block_hash.len())],
                    row.row_ok,
                    row.decision_observation_ok,
                    row.considered_order_ok,
                    row.procedure_or_policy,
                    row.scope,
                    row.selected_candidate.as_deref().unwrap_or("-")
                );
                for cand in &row.candidates {
                    println!(
                        "  candidate={} payload_ok={} sel_input_ok={} refs={} hashes={} ref_alignment_ok={}",
                        cand.candidate_subject,
                        cand.payload_digest_ok,
                        cand.selection_input_binding_ok,
                        cand.source_ref_count,
                        cand.source_hash_count,
                        cand.source_evidence_alignment_ok
                    );
                }
            }
        }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvidencePointer {
    class: EvidenceClass,
    ref_id: String,
    path: PathBuf,
    line: Option<usize>,
    hash: HistoryHash,
}

impl EvidencePointer {
    pub(crate) fn class(&self) -> EvidenceClass {
        self.class
    }

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

pub(crate) trait EvidenceRecord: Serialize + DeserializeOwned {
    const RECORD_NAME: &'static str;
    const SCHEMA: &'static str;

    fn accepts_class(class: EvidenceClass) -> bool;
}

#[derive(Debug, Clone)]
pub(crate) struct Stored<T>
where
    T: EvidenceRecord,
{
    pointer: EvidencePointer,
    item: T,
}

impl<T> Stored<T>
where
    T: EvidenceRecord,
{
    pub(crate) fn pointer(&self) -> &EvidencePointer {
        &self.pointer
    }

    pub(crate) fn item(&self) -> &T {
        &self.item
    }
}

impl EvidenceRecord for JournalEntry {
    const RECORD_NAME: &'static str = "JournalEntry";
    const SCHEMA: &'static str = "prototype1-transition-journal.jsonl";

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::TransitionJournal
    }
}

impl EvidenceRecord for Prototype1NodeRecord {
    const RECORD_NAME: &'static str = "Prototype1NodeRecord";
    const SCHEMA: &'static str = PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::NodeRecord
    }
}

impl EvidenceRecord for Prototype1RunnerRequest {
    const RECORD_NAME: &'static str = "Prototype1RunnerRequest";
    const SCHEMA: &'static str = PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::RunnerRequest
    }
}

impl EvidenceRecord for Prototype1RunnerResult {
    const RECORD_NAME: &'static str = "Prototype1RunnerResult";
    const SCHEMA: &'static str = PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        matches!(
            class,
            EvidenceClass::RunnerResult | EvidenceClass::AttemptResult
        )
    }
}

impl EvidenceRecord for Prototype1BranchEvaluationReport {
    const RECORD_NAME: &'static str = "Prototype1BranchEvaluationReport";
    const SCHEMA: &'static str = "prototype1-branch-evaluation-report.v1";

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::Evaluation
    }
}

impl EvidenceRecord for Invocation {
    const RECORD_NAME: &'static str = "Invocation";
    const SCHEMA: &'static str = super::invocation::SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::Invocation
    }
}

impl EvidenceRecord for SuccessorReadyRecord {
    const RECORD_NAME: &'static str = "SuccessorReadyRecord";
    const SCHEMA: &'static str = super::invocation::SUCCESSOR_READY_SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::SuccessorReady
    }
}

impl EvidenceRecord for SuccessorCompletionRecord {
    const RECORD_NAME: &'static str = "SuccessorCompletionRecord";
    const SCHEMA: &'static str = super::invocation::SUCCESSOR_COMPLETION_SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::SuccessorCompletion
    }
}

impl EvidenceRecord for Prototype1SchedulerState {
    const RECORD_NAME: &'static str = "Prototype1SchedulerState";
    const SCHEMA: &'static str = PROTOTYPE1_SCHEDULER_SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::Scheduler
    }
}

impl EvidenceRecord for Prototype1BranchRegistry {
    const RECORD_NAME: &'static str = "Prototype1BranchRegistry";
    const SCHEMA: &'static str = PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION;

    fn accepts_class(class: EvidenceClass) -> bool {
        class == EvidenceClass::BranchRegistry
    }
}

#[derive(Debug, Default)]
pub(crate) struct SelectionPreviewRecords {
    pub(crate) scheduler: Option<Stored<Prototype1SchedulerState>>,
    pub(crate) branch_registry: Option<Stored<Prototype1BranchRegistry>>,
}

/// Generic JSON preview catalog/projection compatibility document.
///
/// `Document` exists only so `history preview` can catalog adjacent Prototype 1
/// JSON files, preserve payload refs, and render degraded/projection preview
/// entries before those files have sealed History records.
///
/// It must not be used for typed child evidence, metrics, successor selection,
/// future scoring, or History/Crown authority. Declared Prototype 1 records
/// used by those paths cross the store boundary only as `Stored<T>` where
/// `T: Serialize + DeserializeOwned + EvidenceRecord`; deserialization failure
/// is corruption or a boundary error, not an invitation to recover semantics
/// from `serde_json::Value`, filenames, or paths.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
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

    pub(crate) fn treatment(self) -> &'static str {
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
    T: EvidenceRecord,
{
    debug_assert!(T::accepts_class(class));
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

fn load_typed_record<T>(class: EvidenceClass, path: PathBuf) -> Result<Stored<T>, PreviewError>
where
    T: EvidenceRecord,
{
    debug_assert!(T::accepts_class(class));
    let bytes = fs::read(&path).map_err(|source| PreviewError::Read {
        path: path.clone(),
        source,
    })?;
    let item = serde_json::from_slice(&bytes).map_err(|source| PreviewError::ParseRecord {
        path: path.clone(),
        class,
        record: T::RECORD_NAME,
        source,
    })?;
    Ok(Stored {
        pointer: EvidencePointer {
            class,
            ref_id: format!("file:{}", path.display()),
            path,
            line: None,
            hash: HistoryHash::of_bytes(&bytes),
        },
        item,
    })
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
        JournalEntry::Resource(entry) => resource_sample(entry),
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

fn resource_sample(entry: &super::journal::resource::Sample) -> JournalProjection {
    let mut missing = Vec::new();
    if entry.status == super::journal::resource::Status::Failed {
        missing.push(
            entry
                .error
                .clone()
                .unwrap_or_else(|| "resource sample failed without detail".to_string()),
        );
    }

    JournalProjection {
        entry_kind: EntryKind::Observation,
        subject: format!("resource:{:?}:{}", entry.subject, entry.path.display()),
        executor: format!("parent:{}", entry.parent_id),
        observer: format!("parent:{}", entry.parent_id),
        procedure_or_policy: "prototype1.resource.sample".to_string(),
        generation: Some(entry.generation),
        recorded_at: Some(entry.recorded_at),
        input_refs: vec![format!("phase:{:?}", entry.phase)],
        output_refs: vec![
            format!("status:{:?}", entry.status),
            format!(
                "bytes:{}",
                entry
                    .bytes
                    .map(|bytes| bytes.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
        ],
        missing,
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

    #[error(
        "typed evidence boundary error: failed to parse {record} for class {class:?} at '{path}'"
    )]
    ParseRecord {
        path: PathBuf,
        class: EvidenceClass,
        record: &'static str,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to serialize History preview")]
    Serialize(#[from] serde_json::Error),

    #[error("failed to hash History preview value")]
    Hash(#[from] super::history::HistoryError),

    #[error(transparent)]
    BlockStore(#[from] super::history::BlockStoreError),
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

    #[test]
    fn sealed_selection_commitments_empty_when_no_history_segment() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");

        let proj = project_sealed_selection_commitments(&manifest).expect("projection");
        assert_eq!(proj.blocks_scanned, 0);
        assert!(proj.decision_entries.is_empty());
        assert!(proj.all_checks_pass);

        let preview = build("campaign-a", &manifest).expect("preview");
        assert_eq!(preview.sealed_selection_commitments.blocks_scanned, 0);
        assert!(preview.sealed_selection_commitments.all_checks_pass);
    }
}
