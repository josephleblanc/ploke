//! Read-only audit reports for Prototype 1 walk persistence surfaces.
//!
//! The first slice is intentionally narrow: it audits the documents `R0 -> R1`
//! needs before the transition can collect campaign context, plus the file/DB
//! surfaces that transition is expected to touch.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use cozo::DataValue;
use ploke_db::Database;
use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    campaign::{campaign_closure_state_path, campaign_manifest_path, load_campaign_manifest},
    cli::{
        Prototype1StateWalkAuditScope, Prototype1StateWalkAuditTransition,
        prototype1_state::identity,
    },
    layout::prototype1_monitor_target_file,
};

use super::phase::WalkPhase;
use crate::cli::prototype1_state::{
    candidate_review,
    eval_store::{
        load_owner_eval_database, prototype1_eval_store_db_path, selection_decision_id,
        selection_member_id,
    },
    history::SelectionDecisionEntry,
    journal::{JournalEntry, PrototypeJournal},
    successor,
};
use crate::successor_selection::{
    PatchChange, PatchGate, PatchReview, PatchVerdict, domains::Confidence,
};

const SCHEMA_VERSION: &str = "prototype1.walk.audit.v1";

const EVAL_RELS: &[(&str, &str)] = &[
    ("eval_campaign", "campaign_id"),
    ("eval_campaign_eval_policy", "campaign_id"),
    ("eval_campaign_eval_budget", "campaign_id"),
    ("eval_campaign_protocol_policy", "campaign_id"),
    ("eval_profile_commitment", "profile_ref_id"),
    ("eval_patch_gate", "campaign_id"),
    ("eval_closure_ref", "closure_ref_id"),
    ("eval_closure_instance", "closure_ref_id"),
    ("eval_closure_artifact_ref", "closure_ref_id"),
    ("eval_closure_protocol_procedure", "closure_ref_id"),
    ("eval_closure_protocol_counts", "closure_ref_id"),
    ("eval_baseline", "baseline_id"),
    ("eval_baseline_instance", "baseline_id"),
    ("eval_baseline_instance_metrics", "baseline_id"),
    ("eval_transition_event", "event_id"),
    ("eval_record_ref", "record_ref_id"),
    ("eval_log_ref", "log_ref_id"),
    ("eval_attempt", "attempt_id"),
    ("eval_invocation", "invocation_id"),
    ("eval_channel_message", "channel_message_id"),
    ("eval_channel_receipt", "receipt_id"),
    ("eval_import_event", "import_id"),
    ("eval_trace_event", "trace_event_id"),
    ("eval_agent_turn", "turn_id"),
    ("eval_agent_turn_event", "event_id"),
    ("eval_model_exchange", "exchange_id"),
    ("eval_message_event", "message_event_id"),
    ("eval_tool_event", "tool_event_id"),
    ("eval_evaluation", "evaluation_id"),
    ("eval_evaluation_instance", "evaluation_id"),
    ("eval_continuation_decision", "decision_id"),
    ("eval_selection_decision", "decision_id"),
    ("eval_selection_receipt", "decision_id"),
    ("eval_selection_candidate", "decision_id"),
    ("eval_selection_finding", "finding_id"),
    ("eval_selection_score", "decision_id"),
    ("eval_selection_oracle", "decision_id"),
    ("eval_selection_patch_gate", "decision_id"),
    ("eval_selection_patch_review", "decision_id"),
    ("eval_selection_patch_change", "decision_id"),
    ("eval_selection_projection_failure", "decision_id"),
    ("eval_artifact", "artifact_id"),
    ("eval_artifact_surface", "surface_id"),
    ("eval_artifact_ref", "artifact_ref_id"),
    ("eval_binary_ref", "binary_ref_id"),
    ("eval_build_event", "build_id"),
    ("eval_operation", "operation_id"),
    ("eval_patch", "patch_id"),
    ("eval_apply_event", "apply_id"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkAuditReport {
    pub schema_version: String,
    pub generated_at: String,
    pub scope: Prototype1StateWalkAuditScope,
    pub transition_filter: Option<Prototype1StateWalkAuditTransition>,
    pub phase: WalkPhase,
    pub verbose: bool,
    pub with_note: bool,
    pub repo_root: PathBuf,
    pub campaign: CampaignAudit,
    pub documents: Vec<DocumentAudit>,
    pub database: DatabaseAudit,
    pub transition: TransitionAudit,
    pub transitions: Vec<TransitionChecklist>,
    pub summary: AuditSummary,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionChecklist {
    pub transition: String,
    pub from: WalkPhase,
    pub to: WalkPhase,
    pub file_status: PersistenceStatus,
    pub db_status: PersistenceStatus,
    pub overall_status: PersistenceStatus,
    pub items: Vec<PersistenceItemAudit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceItemAudit {
    pub name: String,
    pub file: PersistenceSide,
    pub database: PersistenceSide,
    pub overall_status: PersistenceStatus,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceSide {
    pub status: PersistenceStatus,
    pub count: Option<i64>,
    pub path: Option<PathBuf>,
    pub relation: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceStatus {
    Ok,
    Partial,
    None,
    NotApplicable,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignAudit {
    pub campaign_id: Option<CampaignId>,
    pub source: CampaignSource,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignSource {
    Explicit,
    ParentIdentity,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAudit {
    pub name: String,
    pub path: PathBuf,
    pub role: DocumentRole,
    pub expectation: DocumentExpectation,
    pub expected_at: ExpectedAt,
    pub exists: bool,
    pub status: DocumentStatus,
    pub sha256: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentRole {
    Authority,
    Projection,
    DerivedCache,
    Pointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentExpectation {
    Required,
    Optional,
    DerivedIfMissing,
    WrittenByTransition,
    PathOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedAt {
    Precondition,
    TransitionOutput,
    TransitionPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Ok,
    Missing,
    ReadError,
    ParseError,
    NotChecked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseAudit {
    pub path: Option<PathBuf>,
    pub exists: bool,
    pub status: DatabaseStatus,
    pub relation_counts: Vec<RelationCount>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseStatus {
    Ok,
    Missing,
    OpenError,
    CampaignUnresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationCount {
    pub relation: String,
    pub exists: bool,
    pub count: Option<i64>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
struct ReviewMirrorAudit {
    review_file: PersistenceSide,
    review_db: PersistenceSide,
    change_file: PersistenceSide,
    change_db: PersistenceSide,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PatchMirrorCoverage {
    review_count: usize,
    change_count: usize,
    review_rows: usize,
    change_rows: usize,
    pending_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionAudit {
    pub transition: String,
    pub from: WalkPhase,
    pub to: WalkPhase,
    pub code_symbol: String,
    pub expected_file_writes: Vec<ExpectedPersistence>,
    pub expected_db_writes: Vec<ExpectedPersistence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedPersistence {
    pub surface: String,
    pub expectation: DocumentExpectation,
    pub status: ExpectedStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExpectedStatus {
    Expected,
    NotExpected,
    Conditional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditSummary {
    pub required_documents: usize,
    pub required_ok: usize,
    pub missing_required: usize,
    pub parse_errors: usize,
    pub db_rows_total: i64,
    pub expected_transition_db_rows: i64,
    pub observed_transition_db_rows: i64,
    pub verdict: AuditVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditVerdict {
    Ready,
    MissingPreconditions,
    NeedsReview,
}

pub(crate) fn audit_r0_to_r1(
    repo_root: PathBuf,
    phase: WalkPhase,
    campaign: Option<CampaignId>,
    transition_filter: Option<Prototype1StateWalkAuditTransition>,
) -> WalkAuditReport {
    let campaign_audit = resolve_campaign(&repo_root, campaign);
    let mut docs = Vec::new();

    docs.push(check_directory(
        "repo_root",
        repo_root.clone(),
        DocumentRole::Authority,
        DocumentExpectation::Required,
        ExpectedAt::Precondition,
    ));

    let parent_path = identity::parent_identity_path(&repo_root);
    docs.push(check_json_doc(
        "parent_identity",
        parent_path,
        DocumentRole::Authority,
        if campaign_audit.source == CampaignSource::Explicit {
            DocumentExpectation::Optional
        } else {
            DocumentExpectation::Required
        },
        ExpectedAt::Precondition,
    ));

    if let Some(campaign_id) = campaign_audit.campaign_id.as_ref() {
        match campaign_manifest_path(campaign_id) {
            Ok(path) => docs.push(check_campaign_manifest(path)),
            Err(error) => docs.push(error_doc(
                "campaign_manifest",
                PathBuf::from("<unresolved>"),
                DocumentRole::Authority,
                DocumentExpectation::Required,
                ExpectedAt::Precondition,
                error.to_string(),
            )),
        }

        if let Ok(manifest_path) = campaign_manifest_path(campaign_id) {
            let proto_root = manifest_path
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("prototype1");
            docs.push(check_toml_doc(
                "run_profile",
                proto_root.join("run-profile.toml"),
                DocumentRole::Authority,
                DocumentExpectation::Required,
                ExpectedAt::Precondition,
            ));
            docs.push(check_json_doc(
                "run_profile_commitment",
                proto_root.join("run-profile.commitment.json"),
                DocumentRole::Authority,
                DocumentExpectation::Optional,
                ExpectedAt::Precondition,
            ));
            docs.push(check_jsonl_doc(
                "transition_journal",
                proto_root.join("transition-journal.jsonl"),
                DocumentRole::Authority,
                DocumentExpectation::PathOnly,
                ExpectedAt::TransitionPath,
            ));
            docs.push(check_json_doc(
                "active_monitor_target",
                prototype1_monitor_target_file().unwrap_or_else(|_| PathBuf::from("<unresolved>")),
                DocumentRole::Pointer,
                DocumentExpectation::WrittenByTransition,
                ExpectedAt::TransitionOutput,
            ));
        }

        match campaign_closure_state_path(campaign_id) {
            Ok(path) => docs.push(check_json_doc(
                "closure_state",
                path,
                DocumentRole::DerivedCache,
                DocumentExpectation::DerivedIfMissing,
                ExpectedAt::Precondition,
            )),
            Err(error) => docs.push(error_doc(
                "closure_state",
                PathBuf::from("<unresolved>"),
                DocumentRole::DerivedCache,
                DocumentExpectation::DerivedIfMissing,
                ExpectedAt::Precondition,
                error.to_string(),
            )),
        }
    }

    let (database, review_mirror) = database_audit(campaign_audit.campaign_id.as_ref());
    let transition = r0_to_r1_expectations();
    let stopped = stopped_reconstruction_side(&repo_root, campaign_audit.campaign_id.as_ref());
    let mut transitions = transition_checklist(
        campaign_audit.campaign_id.as_ref(),
        &docs,
        &database,
        &review_mirror,
        &stopped,
    );
    if let Some(filter) = transition_filter {
        transitions.retain(|transition| transition.transition == audit_transition_label(filter));
    }
    let summary = summarize(&docs, &database, &transition, &transitions);
    let notes = vec![
        "R0 is an in-memory command carrier; this audit checks documents needed before r0_to_r1 can collect context.".to_string(),
        "r0_to_r1 writes/updates the active monitor target and creates an in-memory journal handle, but it does not append a transition-journal entry.".to_string(),
        "When the admitted run profile selects database or dual-strict eval storage, r0_to_r1 mirrors campaign/profile/closure setup rows into the owner eval DB.".to_string(),
    ];

    WalkAuditReport {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        scope: Prototype1StateWalkAuditScope::R0ToR1,
        transition_filter,
        phase,
        verbose: false,
        with_note: false,
        repo_root,
        campaign: campaign_audit,
        documents: docs,
        database,
        transition,
        transitions,
        summary,
        notes,
    }
}

impl WalkAuditReport {
    pub fn render_table(&self) -> String {
        let mut lines = Vec::new();
        lines.push("walk audit".to_string());
        lines.push("-".repeat(40));
        lines.push(format!("scope: {}", scope_label(self.scope)));
        if let Some(filter) = self.transition_filter {
            lines.push(format!(
                "transition_filter: {}",
                audit_transition_label(filter)
            ));
        }
        lines.push(format!("phase: {} - {}", self.phase, self.phase.detail()));
        lines.push(format!(
            "mode: {}",
            if self.verbose { "verbose" } else { "summary" }
        ));
        lines.push(format!("repo_root: {}", self.repo_root.display()));
        lines.push(format!(
            "campaign: {} ({:?})",
            self.campaign
                .campaign_id
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "-".to_string()),
            self.campaign.source
        ));
        if let Some(detail) = self.campaign.detail.as_ref() {
            lines.push(format!("campaign_detail: {detail}"));
        }
        lines.push(format!(
            "verdict: {:?} (required {}/{}, missing {}, parse_errors {})",
            self.summary.verdict,
            self.summary.required_ok,
            self.summary.required_documents,
            self.summary.missing_required,
            self.summary.parse_errors
        ));
        lines.push("".to_string());
        lines.push("transition persistence checklist:".to_string());
        if self.with_note {
            lines.push(format!(
                "  {:<12} {:<30} {:<12} {:<12} {:<12} {}",
                "transition", "item", "file", "db", "overall", "note"
            ));
        } else {
            lines.push(format!(
                "  {:<12} {:<30} {:<12} {:<12} {:<12}",
                "transition", "item", "file", "db", "overall"
            ));
        }
        for transition in &self.transitions {
            for item in &transition.items {
                if self.with_note {
                    lines.push(format!(
                        "  {:<12} {:<30} {:<12} {:<12} {:<12} {}",
                        transition.transition,
                        item.name,
                        persistence_label(item.file.status),
                        persistence_label(item.database.status),
                        persistence_label(item.overall_status),
                        item.note
                    ));
                } else {
                    lines.push(format!(
                        "  {:<12} {:<30} {:<12} {:<12} {:<12}",
                        transition.transition,
                        item.name,
                        persistence_label(item.file.status),
                        persistence_label(item.database.status),
                        persistence_label(item.overall_status)
                    ));
                }
            }
            lines.push(format!(
                "  {:<12} {:<30} {:<12} {:<12} {:<12}",
                "",
                "transition rollup",
                persistence_label(transition.file_status),
                persistence_label(transition.db_status),
                persistence_label(transition.overall_status)
            ));
        }

        if self.verbose {
            self.render_verbose(&mut lines);
        } else {
            lines.push("".to_string());
            lines.push("hint: pass --with-note to include checklist notes; pass --verbose to show paths, relations, counts, and legacy document/DB details".to_string());
        }

        lines.push("".to_string());
        lines.push("notes:".to_string());
        for note in &self.notes {
            lines.push(format!("  - {note}"));
        }
        lines.join("\n")
    }

    fn render_verbose(&self, lines: &mut Vec<String>) {
        lines.push("".to_string());
        lines.push("checklist details:".to_string());
        for transition in &self.transitions {
            lines.push(format!(
                "  {} ({} -> {})",
                transition.transition, transition.from, transition.to
            ));
            for item in &transition.items {
                lines.push(format!("    {}:", item.name));
                lines.push(format_side("file", &item.file));
                lines.push(format_side("db", &item.database));
                lines.push(format!(
                    "      overall={} note={}",
                    persistence_label(item.overall_status),
                    item.note
                ));
            }
        }

        lines.push("".to_string());
        lines.push("documents:".to_string());
        for doc in &self.documents {
            lines.push(format!(
                "  [{}] {} {:?}/{:?} {}",
                status_label(doc.status),
                doc.name,
                doc.role,
                doc.expectation,
                doc.path.display()
            ));
            if let Some(sha) = doc.sha256.as_ref() {
                lines.push(format!("      sha256={sha}"));
            }
            if let Some(detail) = doc.detail.as_ref() {
                lines.push(format!("      {detail}"));
            }
        }
        lines.push("".to_string());
        lines.push("database:".to_string());
        lines.push(format!(
            "  path: {}",
            self.database
                .path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_string())
        ));
        lines.push(format!(
            "  status: {:?}, exists: {}, total_rows: {}",
            self.database.status, self.database.exists, self.summary.db_rows_total
        ));
        if let Some(detail) = self.database.detail.as_ref() {
            lines.push(format!("  detail: {detail}"));
        }
        lines.push("  relations:".to_string());
        for row in &self.database.relation_counts {
            let count = row
                .count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string());
            lines.push(format!(
                "    {} exists={} count={}{}",
                row.relation,
                row.exists,
                count,
                row.detail
                    .as_ref()
                    .map(|detail| format!(" detail={detail}"))
                    .unwrap_or_default()
            ));
        }
        lines.push("".to_string());
        lines.push(format!(
            "transition expectation: {} ({} -> {})",
            self.transition.transition, self.transition.from, self.transition.to
        ));
        lines.push(format!("  code: {}", self.transition.code_symbol));
        lines.push("  expected_file_persistence:".to_string());
        for item in &self.transition.expected_file_writes {
            lines.push(format!(
                "    {:?} {}: {}",
                item.status, item.surface, item.detail
            ));
        }
        lines.push("  expected_db_persistence:".to_string());
        for item in &self.transition.expected_db_writes {
            lines.push(format!(
                "    {:?} {}: {}",
                item.status, item.surface, item.detail
            ));
        }
    }
}

fn transition_checklist(
    campaign_id: Option<&CampaignId>,
    docs: &[DocumentAudit],
    database: &DatabaseAudit,
    reviews: &ReviewMirrorAudit,
    stopped: &PersistenceSide,
) -> Vec<TransitionChecklist> {
    let manifest = campaign_id.and_then(|id| campaign_manifest_path(id).ok());
    let root = manifest.as_ref().map(prototype_root_for_manifest);
    let journal = root
        .as_ref()
        .map(|root| root.join("transition-journal.jsonl"));
    let nodes = root.as_ref().map(|root| root.join("nodes"));
    let messages = root.as_ref().map(|root| root.join("messages"));
    let history = root.as_ref().map(|root| root.join("history"));

    vec![
        checklist_transition(
            "r0->r1",
            WalkPhase::R0,
            WalkPhase::R1,
            vec![
                item(
                    "campaign manifest",
                    doc_side("campaign_manifest", docs),
                    db_side(database, "eval_campaign", None, DbCompare::AnyRows),
                    "campaign authority file plus setup DB campaign row",
                ),
                item(
                    "run profile",
                    doc_side("run_profile", docs),
                    db_side(
                        database,
                        "eval_profile_commitment",
                        None,
                        DbCompare::AnyRows,
                    ),
                    "admitted profile file plus profile commitment row",
                ),
                item(
                    "configured patch gate",
                    doc_side("run_profile", docs),
                    db_side(database, "eval_patch_gate", None, DbCompare::AnyRows),
                    "admitted profile patch-safety policy plus its queryable DB projection",
                ),
                item(
                    "closure ref",
                    doc_side("closure_state", docs),
                    db_side(database, "eval_closure_ref", None, DbCompare::AnyRows),
                    "closure cache/file authority plus DB closure ref",
                ),
                item(
                    "journal path",
                    doc_side("transition_journal", docs),
                    PersistenceSide::not_applicable(
                        "r0_to_r1 opens the JSONL stream but does not append",
                    ),
                    "transition journal is the later append authority; no r0->r1 row is appended",
                ),
                item(
                    "active monitor target",
                    doc_side("active_monitor_target", docs),
                    PersistenceSide::not_applicable("file-only runtime pointer"),
                    "operator monitor target is currently file-only",
                ),
            ],
        ),
        checklist_transition(
            "r1->r2a",
            WalkPhase::R1,
            WalkPhase::R2a,
            vec![item(
                "parent identity",
                doc_side("parent_identity", docs),
                PersistenceSide::not_applicable(
                    "active checkout identity is artifact authority, not eval-store authority",
                ),
                "initialization writes and commits .ploke/prototype1/parent_identity.json",
            )],
        ),
        checklist_transition(
            "r1->r3",
            WalkPhase::R1,
            WalkPhase::R3,
            vec![no_write_item(
                "resolved parent",
                "normal startup resolves existing parent identity or successor invocation without new persistence",
            )],
        ),
        checklist_transition(
            "r2a->r3",
            WalkPhase::R2a,
            WalkPhase::R3,
            vec![no_write_item(
                "initialized parent",
                "moves the identity initialized by r1->r2a into the normal parent path without another filesystem read or write",
            )],
        ),
        checklist_transition(
            "r3->r4a",
            WalkPhase::R3,
            WalkPhase::R4a,
            vec![no_write_item(
                "unchecked parent",
                "loads Parent<Unchecked>; validation/writes happen in later startup branches",
            )],
        ),
        checklist_transition(
            "r4a->r4b",
            WalkPhase::R4a,
            WalkPhase::R4b,
            vec![no_write_item(
                "genesis proof",
                "validates active checkout and genesis startup proof without writing new rows/files",
            )],
        ),
        checklist_transition(
            "r4a->r4c",
            WalkPhase::R4a,
            WalkPhase::R4c,
            vec![
                item(
                    "successor ready channel",
                    count_side(nodes.clone(), count_channel_lines),
                    db_side_for_file(database, "eval_channel_message", &root, |root| {
                        count_channel_lines(&root.join("nodes"))
                    }),
                    "successor startup sends ready over child-to-parent channel; DB rows are channel mirrors",
                ),
                item(
                    "successor ready journal",
                    count_side(journal.clone(), |path| count_successor_state(path, "ready")),
                    PersistenceSide::not_applicable(
                        "successor-ready journal remains JSONL evidence",
                    ),
                    "predecessor startup appends SuccessorRecord::ready evidence",
                ),
            ],
        ),
        checklist_transition(
            "r4b->r4c",
            WalkPhase::R4b,
            WalkPhase::R4c,
            vec![no_write_item(
                "ready carrier",
                "converges checked genesis startup into Parent<Ready> without durable writes",
            )],
        ),
        checklist_transition(
            "r4c->r5",
            WalkPhase::R4c,
            WalkPhase::R5,
            vec![
                item(
                    "parent-start journal",
                    count_side(journal.clone(), count_parent_start),
                    db_side_for_file(database, "eval_transition_event", &root, |root| {
                        count_parent_start(&root.join("transition-journal.jsonl"))
                    }),
                    "ParentStarted plus parent-start resource evidence mirrored as transition rows",
                ),
                item(
                    "record refs",
                    count_side(journal.clone(), count_jsonl_rows),
                    db_side_for_file(database, "eval_record_ref", &root, |root| {
                        count_jsonl_rows(&root.join("transition-journal.jsonl"))
                    }),
                    "journal/resource references mirrored as eval_record_ref where implemented",
                ),
            ],
        ),
        checklist_transition(
            "r5->r6",
            WalkPhase::R5,
            WalkPhase::R6,
            vec![item(
                "complete baseline",
                baseline_source_side(campaign_id, root.as_deref()),
                db_side(database, "eval_baseline", None, DbCompare::AnyRows),
                "baseline authority is derived from closure/evaluation files; DB rows mirror baseline instances",
            )],
        ),
        checklist_transition(
            "r6->r7",
            WalkPhase::R6,
            WalkPhase::R7,
            vec![no_write_item(
                "policy budget",
                "derives policy and child-planning budget in memory from admitted profile/scheduler inputs",
            )],
        ),
        checklist_transition(
            "r7->r8",
            WalkPhase::R7,
            WalkPhase::R8,
            vec![
                item(
                    "parent node status",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "node.json", None)
                    }),
                    PersistenceSide::not_applicable("node.json is a compatibility projection"),
                    "child planning may update parent/child node projections",
                ),
                item(
                    "child plan message",
                    count_side(
                        messages.as_ref().map(|root| root.join("child-plan")),
                        |path| count_extension_files(path, "json"),
                    ),
                    PersistenceSide::not_applicable(
                        "child-plan authority is MessageBox/file backed today",
                    ),
                    "Received child-plan MessageBox body remains file authority",
                ),
                item(
                    "runner requests",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "runner-request.json", None)
                    }),
                    PersistenceSide::not_applicable(
                        "runner requests are bootstrap files, not eval-store rows",
                    ),
                    "planned child bootstrap payloads are written under node directories",
                ),
                item(
                    "edit harness requests",
                    optional_count_side(
                        messages
                            .as_ref()
                            .map(|root| root.join("edit-harness-request")),
                        |path| count_extension_files(path, "json"),
                        "not used by deterministic/non-broad child planning",
                    ),
                    PersistenceSide::not_applicable(
                        "provider request files are not owner eval-store rows",
                    ),
                    "broad planning writes request JSON plus prompt markdown files",
                ),
                item(
                    "pre-child planning",
                    optional_count_side(
                        messages
                            .as_ref()
                            .map(|root| root.join("pre-child-planning")),
                        |path| {
                            count_files(path, &mut |candidate| {
                                candidate
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .is_some_and(|ext| ext == "json" || ext == "md")
                            })
                        },
                        "not used when pre-child planner is disabled or non-broad",
                    ),
                    PersistenceSide::not_applicable(
                        "planner artifacts are provider evidence files",
                    ),
                    "broad planning may write planner prompt and artifact files before child-plan admission",
                ),
                item(
                    "edit harness results",
                    optional_count_side(
                        messages
                            .as_ref()
                            .map(|root| root.join("edit-harness-result")),
                        |path| count_extension_files(path, "json"),
                        "not used by deterministic/non-broad child planning",
                    ),
                    PersistenceSide::not_applicable(
                        "admitted harness results are file evidence today",
                    ),
                    "admitted/rejected broad harness results feed child-plan membership",
                ),
                item(
                    "agent-turn bundles",
                    optional_count_side(
                        root.clone(),
                        |path| count_dirs_named(path, "turn-live"),
                        "not used when broad headless TUI does not run",
                    ),
                    db_side_when_file_present(
                        database,
                        "eval_agent_turn",
                        &root,
                        |root| count_dirs_named(root, "turn-live"),
                        "not used when broad headless TUI does not run",
                    ),
                    "headless-TUI turn-live bundles mirror into queryable eval_agent_turn rows",
                ),
                item(
                    "agent-turn events",
                    optional_count_side(
                        root.clone(),
                        |path| count_dirs_named(path, "turn-live"),
                        "not used when broad headless TUI does not run",
                    ),
                    db_side_when_file_present(
                        database,
                        "eval_agent_turn_event",
                        &root,
                        |root| count_dirs_named(root, "turn-live"),
                        "not used when broad headless TUI does not run",
                    ),
                    "turn summaries produce ordered event rows for conversation/tool timelines",
                ),
            ],
        ),
        checklist_transition(
            "r8->r9",
            WalkPhase::R8,
            WalkPhase::R9,
            vec![no_write_item(
                "child schedule",
                "shapes child budget and truncates in-memory plan facts without durable writes",
            )],
        ),
        checklist_transition(
            "r9->r10",
            WalkPhase::R9,
            WalkPhase::R10,
            vec![no_write_item(
                "selection strategy",
                "resolves successor-selection strategy in memory from admitted policy/profile inputs",
            )],
        ),
        checklist_transition(
            "r10->r11a",
            WalkPhase::R10,
            WalkPhase::R11a,
            vec![no_write_item(
                "rejected-only projection",
                "payload-only rejected attempts are projected into selection/report facts without new persistence",
            )],
        ),
        checklist_transition(
            "r10->r11",
            WalkPhase::R10,
            WalkPhase::R11,
            vec![
                item(
                    "materialize journal",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "materialize_branch")
                    }),
                    PersistenceSide::not_applicable(
                        "materialization journal entries are JSONL evidence",
                    ),
                    "C1->C2 records before/after materialization entries",
                ),
                item(
                    "child workspaces",
                    optional_count_side(
                        root.as_ref().map(|root| root.join("workspaces")),
                        count_dirs,
                        "workspaces may be cleaned after successful completion",
                    ),
                    PersistenceSide::not_applicable("worktree contents are artifact authority"),
                    "materialization writes child workspaces or broad harness workspace refs",
                ),
                item(
                    "artifact rows",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "child_artifact_committed")
                    }),
                    db_side(database, "eval_artifact", None, DbCompare::AnyRows),
                    "C1/broad and selected artifacts mirror into eval_artifact rows where DB exists",
                ),
                item(
                    "artifact surfaces",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "child_artifact_committed")
                    }),
                    db_side(database, "eval_artifact_surface", None, DbCompare::AnyRows),
                    "artifact surface commitments mirror into eval_artifact_surface rows",
                ),
                item(
                    "artifact refs",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "child_artifact_committed")
                    }),
                    db_side(database, "eval_artifact_ref", None, DbCompare::AnyRows),
                    "artifact citations mirror into eval_artifact_ref rows",
                ),
                item(
                    "operation rows",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "materialize_branch")
                    }),
                    db_side(database, "eval_operation", None, DbCompare::AnyRows),
                    "broad materialization operation provenance mirrors into eval_operation",
                ),
                item(
                    "patch rows",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "materialize_branch")
                    }),
                    db_side(database, "eval_patch", None, DbCompare::AnyRows),
                    "broad materialization patch provenance mirrors into eval_patch",
                ),
                item(
                    "apply rows",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "materialize_branch")
                    }),
                    db_side(database, "eval_apply_event", None, DbCompare::AnyRows),
                    "broad materialization apply provenance mirrors into eval_apply_event",
                ),
                item(
                    "build journal",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "build_child")
                    }),
                    db_side(database, "eval_build_event", None, DbCompare::AnyRows),
                    "C2->C3 and C3->C4 build/spawn provenance mirror into build rows",
                ),
                item(
                    "binary refs",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "build_child")
                    }),
                    db_side(database, "eval_binary_ref", None, DbCompare::AnyRows),
                    "promoted child binaries mirror into eval_binary_ref rows when retained long enough to hash",
                ),
                item(
                    "child invocations",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "invocations", Some("json"))
                    }),
                    db_side_for_file(database, "eval_invocation", &root, |root| {
                        count_files_named(&root.join("nodes"), "invocations", Some("json"))
                    }),
                    "child/successor invocation files mirror as eval_invocation rows",
                ),
                item(
                    "attempt rows",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "runner-result.json", None)
                    }),
                    db_side_for_file(database, "eval_attempt", &root, |root| {
                        count_files_named(&root.join("nodes"), "runner-result.json", None)
                    }),
                    "invocation/runner-result evidence mirrors as eval_attempt rows",
                ),
                item(
                    "result files",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "results", Some("json"))
                    }),
                    PersistenceSide::not_applicable("result JSON files are child runtime evidence"),
                    "child result payloads are written under nodes/<node>/results plus runner-result.json",
                ),
                item(
                    "channel messages",
                    count_side(nodes.clone(), count_channel_lines),
                    db_side_for_file(database, "eval_channel_message", &root, |root| {
                        count_channel_lines(&root.join("nodes"))
                    }),
                    "child-to-parent channel JSONL lines mirror as channel message rows",
                ),
                item(
                    "channel receipts",
                    count_side(nodes.clone(), count_channel_lines),
                    db_side(database, "eval_channel_receipt", None, DbCompare::AnyRows),
                    "parent channel imports mirror receipt rows where implemented",
                ),
                item(
                    "import events",
                    count_side(nodes.clone(), count_channel_lines),
                    db_side(database, "eval_import_event", None, DbCompare::AnyRows),
                    "parent channel imports mirror import-event rows where implemented",
                ),
                item(
                    "runtime streams",
                    optional_count_side(
                        nodes.clone(),
                        count_stream_logs,
                        "streams exist only after spawn",
                    ),
                    db_side(database, "eval_log_ref", None, DbCompare::AnyRows),
                    "stdout/stderr logs are file evidence; log-ref mirror is partial/optional",
                ),
                item(
                    "observe journal",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "observe_child")
                    }),
                    PersistenceSide::not_applicable("observe journal entries are JSONL evidence"),
                    "C4->C5 records child-completion observation entries",
                ),
                item(
                    "evaluation reports",
                    count_side(root.as_ref().map(|root| root.join("evaluations")), |path| {
                        count_extension_files(path, "json")
                    }),
                    db_side_for_file(database, "eval_evaluation", &root, |root| {
                        count_extension_files(&root.join("evaluations"), "json")
                    }),
                    "C5 comparison writes branch evaluation files and eval_evaluation rows",
                ),
                item(
                    "evaluation instances",
                    count_side(root.as_ref().map(|root| root.join("evaluations")), |path| {
                        count_extension_files(path, "json")
                    }),
                    db_side(
                        database,
                        "eval_evaluation_instance",
                        None,
                        DbCompare::AnyRows,
                    ),
                    "per-instance comparison details mirror into eval_evaluation_instance rows",
                ),
                item(
                    "branch registry",
                    count_side(
                        root.as_ref().map(|root| root.join("branches.json")),
                        |path| count_file_exists(path),
                    ),
                    PersistenceSide::not_applicable("branches.json is a compatibility projection"),
                    "comparison appends branch summary projection for reports/recovery",
                ),
                item(
                    "selection rows",
                    count_side(root.as_ref().map(|root| root.join("evaluations")), |path| {
                        count_extension_files(path, "json")
                    }),
                    db_side(
                        database,
                        "eval_selection_decision",
                        None,
                        DbCompare::AnyRows,
                    ),
                    "successor selection DB decision rows are emitted during selection inside r10->r11",
                ),
                item(
                    "selection candidates",
                    count_side(root.as_ref().map(|root| root.join("evaluations")), |path| {
                        count_extension_files(path, "json")
                    }),
                    db_side(
                        database,
                        "eval_selection_candidate",
                        None,
                        DbCompare::AnyRows,
                    ),
                    "candidate membership rows mirror selection material where implemented",
                ),
                item(
                    "selection findings",
                    count_side(root.as_ref().map(|root| root.join("evaluations")), |path| {
                        count_extension_files(path, "json")
                    }),
                    db_side(database, "eval_selection_finding", None, DbCompare::AnyRows),
                    "finding/rationale rows mirror selection diagnostics where implemented",
                ),
                item(
                    "selection scores",
                    count_side(root.as_ref().map(|root| root.join("evaluations")), |path| {
                        count_extension_files(path, "json")
                    }),
                    db_side(database, "eval_selection_score", None, DbCompare::AnyRows),
                    "score rows mirror traversal scoring where implemented",
                ),
                item(
                    "applied patch gate",
                    PersistenceSide::not_applicable(
                        "the applied gate is sealed in selection receipt authority",
                    ),
                    db_side(
                        database,
                        "eval_selection_patch_gate",
                        None,
                        DbCompare::AnyRows,
                    ),
                    "selection records the gate actually applied, separately from the admitted profile",
                ),
                item(
                    "candidate patch reviews",
                    reviews.review_file.clone(),
                    reviews.review_db.clone(),
                    "artifact-bound review files mirror into normalized per-candidate DB rows",
                ),
                item(
                    "reviewed patch changes",
                    reviews.change_file.clone(),
                    reviews.change_db.clone(),
                    "each review exposes every admitted changed path and its before/after hashes",
                ),
            ],
        ),
        checklist_transition(
            "r11a->r12",
            WalkPhase::R11a,
            WalkPhase::R12,
            vec![no_write_item(
                "report facts",
                "projects rejected-only report facts in memory without durable writes",
            )],
        ),
        checklist_transition(
            "r11->r12",
            WalkPhase::R11,
            WalkPhase::R12,
            vec![no_write_item(
                "report facts",
                "projects child outcome report facts in memory; evaluation files were produced during r10->r11 child comparison",
            )],
        ),
        checklist_transition(
            "r12->r13a",
            WalkPhase::R12,
            WalkPhase::R13a,
            vec![
                item(
                    "stopped journal",
                    count_side(journal.clone(), |path| {
                        count_successor_state(path, "stopped")
                    }),
                    PersistenceSide::not_applicable(
                        "stopped continuation journal is JSONL evidence",
                    ),
                    "stopped/no-successor path appends stopped successor evidence when a selected branch is not handed off",
                ),
                item(
                    "typed stopped reconstruction",
                    stopped.clone(),
                    PersistenceSide::not_applicable(
                        "exact stopped-receipt validation is a typed file/journal reconstruction check",
                    ),
                    "the active parent's stopped record advances to R13a only when its continuation and selection receipt reconstruct exactly",
                ),
                item(
                    "continuation rows",
                    count_side(journal.clone(), count_continuation),
                    db_side_for_file(database, "eval_continuation_decision", &root, |root| {
                        count_continuation(&root.join("transition-journal.jsonl"))
                    }),
                    "continuation policy decision mirrors into eval_continuation_decision where implemented",
                ),
            ],
        ),
        checklist_transition(
            "r12->r13b",
            WalkPhase::R12,
            WalkPhase::R13b,
            vec![
                item(
                    "selected journal",
                    count_side(journal.clone(), |path| {
                        count_successor_state(path, "selected")
                    }),
                    PersistenceSide::not_applicable("selected successor journal is JSONL evidence"),
                    "selected successor evidence is appended before handoff branch work",
                ),
                item(
                    "continuation rows",
                    count_side(journal.clone(), count_continuation),
                    db_side_for_file(database, "eval_continuation_decision", &root, |root| {
                        count_continuation(&root.join("transition-journal.jsonl"))
                    }),
                    "continuation policy decision mirrors into eval_continuation_decision where implemented",
                ),
                item(
                    "checkout journal",
                    count_side(journal.clone(), |path| {
                        count_successor_state(path, "checkout")
                    }),
                    PersistenceSide::not_applicable(
                        "checkout evidence remains transition-journal authority",
                    ),
                    "handoff records before/after active checkout install entries",
                ),
                item(
                    "active checkout identity",
                    doc_side("parent_identity", docs),
                    PersistenceSide::not_applicable(
                        "active checkout identity is artifact authority",
                    ),
                    "selected successor parent_identity.json is written and committed in active checkout",
                ),
                item(
                    "active checkout advanced",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "active_checkout_advanced")
                    }),
                    PersistenceSide::not_applicable(
                        "active checkout advancement is journal/artifact authority",
                    ),
                    "handoff appends ActiveCheckoutAdvanced evidence after commit",
                ),
                item(
                    "history blocks",
                    count_side(
                        history
                            .as_ref()
                            .map(|root| root.join("blocks/segment-000000.jsonl")),
                        count_jsonl_rows,
                    ),
                    PersistenceSide::not_applicable(
                        "History blocks are authority, not generic eval-store rows",
                    ),
                    "handoff seals and appends History block authority",
                ),
                item(
                    "history indexes",
                    count_side(
                        history.as_ref().map(|root| root.join("index")),
                        count_all_files,
                    ),
                    PersistenceSide::not_applicable("History indexes are file projections"),
                    "History head/hash/lineage indexes are rebuilt as file projections",
                ),
                item(
                    "install artifact rows",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "active_checkout_advanced")
                    }),
                    db_side(database, "eval_artifact", None, DbCompare::AnyRows),
                    "selected active checkout artifact provenance mirrors into eval_artifact when DB exists",
                ),
                item(
                    "install surface rows",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "active_checkout_advanced")
                    }),
                    db_side(database, "eval_artifact_surface", None, DbCompare::AnyRows),
                    "selected active checkout surface mirrors into eval_artifact_surface",
                ),
                item(
                    "install ref rows",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "active_checkout_advanced")
                    }),
                    db_side(database, "eval_artifact_ref", None, DbCompare::AnyRows),
                    "selected active checkout ref mirrors into eval_artifact_ref",
                ),
                item(
                    "successor invocation",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "invocations", Some("json"))
                    }),
                    db_side_for_file(database, "eval_invocation", &root, |root| {
                        count_files_named(&root.join("nodes"), "invocations", Some("json"))
                    }),
                    "handoff writes executable successor invocation JSON and eval_invocation mirror",
                ),
                item(
                    "successor spawn journal",
                    count_side(journal.clone(), |path| {
                        count_successor_state(path, "spawned")
                    }),
                    PersistenceSide::not_applicable("successor spawn journal is JSONL evidence"),
                    "predecessor appends successor spawned evidence with pid/streams",
                ),
                item(
                    "successor ready channel",
                    count_side(nodes.clone(), count_channel_lines),
                    db_side_for_file(database, "eval_channel_message", &root, |root| {
                        count_channel_lines(&root.join("nodes"))
                    }),
                    "successor ready acknowledgement is transported through child-to-parent channel",
                ),
                item(
                    "successor handoff",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "successor_handoff")
                    }),
                    PersistenceSide::not_applicable("successor handoff journal is JSONL evidence"),
                    "handoff-ready acknowledgement appends SuccessorHandoffEntry",
                ),
            ],
        ),
        checklist_transition(
            "r12->r13c",
            WalkPhase::R12,
            WalkPhase::R13c,
            vec![
                item(
                    "selected journal",
                    count_side(journal.clone(), |path| {
                        count_successor_state(path, "selected")
                    }),
                    PersistenceSide::not_applicable("selected successor journal is JSONL evidence"),
                    "selected successor evidence precedes the incomplete handoff outcome",
                ),
                item(
                    "active checkout advanced",
                    count_side(journal.clone(), |path| {
                        count_journal_kind(path, "active_checkout_advanced")
                    }),
                    PersistenceSide::not_applicable(
                        "active checkout advancement is journal/artifact authority",
                    ),
                    "R13c retains the already-installed successor checkout rather than restoring R12",
                ),
                item(
                    "history blocks",
                    count_side(
                        history
                            .as_ref()
                            .map(|root| root.join("blocks/segment-000000.jsonl")),
                        count_jsonl_rows,
                    ),
                    PersistenceSide::not_applicable(
                        "sealed History blocks are authority, not eval-store rows",
                    ),
                    "R13c requires the sealed predecessor History advance",
                ),
                item(
                    "successor invocation",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "invocations", Some("json"))
                    }),
                    db_side_for_file(database, "eval_invocation", &root, |root| {
                        count_files_named(&root.join("nodes"), "invocations", Some("json"))
                    }),
                    "the incomplete attempt remains correlated to its executable invocation",
                ),
                item(
                    "successor spawn journal",
                    count_side(journal.clone(), |path| {
                        count_successor_state(path, "spawned")
                    }),
                    PersistenceSide::not_applicable("successor spawn journal is JSONL evidence"),
                    "predecessor retirement is correlated with the spawned runtime",
                ),
                item(
                    "incomplete successor outcome",
                    count_side(journal.clone(), |path| {
                        count_latest_outcome(path, &["timed_out", "exited_before_ready"])
                    }),
                    PersistenceSide::not_applicable(
                        "timeout or early exit is successor journal evidence",
                    ),
                    "exactly one timeout-or-exit path is sufficient incomplete handoff evidence; neither outcome is committed readiness",
                ),
            ],
        ),
        checklist_transition(
            "r13a->r14a",
            WalkPhase::R13a,
            WalkPhase::R14a,
            vec![
                item(
                    "parent-complete journal",
                    count_side(journal.clone(), |path| {
                        count_resource_phase(path, "parent_complete")
                    }),
                    PersistenceSide::not_applicable(
                        "parent-complete resource sample is JSONL evidence",
                    ),
                    "final stopped report appends parent-complete resource sample",
                ),
                item(
                    "state report",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "state-report.json", None)
                    }),
                    PersistenceSide::not_applicable("state report is file evidence/projection"),
                    "final stopped report writes nodes/<parent>/reports/state-report.json",
                ),
            ],
        ),
        checklist_transition(
            "r13b->r14b",
            WalkPhase::R13b,
            WalkPhase::R14b,
            vec![
                item(
                    "parent-complete journal",
                    count_side(journal.clone(), |path| {
                        count_resource_phase(path, "parent_complete")
                    }),
                    PersistenceSide::not_applicable(
                        "parent-complete resource sample is JSONL evidence",
                    ),
                    "final handoff report appends parent-complete resource sample",
                ),
                item(
                    "state report",
                    count_side(nodes.clone(), |path| {
                        count_files_named(path, "state-report.json", None)
                    }),
                    PersistenceSide::not_applicable("state report is file evidence/projection"),
                    "final handoff report writes nodes/<parent>/reports/state-report.json",
                ),
                item(
                    "successor completion",
                    count_side(journal.clone(), |path| {
                        count_successor_state(path, "completed")
                    }),
                    PersistenceSide::not_applicable(
                        "successor completion is channel/journal evidence",
                    ),
                    "handoff-launched successor writes bounded-turn completion evidence",
                ),
            ],
        ),
    ]
}

fn stopped_reconstruction_side(
    repo_root: &Path,
    campaign_id: Option<&CampaignId>,
) -> PersistenceSide {
    let Some(campaign_id) = campaign_id else {
        return PersistenceSide {
            status: PersistenceStatus::None,
            count: None,
            path: None,
            relation: None,
            detail: Some("campaign unresolved".to_string()),
        };
    };
    let Ok(manifest) = campaign_manifest_path(campaign_id) else {
        return PersistenceSide {
            status: PersistenceStatus::None,
            count: None,
            path: None,
            relation: None,
            detail: Some("campaign manifest unresolved".to_string()),
        };
    };
    let path = manifest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
        .join("transition-journal.jsonl");
    let journal = PrototypeJournal::new(&path);
    let entries = match journal.load_entries() {
        Ok(entries) => entries,
        Err(error) => {
            return PersistenceSide {
                status: PersistenceStatus::Error,
                count: None,
                path: Some(path),
                relation: None,
                detail: Some(format!("failed to load transition journal: {error}")),
            };
        }
    };
    let active = match identity::load_parent_identity_optional(repo_root) {
        Ok(Some(active)) => active,
        Ok(None) => {
            return PersistenceSide {
                status: PersistenceStatus::None,
                count: Some(0),
                path: Some(path),
                relation: None,
                detail: Some("active parent identity is unavailable".to_string()),
            };
        }
        Err(error) => {
            return PersistenceSide {
                status: PersistenceStatus::Error,
                count: None,
                path: Some(path),
                relation: None,
                detail: Some(format!("failed to load active parent identity: {error}")),
            };
        }
    };
    if active.campaign_id() != campaign_id {
        return PersistenceSide {
            status: PersistenceStatus::None,
            count: Some(0),
            path: Some(path),
            relation: None,
            detail: Some(format!(
                "active parent belongs to campaign '{}', not audited campaign '{}'",
                active.campaign_id(),
                campaign_id
            )),
        };
    }
    if !has_turn_stop(&entries, &active) {
        return PersistenceSide {
            status: PersistenceStatus::None,
            count: Some(0),
            path: Some(path),
            relation: None,
            detail: Some(format!(
                "active parent '{}' has no stopped continuation",
                active.node_id()
            )),
        };
    }
    match crate::cli::prototype1_state::driver::reconstruct::reconstruct_early(repo_root) {
        Ok(snapshot)
            if snapshot.campaign_id.as_ref() == Some(campaign_id)
                && snapshot.blockers.is_empty()
                && snapshot.state.as_ref().is_some_and(|state| {
                    matches!(state.phase(), WalkPhase::R13a | WalkPhase::R14a)
                }) =>
        {
            PersistenceSide {
                status: PersistenceStatus::Ok,
                count: Some(1),
                path: Some(path),
                relation: None,
                detail: Some(
                    "active parent stopped continuation passed exact typed reconstruction"
                        .to_string(),
                ),
            }
        }
        Ok(snapshot) => PersistenceSide {
            status: PersistenceStatus::Error,
            count: Some(1),
            path: Some(path),
            relation: None,
            detail: Some(if snapshot.blockers.is_empty() {
                "active parent stopped continuation did not reconstruct to R13a/R14a".to_string()
            } else {
                snapshot.blockers.join("; ")
            }),
        },
        Err(error) => PersistenceSide {
            status: PersistenceStatus::Error,
            count: Some(1),
            path: Some(path),
            relation: None,
            detail: Some(format!("stopped reconstruction failed: {error}")),
        },
    }
}

fn has_turn_stop(entries: &[JournalEntry], active: &identity::ParentIdentity) -> bool {
    let Some(turn_start) = entries.iter().rposition(|entry| {
        matches!(
            entry,
            JournalEntry::ParentStarted(started)
                if started.campaign_id == *active.campaign_id()
                    && started.parent_identity == *active
        )
    }) else {
        return false;
    };
    entries
        .iter()
        .skip(turn_start + 1)
        .any(|entry| match entry {
            JournalEntry::Successor(record) => {
                record.campaign_id == *active.campaign_id()
                    && matches!(&record.state, successor::State::Stopped { .. })
            }
            _ => false,
        })
}

fn checklist_transition(
    transition: &'static str,
    from: WalkPhase,
    to: WalkPhase,
    items: Vec<PersistenceItemAudit>,
) -> TransitionChecklist {
    let file_status = rollup_side(&items, |item| item.file.status);
    let db_status = rollup_side(&items, |item| item.database.status);
    let overall_status = rollup_side(&items, |item| item.overall_status);
    TransitionChecklist {
        transition: transition.to_string(),
        from,
        to,
        file_status,
        db_status,
        overall_status,
        items,
    }
}

fn item(
    name: &'static str,
    file: PersistenceSide,
    database: PersistenceSide,
    note: &'static str,
) -> PersistenceItemAudit {
    let overall_status = combined_status(file.status, database.status);
    PersistenceItemAudit {
        name: name.to_string(),
        file,
        database,
        overall_status,
        note: note.to_string(),
    }
}

fn no_write_item(name: &'static str, note: &'static str) -> PersistenceItemAudit {
    item(
        name,
        PersistenceSide::not_applicable("no file write in this transition"),
        PersistenceSide::not_applicable("no owner eval-store write in this transition"),
        note,
    )
}

fn rollup_side(
    items: &[PersistenceItemAudit],
    status: impl Fn(&PersistenceItemAudit) -> PersistenceStatus,
) -> PersistenceStatus {
    let statuses = items
        .iter()
        .map(status)
        .filter(|status| *status != PersistenceStatus::NotApplicable)
        .collect::<Vec<_>>();
    if statuses.is_empty() {
        return PersistenceStatus::NotApplicable;
    }
    if statuses.contains(&PersistenceStatus::Error) {
        return PersistenceStatus::Error;
    }
    if statuses.contains(&PersistenceStatus::Partial) {
        return PersistenceStatus::Partial;
    }
    if statuses.contains(&PersistenceStatus::None) {
        if statuses.contains(&PersistenceStatus::Ok) {
            PersistenceStatus::Partial
        } else {
            PersistenceStatus::None
        }
    } else {
        PersistenceStatus::Ok
    }
}

fn combined_status(file: PersistenceStatus, database: PersistenceStatus) -> PersistenceStatus {
    match (file, database) {
        (PersistenceStatus::Error, _) | (_, PersistenceStatus::Error) => PersistenceStatus::Error,
        (PersistenceStatus::None, PersistenceStatus::None) => PersistenceStatus::None,
        (PersistenceStatus::None, PersistenceStatus::NotApplicable) => PersistenceStatus::None,
        (PersistenceStatus::Ok, PersistenceStatus::NotApplicable) => PersistenceStatus::Ok,
        (PersistenceStatus::Partial, _) | (_, PersistenceStatus::Partial) => {
            PersistenceStatus::Partial
        }
        (PersistenceStatus::Ok, PersistenceStatus::Ok) => PersistenceStatus::Ok,
        (PersistenceStatus::Ok, PersistenceStatus::None)
        | (PersistenceStatus::None, PersistenceStatus::Ok) => PersistenceStatus::Partial,
        (PersistenceStatus::NotApplicable, PersistenceStatus::Ok) => PersistenceStatus::Ok,
        (PersistenceStatus::NotApplicable, PersistenceStatus::None) => PersistenceStatus::None,
        (PersistenceStatus::NotApplicable, PersistenceStatus::NotApplicable) => {
            PersistenceStatus::NotApplicable
        }
    }
}

impl PersistenceSide {
    fn not_applicable(detail: &'static str) -> Self {
        Self {
            status: PersistenceStatus::NotApplicable,
            count: None,
            path: None,
            relation: None,
            detail: Some(detail.to_string()),
        }
    }
}

#[derive(Clone, Copy)]
enum DbCompare {
    AnyRows,
    AtLeastFile(i64),
}

fn doc_side(name: &str, docs: &[DocumentAudit]) -> PersistenceSide {
    match docs.iter().find(|doc| doc.name == name) {
        Some(doc) => PersistenceSide {
            status: document_persistence_status(doc.status),
            count: Some(i64::from(doc.status == DocumentStatus::Ok)),
            path: Some(doc.path.clone()),
            relation: None,
            detail: doc.detail.clone(),
        },
        None => PersistenceSide {
            status: PersistenceStatus::None,
            count: Some(0),
            path: None,
            relation: None,
            detail: Some(format!("document audit entry '{name}' was not collected")),
        },
    }
}

fn document_persistence_status(status: DocumentStatus) -> PersistenceStatus {
    match status {
        DocumentStatus::Ok => PersistenceStatus::Ok,
        DocumentStatus::Missing | DocumentStatus::NotChecked => PersistenceStatus::None,
        DocumentStatus::ReadError | DocumentStatus::ParseError => PersistenceStatus::Error,
    }
}

fn db_side(
    database: &DatabaseAudit,
    relation: &'static str,
    compare: Option<DbCompare>,
    default_compare: DbCompare,
) -> PersistenceSide {
    let compare = compare.unwrap_or(default_compare);
    match relation_map(database).get(relation) {
        Some(row) if row.exists => {
            let (status, detail) = match row.count {
                Some(count) => (db_count_status(count, compare), row.detail.clone()),
                None => (PersistenceStatus::Error, row.detail.clone()),
            };
            PersistenceSide {
                status,
                count: row.count,
                path: database.path.clone(),
                relation: Some(relation.to_string()),
                detail,
            }
        }
        Some(row) => PersistenceSide {
            status: PersistenceStatus::None,
            count: Some(0),
            path: database.path.clone(),
            relation: Some(relation.to_string()),
            detail: row
                .detail
                .clone()
                .or_else(|| Some("relation missing".to_string())),
        },
        None => PersistenceSide {
            status: match database.status {
                DatabaseStatus::OpenError => PersistenceStatus::Error,
                _ => PersistenceStatus::None,
            },
            count: None,
            path: database.path.clone(),
            relation: Some(relation.to_string()),
            detail: database
                .detail
                .clone()
                .or_else(|| Some("relation not listed".to_string())),
        },
    }
}

fn db_side_for_file(
    database: &DatabaseAudit,
    relation: &'static str,
    proto_root: &Option<PathBuf>,
    count: impl Fn(&Path) -> Result<i64, String>,
) -> PersistenceSide {
    let compare = proto_root
        .as_deref()
        .and_then(|root| count(root).ok())
        .map(DbCompare::AtLeastFile);
    db_side(database, relation, compare, DbCompare::AnyRows)
}

fn db_side_when_file_present(
    database: &DatabaseAudit,
    relation: &'static str,
    proto_root: &Option<PathBuf>,
    count: impl Fn(&Path) -> Result<i64, String>,
    empty_detail: &'static str,
) -> PersistenceSide {
    match proto_root.as_deref().and_then(|root| count(root).ok()) {
        Some(value) if value > 0 => db_side(database, relation, None, DbCompare::AnyRows),
        _ => PersistenceSide::not_applicable(empty_detail),
    }
}

fn db_count_status(count: i64, compare: DbCompare) -> PersistenceStatus {
    match compare {
        DbCompare::AnyRows => {
            if count > 0 {
                PersistenceStatus::Ok
            } else {
                PersistenceStatus::None
            }
        }
        DbCompare::AtLeastFile(file_count) => {
            if count == 0 {
                PersistenceStatus::None
            } else if count < file_count {
                PersistenceStatus::Partial
            } else {
                PersistenceStatus::Ok
            }
        }
    }
}

fn relation_map(database: &DatabaseAudit) -> BTreeMap<&str, &RelationCount> {
    database
        .relation_counts
        .iter()
        .map(|row| (row.relation.as_str(), row))
        .collect()
}

fn count_side(
    path: Option<PathBuf>,
    count: impl Fn(&Path) -> Result<i64, String>,
) -> PersistenceSide {
    let Some(path) = path else {
        return PersistenceSide {
            status: PersistenceStatus::None,
            count: None,
            path: None,
            relation: None,
            detail: Some("campaign path unresolved".to_string()),
        };
    };
    match count(&path) {
        Ok(value) => PersistenceSide {
            status: if value > 0 {
                PersistenceStatus::Ok
            } else {
                PersistenceStatus::None
            },
            count: Some(value),
            path: Some(path),
            relation: None,
            detail: None,
        },
        Err(error) => PersistenceSide {
            status: PersistenceStatus::Error,
            count: None,
            path: Some(path),
            relation: None,
            detail: Some(error),
        },
    }
}

fn optional_count_side(
    path: Option<PathBuf>,
    count: impl Fn(&Path) -> Result<i64, String>,
    empty_detail: &'static str,
) -> PersistenceSide {
    let mut side = count_side(path, count);
    if side.status == PersistenceStatus::None && side.count == Some(0) {
        side.status = PersistenceStatus::NotApplicable;
        side.detail = Some(empty_detail.to_string());
    }
    side
}

fn baseline_source_side(
    campaign_id: Option<&CampaignId>,
    proto_root: Option<&Path>,
) -> PersistenceSide {
    let mut count = 0_i64;
    let mut detail = Vec::new();
    if let Some(campaign_id) = campaign_id {
        match campaign_closure_state_path(campaign_id) {
            Ok(path) if path.exists() => {
                count += 1;
                detail.push(format!("closure_state={}", path.display()));
            }
            Ok(path) => detail.push(format!("closure_state_missing={}", path.display())),
            Err(error) => detail.push(format!("closure_state_error={error}")),
        }
    }
    if let Some(root) = proto_root {
        match count_extension_files(&root.join("evaluations"), "json") {
            Ok(value) => {
                count += value;
                detail.push(format!("evaluation_reports={value}"));
            }
            Err(error) => detail.push(format!("evaluation_report_error={error}")),
        }
    }
    PersistenceSide {
        status: if count > 0 {
            PersistenceStatus::Ok
        } else {
            PersistenceStatus::None
        },
        count: Some(count),
        path: proto_root.map(Path::to_path_buf),
        relation: None,
        detail: Some(detail.join("; ")),
    }
}

fn prototype_root_for_manifest(path: &PathBuf) -> PathBuf {
    path.parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

fn count_extension_files(path: &Path, extension: &str) -> Result<i64, String> {
    count_files(path, &mut |candidate| {
        candidate.extension().and_then(|ext| ext.to_str()) == Some(extension)
    })
}

fn count_files_named(path: &Path, name: &str, extension: Option<&str>) -> Result<i64, String> {
    count_files(path, &mut |candidate| {
        let name_matches = candidate
            .file_name()
            .and_then(|file| file.to_str())
            .is_some_and(|file| file == name);
        let parent_matches = candidate
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|file| file.to_str())
            .is_some_and(|file| file == name);
        let extension_matches = extension
            .is_none_or(|ext| candidate.extension().and_then(|value| value.to_str()) == Some(ext));
        (name_matches || parent_matches) && extension_matches
    })
}

fn count_file_exists(path: &Path) -> Result<i64, String> {
    match fs::metadata(path) {
        Ok(meta) => Ok(i64::from(meta.is_file())),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(source) => Err(source.to_string()),
    }
}

fn count_all_files(path: &Path) -> Result<i64, String> {
    count_files(path, &mut |_| true)
}

fn count_dirs(path: &Path) -> Result<i64, String> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => return Err(source.to_string()),
    };
    if !meta.is_dir() {
        return Ok(0);
    }
    let mut total = 0_i64;
    for entry in fs::read_dir(path).map_err(|source| source.to_string())? {
        let entry = entry.map_err(|source| source.to_string())?;
        let meta = entry.metadata().map_err(|source| source.to_string())?;
        if meta.is_dir() {
            total += 1;
            total += count_dirs(&entry.path())?;
        }
    }
    Ok(total)
}

fn count_dirs_named(path: &Path, name: &str) -> Result<i64, String> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => return Err(source.to_string()),
    };
    if !meta.is_dir() {
        return Ok(0);
    }
    let mut total = i64::from(
        path.file_name()
            .and_then(|file| file.to_str())
            .is_some_and(|file| file == name || file.ends_with(&format!(".{name}"))),
    );
    for entry in fs::read_dir(path).map_err(|source| source.to_string())? {
        let entry = entry.map_err(|source| source.to_string())?;
        let entry_path = entry.path();
        let meta = entry.metadata().map_err(|source| source.to_string())?;
        if meta.is_dir() && !skip_audit_descent(&entry_path) {
            total += count_dirs_named(&entry_path, name)?;
        }
    }
    Ok(total)
}

fn count_files(path: &Path, predicate: &mut dyn FnMut(&Path) -> bool) -> Result<i64, String> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => return Err(source.to_string()),
    };
    if meta.is_file() {
        return Ok(i64::from(predicate(path)));
    }
    let mut total = 0_i64;
    let entries = fs::read_dir(path).map_err(|source| source.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|source| source.to_string())?;
        let entry_path = entry.path();
        let meta = entry.metadata().map_err(|source| source.to_string())?;
        if meta.is_dir() {
            if !skip_audit_descent(&entry_path) {
                total += count_files(&entry_path, predicate)?;
            }
        } else if predicate(&entry_path) {
            total += 1;
        }
    }
    Ok(total)
}

fn skip_audit_descent(path: &Path) -> bool {
    path.file_name()
        .and_then(|file| file.to_str())
        .is_some_and(|file| file == "worktree")
}

fn count_jsonl_rows(path: &Path) -> Result<i64, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => return Err(source.to_string()),
    };
    Ok(text.lines().filter(|line| !line.trim().is_empty()).count() as i64)
}

fn count_journal_kind(path: &Path, expected: &str) -> Result<i64, String> {
    count_journal(path, |value| journal_kind(value) == Some(expected))
}

fn count_successor_state(path: &Path, state: &str) -> Result<i64, String> {
    count_journal(path, |value| {
        journal_kind(value) == Some("successor")
            && value
                .get("state")
                .and_then(|state_value| state_value.get(state))
                .is_some()
    })
}

fn count_latest_outcome(path: &Path, outcomes: &[&str]) -> Result<i64, String> {
    let entries = PrototypeJournal::new(path.to_path_buf())
        .load_entries()
        .map_err(|source| source.to_string())?;
    let Some(node_id) = entries.iter().rev().find_map(|entry| match entry {
        JournalEntry::ActiveCheckoutAdvanced(entry) => {
            Some(entry.selected_parent_identity.node_id().to_string())
        }
        _ => None,
    }) else {
        return Ok(0);
    };

    let mut attempt = None;
    let mut incomplete = false;
    for entry in &entries {
        let JournalEntry::Successor(record) = entry else {
            continue;
        };
        if record.node_id != node_id {
            continue;
        }
        let key = record
            .runtime_id
            .map(|runtime| (runtime, record.node_id.as_str()));
        match &record.state {
            successor::State::Spawned { .. } => {
                attempt = key;
                incomplete = false;
            }
            successor::State::TimedOut { .. } => {
                if key == attempt {
                    incomplete = outcomes.contains(&"timed_out");
                }
            }
            successor::State::ExitedBeforeReady { .. } => {
                if key == attempt {
                    incomplete = outcomes.contains(&"exited_before_ready");
                }
            }
            successor::State::Ready { .. } | successor::State::Completed { .. } => {
                if key == attempt && !incomplete {
                    incomplete = false;
                }
            }
            successor::State::Selected { .. }
            | successor::State::Stopped { .. }
            | successor::State::Checkout { .. } => {}
        }
    }
    Ok(i64::from(incomplete))
}

fn count_resource_phase(path: &Path, expected: &str) -> Result<i64, String> {
    count_journal(path, |value| {
        journal_kind(value) == Some("resource")
            && value.get("phase").and_then(|value| value.as_str()) == Some(expected)
    })
}

fn count_journal(
    path: &Path,
    mut accepts: impl FnMut(&serde_json::Value) -> bool,
) -> Result<i64, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => return Err(source.to_string()),
    };
    let mut total = 0_i64;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(trimmed).map_err(|source| source.to_string())?;
        if accepts(&value) {
            total += 1;
        }
    }
    Ok(total)
}

fn journal_kind(value: &serde_json::Value) -> Option<&str> {
    value.get("kind").and_then(|value| value.as_str())
}

fn count_parent_start(path: &Path) -> Result<i64, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => return Err(source.to_string()),
    };
    let mut total = 0_i64;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(trimmed).map_err(|source| source.to_string())?;
        let kind = value.get("kind").and_then(|value| value.as_str());
        let phase = value.get("phase").and_then(|value| value.as_str());
        if kind == Some("parent_started")
            || (kind == Some("resource") && phase == Some("parent_start"))
        {
            total += 1;
        }
    }
    Ok(total)
}

fn count_continuation(path: &Path) -> Result<i64, String> {
    count_journal(path, |value| {
        journal_kind(value) == Some("successor")
            && value.get("state").is_some_and(|state| {
                state.get("selected").is_some() || state.get("stopped").is_some()
            })
    })
}

fn count_channel_lines(path: &Path) -> Result<i64, String> {
    count_jsonl_named(path, "child-to-parent.jsonl")
}

fn count_stream_logs(path: &Path) -> Result<i64, String> {
    count_files(path, &mut |candidate| {
        candidate
            .file_name()
            .and_then(|file| file.to_str())
            .is_some_and(|file| file == "stdout.log" || file == "stderr.log")
    })
}

fn count_jsonl_named(path: &Path, name: &str) -> Result<i64, String> {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(source) => return Err(source.to_string()),
    };
    if meta.is_file() {
        return if path.file_name().and_then(|file| file.to_str()) == Some(name) {
            count_jsonl_rows(path)
        } else {
            Ok(0)
        };
    }
    let mut total = 0_i64;
    for entry in fs::read_dir(path).map_err(|source| source.to_string())? {
        let entry = entry.map_err(|source| source.to_string())?;
        let entry_path = entry.path();
        let meta = entry.metadata().map_err(|source| source.to_string())?;
        if meta.is_dir() {
            if !skip_audit_descent(&entry_path) {
                total += count_jsonl_named(&entry_path, name)?;
            }
        } else if entry_path.file_name().and_then(|file| file.to_str()) == Some(name) {
            total += count_jsonl_rows(&entry_path)?;
        }
    }
    Ok(total)
}

fn format_side(label: &str, side: &PersistenceSide) -> String {
    let count = side
        .count
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string());
    let path = side
        .path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "-".to_string());
    let relation = side.relation.as_deref().unwrap_or("-");
    let detail = side.detail.as_deref().unwrap_or("-");
    format!(
        "      {label}: status={} count={} path={} relation={} detail={}",
        persistence_label(side.status),
        count,
        path,
        relation,
        detail
    )
}

fn resolve_campaign(repo_root: &std::path::Path, campaign: Option<CampaignId>) -> CampaignAudit {
    if let Some(campaign_id) = campaign {
        return CampaignAudit {
            campaign_id: Some(campaign_id),
            source: CampaignSource::Explicit,
            detail: None,
        };
    }

    match identity::load_parent_identity_optional(repo_root) {
        Ok(Some(identity)) => CampaignAudit {
            campaign_id: Some(identity.campaign_id().clone()),
            source: CampaignSource::ParentIdentity,
            detail: Some("inferred from .ploke/prototype1/parent_identity.json".to_string()),
        },
        Ok(None) => CampaignAudit {
            campaign_id: None,
            source: CampaignSource::Unresolved,
            detail: Some(
                "pass --campaign or run from a checkout with parent_identity.json".to_string(),
            ),
        },
        Err(error) => CampaignAudit {
            campaign_id: None,
            source: CampaignSource::Unresolved,
            detail: Some(error.to_string()),
        },
    }
}

fn check_campaign_manifest(path: PathBuf) -> DocumentAudit {
    let mut doc = check_json_doc(
        "campaign_manifest",
        path.clone(),
        DocumentRole::Authority,
        DocumentExpectation::Required,
        ExpectedAt::Precondition,
    );
    if doc.status == DocumentStatus::Ok {
        if let Some(parent) = path.parent().and_then(|dir| dir.file_name()) {
            let campaign_id = CampaignId::from(parent.to_string_lossy().as_ref());
            if let Err(error) = load_campaign_manifest(&campaign_id) {
                doc.status = DocumentStatus::ParseError;
                doc.detail = Some(error.to_string());
            }
        }
    }
    doc
}

fn check_directory(
    name: &'static str,
    path: PathBuf,
    role: DocumentRole,
    expectation: DocumentExpectation,
    expected_at: ExpectedAt,
) -> DocumentAudit {
    match fs::metadata(&path) {
        Ok(meta) if meta.is_dir() => DocumentAudit {
            name: name.to_string(),
            path,
            role,
            expectation,
            expected_at,
            exists: true,
            status: DocumentStatus::Ok,
            sha256: None,
            detail: None,
        },
        Ok(_) => DocumentAudit {
            name: name.to_string(),
            path,
            role,
            expectation,
            expected_at,
            exists: true,
            status: DocumentStatus::ParseError,
            sha256: None,
            detail: Some("path exists but is not a directory".to_string()),
        },
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => DocumentAudit {
            name: name.to_string(),
            path,
            role,
            expectation,
            expected_at,
            exists: false,
            status: DocumentStatus::Missing,
            sha256: None,
            detail: None,
        },
        Err(source) => DocumentAudit {
            name: name.to_string(),
            path,
            role,
            expectation,
            expected_at,
            exists: false,
            status: DocumentStatus::ReadError,
            sha256: None,
            detail: Some(source.to_string()),
        },
    }
}

fn check_json_doc(
    name: &'static str,
    path: PathBuf,
    role: DocumentRole,
    expectation: DocumentExpectation,
    expected_at: ExpectedAt,
) -> DocumentAudit {
    check_file(name, path, role, expectation, expected_at, ParseKind::Json)
}

fn check_toml_doc(
    name: &'static str,
    path: PathBuf,
    role: DocumentRole,
    expectation: DocumentExpectation,
    expected_at: ExpectedAt,
) -> DocumentAudit {
    check_file(name, path, role, expectation, expected_at, ParseKind::Toml)
}

fn check_jsonl_doc(
    name: &'static str,
    path: PathBuf,
    role: DocumentRole,
    expectation: DocumentExpectation,
    expected_at: ExpectedAt,
) -> DocumentAudit {
    check_file(name, path, role, expectation, expected_at, ParseKind::Jsonl)
}

#[derive(Clone, Copy)]
enum ParseKind {
    Json,
    Jsonl,
    Toml,
}

fn check_file(
    name: &'static str,
    path: PathBuf,
    role: DocumentRole,
    expectation: DocumentExpectation,
    expected_at: ExpectedAt,
    kind: ParseKind,
) -> DocumentAudit {
    match fs::read(&path) {
        Ok(bytes) => {
            let sha256 = Some(sha256_hex(&bytes));
            let parse = match kind {
                ParseKind::Json => serde_json::from_slice::<serde_json::Value>(&bytes)
                    .map(|_| ())
                    .map_err(|source| source.to_string()),
                ParseKind::Jsonl => std::str::from_utf8(&bytes)
                    .map_err(|source| source.to_string())
                    .and_then(parse_jsonl),
                ParseKind::Toml => std::str::from_utf8(&bytes)
                    .map_err(|source| source.to_string())
                    .and_then(|text| {
                        toml::from_str::<toml::Value>(text)
                            .map(|_| ())
                            .map_err(|source| source.to_string())
                    }),
            };
            match parse {
                Ok(()) => DocumentAudit {
                    name: name.to_string(),
                    path,
                    role,
                    expectation,
                    expected_at,
                    exists: true,
                    status: DocumentStatus::Ok,
                    sha256,
                    detail: None,
                },
                Err(error) => DocumentAudit {
                    name: name.to_string(),
                    path,
                    role,
                    expectation,
                    expected_at,
                    exists: true,
                    status: DocumentStatus::ParseError,
                    sha256,
                    detail: Some(error),
                },
            }
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => DocumentAudit {
            name: name.to_string(),
            path,
            role,
            expectation,
            expected_at,
            exists: false,
            status: DocumentStatus::Missing,
            sha256: None,
            detail: None,
        },
        Err(source) => DocumentAudit {
            name: name.to_string(),
            path,
            role,
            expectation,
            expected_at,
            exists: false,
            status: DocumentStatus::ReadError,
            sha256: None,
            detail: Some(source.to_string()),
        },
    }
}

fn error_doc(
    name: &'static str,
    path: PathBuf,
    role: DocumentRole,
    expectation: DocumentExpectation,
    expected_at: ExpectedAt,
    detail: String,
) -> DocumentAudit {
    DocumentAudit {
        name: name.to_string(),
        path,
        role,
        expectation,
        expected_at,
        exists: false,
        status: DocumentStatus::ReadError,
        sha256: None,
        detail: Some(detail),
    }
}

fn database_audit(campaign_id: Option<&CampaignId>) -> (DatabaseAudit, ReviewMirrorAudit) {
    let Some(campaign_id) = campaign_id else {
        return (
            DatabaseAudit {
                path: None,
                exists: false,
                status: DatabaseStatus::CampaignUnresolved,
                relation_counts: Vec::new(),
                detail: Some(
                    "campaign unresolved; cannot derive prototype1/eval-store.cozo.sqlite"
                        .to_string(),
                ),
            },
            unavailable_review_mirror("campaign unresolved"),
        );
    };

    let manifest = match campaign_manifest_path(campaign_id) {
        Ok(path) => path,
        Err(error) => {
            return (
                DatabaseAudit {
                    path: None,
                    exists: false,
                    status: DatabaseStatus::OpenError,
                    relation_counts: Vec::new(),
                    detail: Some(error.to_string()),
                },
                unavailable_review_mirror("campaign manifest unresolved"),
            );
        }
    };
    let (reviews, mut mirror) = review_file_mirror(&manifest);
    let path = prototype1_eval_store_db_path(&manifest);
    if !path.exists() {
        update_missing_db(&mut mirror, &reviews, &path);
        return (
            DatabaseAudit {
                path: Some(path),
                exists: false,
                status: DatabaseStatus::Missing,
                relation_counts: relation_count_missing(),
                detail: None,
            },
            mirror,
        );
    }

    let db = match load_owner_eval_database(&path) {
        Ok(db) => db,
        Err(error) => {
            update_db_error(&mut mirror, &path, error.to_string());
            return (
                DatabaseAudit {
                    path: Some(path),
                    exists: true,
                    status: DatabaseStatus::OpenError,
                    relation_counts: Vec::new(),
                    detail: Some(error.to_string()),
                },
                mirror,
            );
        }
    };
    match load_relation_counts(&db) {
        Ok(relation_counts) => {
            update_db_mirror(&mut mirror, &reviews, &db, &path, &relation_counts);
            (
                DatabaseAudit {
                    path: Some(path),
                    exists: true,
                    status: DatabaseStatus::Ok,
                    relation_counts,
                    detail: None,
                },
                mirror,
            )
        }
        Err(error) => {
            update_db_error(&mut mirror, &path, error.clone());
            (
                DatabaseAudit {
                    path: Some(path),
                    exists: true,
                    status: DatabaseStatus::OpenError,
                    relation_counts: Vec::new(),
                    detail: Some(error),
                },
                mirror,
            )
        }
    }
}

fn load_relation_counts(db: &Database) -> Result<Vec<RelationCount>, String> {
    let relations = db
        .raw_query_params("::relations", BTreeMap::new())
        .map_err(|source| source.to_string())?;
    let names: BTreeSet<String> = relations
        .rows
        .iter()
        .filter_map(|row| row.first().and_then(DataValue::get_str).map(str::to_string))
        .collect();

    let mut counts = Vec::new();
    for (rel, key) in EVAL_RELS {
        if !names.contains(*rel) {
            counts.push(RelationCount {
                relation: (*rel).to_string(),
                exists: false,
                count: None,
                detail: None,
            });
            continue;
        }
        let query = format!("?[count(x)] := *{rel} {{ {key}: x }}");
        match db.raw_query_params(&query, BTreeMap::new()) {
            Ok(result) => counts.push(RelationCount {
                relation: (*rel).to_string(),
                exists: true,
                count: result
                    .rows
                    .first()
                    .and_then(|row| row.first())
                    .and_then(DataValue::get_int),
                detail: None,
            }),
            Err(error) => counts.push(RelationCount {
                relation: (*rel).to_string(),
                exists: true,
                count: None,
                detail: Some(error.to_string()),
            }),
        }
    }
    Ok(counts)
}

fn review_file_mirror(manifest: &Path) -> (Result<Vec<PatchReview>, String>, ReviewMirrorAudit) {
    let path = manifest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
        .join("reviews");
    match candidate_review::load_review_inventory(manifest).map_err(|error| error.to_string()) {
        Ok(reviews) => {
            let review_count = match i64::try_from(reviews.len()) {
                Ok(count) => count,
                Err(error) => {
                    let detail = format!("candidate review count exceeds audit range: {error}");
                    return (Err(detail.clone()), review_error_mirror(path, detail));
                }
            };
            let changes = match reviews.iter().try_fold(0_usize, |total, review| {
                total.checked_add(review.changes.len())
            }) {
                Some(count) => count,
                None => {
                    let detail = "candidate review change count overflow".to_string();
                    return (Err(detail.clone()), review_error_mirror(path, detail));
                }
            };
            let change_count = match i64::try_from(changes) {
                Ok(count) => count,
                Err(error) => {
                    let detail =
                        format!("candidate review change count exceeds audit range: {error}");
                    return (Err(detail.clone()), review_error_mirror(path, detail));
                }
            };
            let empty = reviews.is_empty();
            let file_status = if empty {
                PersistenceStatus::NotApplicable
            } else {
                PersistenceStatus::Ok
            };
            let file_detail = if empty {
                "not used when patch review is disabled or no candidate reaches review".to_string()
            } else {
                "typed candidate review evidence validated against admitted configuration"
                    .to_string()
            };
            (
                Ok(reviews),
                ReviewMirrorAudit {
                    review_file: PersistenceSide {
                        status: file_status,
                        count: Some(review_count),
                        path: Some(path.clone()),
                        relation: None,
                        detail: Some(file_detail.clone()),
                    },
                    review_db: PersistenceSide::not_applicable(
                        "candidate review database mirror not checked",
                    ),
                    change_file: PersistenceSide {
                        status: file_status,
                        count: Some(change_count),
                        path: Some(path),
                        relation: None,
                        detail: Some(file_detail),
                    },
                    change_db: PersistenceSide::not_applicable(
                        "candidate review database mirror not checked",
                    ),
                },
            )
        }
        Err(detail) => (Err(detail.clone()), review_error_mirror(path, detail)),
    }
}

fn unavailable_review_mirror(detail: &'static str) -> ReviewMirrorAudit {
    ReviewMirrorAudit {
        review_file: PersistenceSide::not_applicable(detail),
        review_db: PersistenceSide::not_applicable(detail),
        change_file: PersistenceSide::not_applicable(detail),
        change_db: PersistenceSide::not_applicable(detail),
    }
}

fn review_error_mirror(path: PathBuf, detail: String) -> ReviewMirrorAudit {
    let file = PersistenceSide {
        status: PersistenceStatus::Error,
        count: None,
        path: Some(path),
        relation: None,
        detail: Some(detail.clone()),
    };
    ReviewMirrorAudit {
        review_file: file.clone(),
        review_db: PersistenceSide {
            status: PersistenceStatus::Error,
            count: None,
            path: None,
            relation: Some("eval_selection_patch_review".to_string()),
            detail: Some(format!(
                "database mirror cannot be verified because file evidence is invalid: {detail}"
            )),
        },
        change_file: file,
        change_db: PersistenceSide {
            status: PersistenceStatus::Error,
            count: None,
            path: None,
            relation: Some("eval_selection_patch_change".to_string()),
            detail: Some(format!(
                "database mirror cannot be verified because file evidence is invalid: {detail}"
            )),
        },
    }
}

fn update_missing_db(
    mirror: &mut ReviewMirrorAudit,
    reviews: &Result<Vec<PatchReview>, String>,
    path: &Path,
) {
    let Ok(reviews) = reviews else {
        return;
    };
    if reviews.is_empty() {
        mirror.review_db = PersistenceSide::not_applicable("no candidate review evidence");
        mirror.change_db = PersistenceSide::not_applicable("no candidate review evidence");
        return;
    }
    let pending = reviews
        .iter()
        .map(|review| review.citation.ref_id.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let detail =
        format!("pending: valid candidate review citations await selection DB linking: {pending}");
    mirror.review_db = PersistenceSide {
        status: PersistenceStatus::Partial,
        count: Some(0),
        path: Some(path.to_path_buf()),
        relation: Some("eval_selection_patch_review".to_string()),
        detail: Some(detail.clone()),
    };
    mirror.change_db = PersistenceSide {
        status: PersistenceStatus::Partial,
        count: Some(0),
        path: Some(path.to_path_buf()),
        relation: Some("eval_selection_patch_change".to_string()),
        detail: Some(detail),
    };
}

fn update_db_error(mirror: &mut ReviewMirrorAudit, path: &Path, detail: String) {
    mirror.review_db = PersistenceSide {
        status: PersistenceStatus::Error,
        count: None,
        path: Some(path.to_path_buf()),
        relation: Some("eval_selection_patch_review".to_string()),
        detail: Some(detail.clone()),
    };
    mirror.change_db = PersistenceSide {
        status: PersistenceStatus::Error,
        count: None,
        path: Some(path.to_path_buf()),
        relation: Some("eval_selection_patch_change".to_string()),
        detail: Some(detail),
    };
}

fn update_db_mirror(
    mirror: &mut ReviewMirrorAudit,
    reviews: &Result<Vec<PatchReview>, String>,
    db: &Database,
    path: &Path,
    counts: &[RelationCount],
) {
    let Ok(reviews) = reviews else {
        return;
    };
    let relation = |name: &str| counts.iter().find(|row| row.relation == name);
    let decision_rel = relation("eval_selection_decision");
    let receipt_rel = relation("eval_selection_receipt");
    let candidate_rel = relation("eval_selection_candidate");
    let gate_rel = relation("eval_selection_patch_gate");
    let review_rel = relation("eval_selection_patch_review");
    let change_rel = relation("eval_selection_patch_change");
    let stored = [
        decision_rel,
        receipt_rel,
        candidate_rel,
        gate_rel,
        review_rel,
        change_rel,
    ]
    .into_iter()
    .flatten()
    .any(|row| row.count.is_some_and(|count| count > 0));
    if reviews.is_empty() && !stored {
        mirror.review_db = PersistenceSide::not_applicable("no candidate review evidence");
        mirror.change_db = PersistenceSide::not_applicable("no candidate review evidence");
        return;
    }
    let relations_ready = [
        decision_rel,
        receipt_rel,
        candidate_rel,
        gate_rel,
        review_rel,
        change_rel,
    ]
    .into_iter()
    .all(|row| row.is_some_and(|row| row.exists && row.count.is_some() && row.detail.is_none()));
    if !relations_ready {
        update_db_error(
            mirror,
            path,
            "candidate review normalized relations are missing or unreadable".to_string(),
        );
        return;
    }
    match verify_review_mirror(db, reviews) {
        Ok(coverage) => update_db_coverage(mirror, path, &coverage),
        Err(detail) => {
            if reviews.is_empty() && stored {
                let file_detail = format!(
                    "typed filesystem review evidence is required by selection DB authority: {detail}"
                );
                mirror.review_file.status = PersistenceStatus::Error;
                mirror.review_file.detail = Some(file_detail.clone());
                mirror.change_file.status = PersistenceStatus::Error;
                mirror.change_file.detail = Some(file_detail);
            }
            update_db_error(mirror, path, detail);
        }
    }
}

fn update_db_coverage(mirror: &mut ReviewMirrorAudit, path: &Path, coverage: &PatchMirrorCoverage) {
    if coverage.review_count == 0
        && coverage.change_count == 0
        && coverage.review_rows == 0
        && coverage.change_rows == 0
    {
        mirror.review_db = PersistenceSide::not_applicable("no candidate review evidence");
        mirror.change_db = PersistenceSide::not_applicable("no candidate review evidence");
        return;
    }
    let status = if coverage.pending_refs.is_empty() {
        PersistenceStatus::Ok
    } else {
        PersistenceStatus::Partial
    };
    let detail = if coverage.pending_refs.is_empty() {
        format!(
            "exact typed mirror: reviews={}/{}, review_rows={}, changes={}/{}, change_rows={}",
            coverage.review_count,
            coverage.review_count,
            coverage.review_rows,
            coverage.change_count,
            coverage.change_count,
            coverage.change_rows
        )
    } else {
        format!(
            "pending: {} valid candidate review citation(s) await selection linking: {}",
            coverage.pending_refs.len(),
            coverage.pending_refs.join(", ")
        )
    };
    mirror.review_db = PersistenceSide {
        status,
        count: i64::try_from(coverage.review_rows).ok(),
        path: Some(path.to_path_buf()),
        relation: Some("eval_selection_patch_review".to_string()),
        detail: Some(detail.clone()),
    };
    mirror.change_db = PersistenceSide {
        status,
        count: i64::try_from(coverage.change_rows).ok(),
        path: Some(path.to_path_buf()),
        relation: Some("eval_selection_patch_change".to_string()),
        detail: Some(detail),
    };
}

fn verify_review_mirror(
    db: &Database,
    reviews: &[PatchReview],
) -> Result<PatchMirrorCoverage, String> {
    let mut inventory = BTreeMap::new();
    let mut change_count = 0_usize;
    for review in reviews {
        let citation = review.citation.ref_id.clone();
        if citation.is_empty() {
            return Err("candidate review inventory contains an empty citation".to_string());
        }
        if inventory.insert(citation.clone(), review).is_some() {
            return Err(format!(
                "candidate review inventory contains duplicate citation '{citation}'"
            ));
        }
        change_count = change_count
            .checked_add(review.changes.len())
            .ok_or_else(|| "candidate review change count overflow".to_string())?;
    }

    let receipt_rows = db
        .raw_query_params(
            r#"
?[decision_id, campaign_id, parent_id, decision_hash, entry_json] :=
    *eval_selection_receipt {
        decision_id,
        campaign_id,
        parent_id,
        decision_hash,
        entry_json,
    }
"#,
            BTreeMap::new(),
        )
        .map_err(|error| format!("failed to read typed selection receipts: {error}"))?;
    let mut receipt_ids = BTreeSet::new();
    let mut bindings = BTreeMap::new();
    let mut strict_receipts = BTreeSet::new();
    let mut occurrences = BTreeMap::new();
    for row in receipt_rows.row_refs() {
        let decision_id = row
            .get::<String>("decision_id")
            .map_err(|error| format!("failed to decode selection receipt decision: {error}"))?;
        let campaign_id = row
            .get::<String>("campaign_id")
            .map_err(|error| format!("failed to decode selection receipt campaign: {error}"))?;
        let parent_id = row
            .get::<String>("parent_id")
            .map_err(|error| format!("failed to decode selection receipt parent: {error}"))?;
        let decision_hash = row
            .get::<String>("decision_hash")
            .map_err(|error| format!("failed to decode selection receipt hash: {error}"))?;
        let entry_json = row
            .get::<String>("entry_json")
            .map_err(|error| format!("failed to decode selection receipt entry: {error}"))?;
        let entry: SelectionDecisionEntry = serde_json::from_str(&entry_json).map_err(|error| {
            format!("selection receipt '{decision_id}' is not a typed decision entry: {error}")
        })?;
        entry.validate_shape().map_err(|error| {
            format!("selection receipt '{decision_id}' has an invalid typed shape: {error}")
        })?;
        crate::successor_selection::traversal::validate_patch_replay(&entry).map_err(|error| {
            format!("selection receipt '{decision_id}' fails deterministic replay: {error}")
        })?;
        let observed_hash = entry
            .decision_hash()
            .map_err(|error| {
                format!("selection receipt '{decision_id}' cannot be hashed: {error}")
            })?
            .as_str()
            .to_string();
        if observed_hash != decision_hash {
            return Err(format!(
                "selection receipt '{decision_id}' decision hash does not match its typed entry"
            ));
        }
        let set_id = entry
            .candidate_set
            .as_ref()
            .map(|set| set.root.as_str())
            .unwrap_or_else(|| entry.considered_order_hash.as_str());
        let expected_id = selection_decision_id(
            &campaign_id,
            &parent_id,
            set_id,
            entry.procedure_or_policy.as_str(),
            &decision_hash,
        );
        if decision_id != expected_id {
            return Err(format!(
                "selection receipt decision id '{decision_id}' does not match production-derived id '{expected_id}'"
            ));
        }
        bindings.insert(
            decision_id.clone(),
            (
                campaign_id.clone(),
                parent_id.clone(),
                set_id.to_string(),
                entry.procedure_or_policy.as_str().to_string(),
                decision_hash.clone(),
            ),
        );
        receipt_ids.insert(decision_id.clone());

        let gate = entry
            .traversal
            .as_ref()
            .map(|traversal| traversal.strategy.patch_gate())
            .unwrap_or(PatchGate::Disabled);
        if gate == PatchGate::ReviewedAdmissible {
            strict_receipts.insert(decision_id.clone());
        }
        for (index, payload) in entry.considered.iter().enumerate() {
            let Some(review) = payload.patch_review.as_ref() else {
                continue;
            };
            if gate != PatchGate::ReviewedAdmissible {
                return Err(format!(
                    "selection receipt '{decision_id}' carries review '{}' without a strict applied patch gate",
                    review.citation.ref_id
                ));
            }
            let file_review = inventory.get(&review.citation.ref_id).copied().ok_or_else(|| {
                format!(
                    "strict selection receipt '{decision_id}' review '{}' has no typed filesystem evidence",
                    review.citation.ref_id
                )
            })?;
            if file_review != review {
                return Err(format!(
                    "strict selection receipt '{decision_id}' review '{}' does not match typed filesystem evidence",
                    review.citation.ref_id
                ));
            }
            let membership = entry
                .candidate_set_membership_for_payload(index, payload)
                .map_err(|error| {
                    format!(
                        "selection receipt '{decision_id}' cannot resolve candidate membership: {error}"
                    )
                })?;
            let member_id = selection_member_id(membership, payload).map_err(|error| {
                format!(
                    "selection receipt '{decision_id}' cannot derive candidate member id: {error}"
                )
            })?;
            let key = (decision_id.clone(), member_id);
            if occurrences.insert(key.clone(), review.clone()).is_some() {
                return Err(format!(
                    "selection receipt review occurrence is duplicated: decision_id={}, member_id={}",
                    key.0, key.1
                ));
            }
        }
    }

    let decision_rows = db
        .raw_query_params(
            r#"
?[decision_id, campaign_id, parent_id, set_id, procedure_id, decision_hash] :=
    *eval_selection_decision {
        decision_id,
        campaign_id,
        parent_id,
        set_id,
        procedure_id,
        decision_hash,
    }
"#,
            BTreeMap::new(),
        )
        .map_err(|error| format!("failed to read normalized selection decisions: {error}"))?;
    let mut decisions = BTreeSet::new();
    for row in decision_rows.row_refs() {
        let decision_id = row
            .get::<String>("decision_id")
            .map_err(|error| format!("failed to decode selection decision id: {error}"))?;
        let Some(binding) = bindings.get(&decision_id) else {
            return Err(format!(
                "normalized selection decision has no typed receipt: decision_id={decision_id}"
            ));
        };
        let observed = (
            row.get::<String>("campaign_id")
                .map_err(|error| format!("failed to decode selection campaign: {error}"))?,
            row.get::<String>("parent_id")
                .map_err(|error| format!("failed to decode selection parent: {error}"))?,
            row.get::<String>("set_id")
                .map_err(|error| format!("failed to decode selection set: {error}"))?,
            row.get::<String>("procedure_id")
                .map_err(|error| format!("failed to decode selection procedure: {error}"))?,
            row.get::<Option<String>>("decision_hash")
                .map_err(|error| format!("failed to decode selection decision hash: {error}"))?,
        );
        if observed.0 != binding.0
            || observed.1 != binding.1
            || observed.2 != binding.2
            || observed.3 != binding.3
            || observed.4.as_deref() != Some(binding.4.as_str())
        {
            return Err(format!(
                "normalized selection decision does not match its typed receipt: decision_id={decision_id}"
            ));
        }
        decisions.insert(decision_id);
    }
    for decision_id in bindings.keys() {
        if !decisions.contains(decision_id) {
            return Err(format!(
                "typed selection receipt has no matching normalized decision row: decision_id={decision_id}"
            ));
        }
    }

    if reviews.is_empty() && !strict_receipts.is_empty() {
        return Err(format!(
            "strict selection receipt authority exists without typed filesystem review evidence: {}",
            strict_receipts.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    let gate_rows = db
        .raw_query_params(
            r#"
?[decision_id, gate] :=
    *eval_selection_patch_gate {
        decision_id,
        gate,
    }
"#,
            BTreeMap::new(),
        )
        .map_err(|error| format!("failed to read applied selection patch gates: {error}"))?;
    let mut gates = BTreeMap::new();
    for row in gate_rows.row_refs() {
        let decision_id = row
            .get::<String>("decision_id")
            .map_err(|error| format!("failed to decode applied patch-gate decision: {error}"))?;
        let gate = row
            .get::<String>("gate")
            .map_err(|error| format!("failed to decode applied patch gate: {error}"))?;
        if !receipt_ids.contains(&decision_id) {
            return Err(format!(
                "applied patch gate has no typed selection receipt: decision_id={decision_id}"
            ));
        }
        gates.insert(decision_id, gate);
    }
    for decision_id in &strict_receipts {
        if gates.get(decision_id).map(String::as_str)
            != Some(PatchGate::ReviewedAdmissible.as_str())
        {
            return Err(format!(
                "strict selection receipt has no matching applied patch gate: decision_id={decision_id}"
            ));
        }
    }

    let candidate_rows = db
        .raw_query_params(
            r#"
?[decision_id, member_id, node_id, branch_id] :=
    *eval_selection_candidate {
        decision_id,
        member_id,
        node_id,
        branch_id,
    }
"#,
            BTreeMap::new(),
        )
        .map_err(|error| format!("failed to read normalized selection candidates: {error}"))?;
    let mut candidates = BTreeMap::new();
    for row in candidate_rows.row_refs() {
        let decision_id = row
            .get::<String>("decision_id")
            .map_err(|error| format!("failed to decode selection candidate decision: {error}"))?;
        let member_id = row
            .get::<String>("member_id")
            .map_err(|error| format!("failed to decode selection candidate member: {error}"))?;
        let node_id = row
            .get::<String>("node_id")
            .map_err(|error| format!("failed to decode selection candidate node: {error}"))?;
        let branch_id = row
            .get::<String>("branch_id")
            .map_err(|error| format!("failed to decode selection candidate branch: {error}"))?;
        candidates.insert((decision_id, member_id), (node_id, branch_id));
    }
    for (key, review) in &occurrences {
        let candidate = candidates.get(key).ok_or_else(|| {
            format!(
                "typed receipt review has no matching selection candidate row: decision_id={}, member_id={}",
                key.0, key.1
            )
        })?;
        if candidate.0 != review.candidate.node_id || candidate.1 != review.candidate.branch_id {
            return Err(format!(
                "selection candidate row does not match typed receipt review: decision_id={}, member_id={}",
                key.0, key.1
            ));
        }
    }

    let review_rows = db
        .raw_query_params(
            r#"
?[decision_id, member_id, schema_version, procedure_id, node_id, branch_id, generation, artifact_id, artifact_surface_hash, evaluation_hash, config_hash, change_set_hash, verdict, confidence, blocking_findings, missing_evidence, rationale, citation_ref, citation_hash, record_name] :=
    *eval_selection_patch_review {
        decision_id,
        member_id,
        schema_version,
        procedure_id,
        node_id,
        branch_id,
        generation,
        artifact_id,
        artifact_surface_hash,
        evaluation_hash,
        config_hash,
        change_set_hash,
        verdict,
        confidence,
        blocking_findings,
        missing_evidence,
        rationale,
        citation_ref,
        citation_hash,
        record_name,
    }
"#,
            BTreeMap::new(),
        )
        .map_err(|error| format!("failed to read normalized candidate reviews: {error}"))?;
    let review_count = review_rows.rows.len();
    let mut normalized = BTreeSet::new();
    let mut linked = BTreeSet::new();

    macro_rules! require_review_field {
        ($row:ident, $ty:ty, $field:literal, $expected:expr, $citation:expr) => {{
            let actual = $row.get::<$ty>($field).map_err(|error| {
                format!(
                    "failed to decode candidate review '{}' field '{}': {error}",
                    $citation, $field
                )
            })?;
            let expected = $expected;
            if actual != expected {
                return Err(format!(
                    "normalized candidate review '{}' field '{}' does not match typed file evidence",
                    $citation, $field
                ));
            }
        }};
    }

    for row in review_rows.row_refs() {
        let decision_id = row
            .get::<String>("decision_id")
            .map_err(|error| format!("failed to decode candidate review decision: {error}"))?;
        let member_id = row
            .get::<String>("member_id")
            .map_err(|error| format!("failed to decode candidate review member: {error}"))?;
        let citation = row
            .get::<String>("citation_ref")
            .map_err(|error| format!("failed to decode candidate review citation: {error}"))?;
        let key = (decision_id, member_id);
        let review = occurrences.get(&key).ok_or_else(|| {
            format!(
                "normalized candidate review is not authorized by typed receipt membership: decision_id={}, member_id={}",
                key.0, key.1
            )
        })?;
        if citation != review.citation.ref_id {
            return Err(format!(
                "normalized candidate review citation does not match typed receipt membership: decision_id={}, member_id={}",
                key.0, key.1
            ));
        }

        require_review_field!(
            row,
            i64,
            "schema_version",
            i64::from(review.schema_version),
            citation
        );
        require_review_field!(
            row,
            String,
            "procedure_id",
            review.procedure_id.clone(),
            citation
        );
        require_review_field!(
            row,
            String,
            "node_id",
            review.candidate.node_id.clone(),
            citation
        );
        require_review_field!(
            row,
            String,
            "branch_id",
            review.candidate.branch_id.clone(),
            citation
        );
        require_review_field!(
            row,
            i64,
            "generation",
            i64::from(review.candidate.generation),
            citation
        );
        require_review_field!(
            row,
            String,
            "artifact_id",
            review.artifact_id.to_string(),
            citation
        );
        require_review_field!(
            row,
            String,
            "artifact_surface_hash",
            review.artifact_surface_hash.as_str().to_string(),
            citation
        );
        require_review_field!(
            row,
            String,
            "evaluation_hash",
            review.evaluation_hash.as_str().to_string(),
            citation
        );
        require_review_field!(
            row,
            String,
            "config_hash",
            review.config_hash.as_str().to_string(),
            citation
        );
        require_review_field!(
            row,
            String,
            "change_set_hash",
            review.change_set_hash.as_str().to_string(),
            citation
        );
        require_review_field!(
            row,
            String,
            "verdict",
            patch_verdict_label(review.verdict).to_string(),
            citation
        );
        require_review_field!(
            row,
            String,
            "confidence",
            confidence_label(review.confidence).to_string(),
            citation
        );
        require_review_field!(
            row,
            Vec<String>,
            "blocking_findings",
            review.blocking_findings.clone(),
            citation
        );
        require_review_field!(
            row,
            Vec<String>,
            "missing_evidence",
            review.missing_evidence.clone(),
            citation
        );
        require_review_field!(
            row,
            Vec<String>,
            "rationale",
            review.rationale.clone(),
            citation
        );
        require_review_field!(
            row,
            Option<String>,
            "citation_hash",
            review
                .citation
                .content_hash
                .as_ref()
                .map(|hash| hash.as_str().to_string()),
            citation
        );
        require_review_field!(
            row,
            Option<String>,
            "record_name",
            review.citation.record_name.clone(),
            citation
        );

        if !normalized.insert(key.clone()) {
            return Err(format!(
                "normalized candidate review occurrence is duplicated: decision_id={}, member_id={}",
                key.0, key.1
            ));
        }
        linked.insert(citation);
    }
    for key in occurrences.keys() {
        if !normalized.contains(key) {
            return Err(format!(
                "typed receipt review has no normalized candidate review row: decision_id={}, member_id={}",
                key.0, key.1
            ));
        }
    }

    let change_rows = db
        .raw_query_params(
            r#"
?[decision_id, member_id, change_index, relpath, source_content_hash, proposed_content_hash] :=
    *eval_selection_patch_change {
        decision_id,
        member_id,
        change_index,
        relpath,
        source_content_hash,
        proposed_content_hash,
    }
"#,
            BTreeMap::new(),
        )
        .map_err(|error| format!("failed to read normalized candidate review changes: {error}"))?;
    let stored_changes = change_rows.rows.len();
    let mut changes: BTreeMap<(String, String), Vec<(i64, PatchChange)>> = BTreeMap::new();
    for row in change_rows.row_refs() {
        let decision_id = row
            .get::<String>("decision_id")
            .map_err(|error| format!("failed to decode patch change decision: {error}"))?;
        let member_id = row
            .get::<String>("member_id")
            .map_err(|error| format!("failed to decode patch change member: {error}"))?;
        let key = (decision_id, member_id);
        if !normalized.contains(&key) {
            return Err(format!(
                "normalized patch change has no candidate review row: decision_id={}, member_id={}",
                key.0, key.1
            ));
        }
        let index = row
            .get::<i64>("change_index")
            .map_err(|error| format!("failed to decode patch change index: {error}"))?;
        let change = PatchChange {
            relpath: PathBuf::from(
                row.get::<String>("relpath")
                    .map_err(|error| format!("failed to decode patch change path: {error}"))?,
            ),
            source_content_hash: row
                .get::<Option<String>>("source_content_hash")
                .map_err(|error| format!("failed to decode patch source hash: {error}"))?,
            proposed_content_hash: row
                .get::<Option<String>>("proposed_content_hash")
                .map_err(|error| format!("failed to decode patch proposed hash: {error}"))?,
        };
        changes.entry(key).or_default().push((index, change));
    }

    for (key, review) in &occurrences {
        let mut actual = changes.remove(key).unwrap_or_default();
        actual.sort_by_key(|(index, _)| *index);
        let expected_changes = review
            .changes
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, change)| {
                i64::try_from(index)
                    .map(|index| (index, change))
                    .map_err(|error| {
                        format!(
                            "candidate review '{}' change index exceeds DB range: {error}",
                            review.citation.ref_id
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if actual != expected_changes {
            return Err(format!(
                "normalized patch changes do not match typed review '{}': decision_id={}, member_id={}",
                review.citation.ref_id, key.0, key.1
            ));
        }
    }
    if let Some((key, _)) = changes.first_key_value() {
        return Err(format!(
            "normalized patch changes remain without a candidate review occurrence: decision_id={}, member_id={}",
            key.0, key.1
        ));
    }

    let pending_refs = inventory
        .keys()
        .filter(|citation| !linked.contains(*citation))
        .cloned()
        .collect();
    Ok(PatchMirrorCoverage {
        review_count: reviews.len(),
        change_count,
        review_rows: review_count,
        change_rows: stored_changes,
        pending_refs,
    })
}

fn patch_verdict_label(verdict: PatchVerdict) -> &'static str {
    match verdict {
        PatchVerdict::Admissible => "admissible",
        PatchVerdict::Rejected => "rejected",
        PatchVerdict::Inconclusive => "inconclusive",
    }
}

fn confidence_label(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Low => "low",
        Confidence::Medium => "medium",
        Confidence::High => "high",
    }
}

fn relation_count_missing() -> Vec<RelationCount> {
    EVAL_RELS
        .iter()
        .map(|(relation, _)| RelationCount {
            relation: (*relation).to_string(),
            exists: false,
            count: None,
            detail: None,
        })
        .collect()
}

fn r0_to_r1_expectations() -> TransitionAudit {
    TransitionAudit {
        transition: "r0_to_r1".to_string(),
        from: WalkPhase::R0,
        to: WalkPhase::R1,
        code_symbol: "crates/ploke-eval/src/cli/prototype1_state/live_edges.rs::r0_to_r1"
            .to_string(),
        expected_file_writes: vec![
            ExpectedPersistence {
                surface: "prototype1-monitor-target.json".to_string(),
                expectation: DocumentExpectation::WrittenByTransition,
                status: ExpectedStatus::Expected,
                detail: "record_active_prototype1_monitor_target writes the active campaign/repo pointer"
                    .to_string(),
            },
            ExpectedPersistence {
                surface: "closure-state.json".to_string(),
                expectation: DocumentExpectation::DerivedIfMissing,
                status: ExpectedStatus::Conditional,
                detail: "ensure_prototype1_baseline_closure_state may recompute this cache when absent"
                    .to_string(),
            },
            ExpectedPersistence {
                surface: "transition-journal.jsonl".to_string(),
                expectation: DocumentExpectation::PathOnly,
                status: ExpectedStatus::NotExpected,
                detail: "r0_to_r1 creates the journal handle/path but does not append an entry"
                    .to_string(),
            },
        ],
        expected_db_writes: vec![
            ExpectedPersistence {
                surface: "eval_campaign".to_string(),
                expectation: DocumentExpectation::WrittenByTransition,
                status: ExpectedStatus::Conditional,
                detail: "written when the admitted profile selects database or dual-strict eval storage"
                    .to_string(),
            },
            ExpectedPersistence {
                surface: "eval_profile_commitment".to_string(),
                expectation: DocumentExpectation::WrittenByTransition,
                status: ExpectedStatus::Conditional,
                detail: "written when an admitted run profile exists and DB-backed eval storage is enabled"
                    .to_string(),
            },
            ExpectedPersistence {
                surface: "eval_closure_ref".to_string(),
                expectation: DocumentExpectation::WrittenByTransition,
                status: ExpectedStatus::Conditional,
                detail: "written when DB-backed eval storage is enabled after closure-state validation"
                    .to_string(),
            },
        ],
    }
}

fn summarize(
    docs: &[DocumentAudit],
    database: &DatabaseAudit,
    transition: &TransitionAudit,
    transitions: &[TransitionChecklist],
) -> AuditSummary {
    let required_documents = docs
        .iter()
        .filter(|doc| doc.expectation == DocumentExpectation::Required)
        .count();
    let required_ok = docs
        .iter()
        .filter(|doc| doc.expectation == DocumentExpectation::Required)
        .filter(|doc| doc.status == DocumentStatus::Ok)
        .count();
    let missing_required = docs
        .iter()
        .filter(|doc| doc.expectation == DocumentExpectation::Required)
        .filter(|doc| doc.status != DocumentStatus::Ok)
        .count();
    let parse_errors = docs
        .iter()
        .filter(|doc| doc.status == DocumentStatus::ParseError)
        .count();
    let db_rows_total = database
        .relation_counts
        .iter()
        .filter_map(|row| row.count)
        .sum();
    let expected_transition_db_rows = transition
        .expected_db_writes
        .iter()
        .filter(|item| item.status == ExpectedStatus::Expected)
        .count() as i64;
    let observed_transition_db_rows = 0;
    let checklist_needs_review = transitions.iter().any(|transition| {
        matches!(
            transition.overall_status,
            PersistenceStatus::Partial | PersistenceStatus::None | PersistenceStatus::Error
        )
    });
    let verdict = if missing_required > 0 {
        AuditVerdict::MissingPreconditions
    } else if parse_errors > 0
        || database.status == DatabaseStatus::OpenError
        || checklist_needs_review
    {
        AuditVerdict::NeedsReview
    } else {
        AuditVerdict::Ready
    };

    AuditSummary {
        required_documents,
        required_ok,
        missing_required,
        parse_errors,
        db_rows_total,
        expected_transition_db_rows,
        observed_transition_db_rows,
        verdict,
    }
}

fn parse_jsonl(text: &str) -> Result<(), String> {
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        serde_json::from_str::<serde_json::Value>(trimmed)
            .map_err(|source| format!("line {}: {source}", index + 1))?;
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn scope_label(scope: Prototype1StateWalkAuditScope) -> &'static str {
    match scope {
        Prototype1StateWalkAuditScope::R0ToR1 => "r0-to-r1",
    }
}

fn audit_transition_label(transition: Prototype1StateWalkAuditTransition) -> &'static str {
    match transition {
        Prototype1StateWalkAuditTransition::R0ToR1 => "r0->r1",
        Prototype1StateWalkAuditTransition::R1ToR2a => "r1->r2a",
        Prototype1StateWalkAuditTransition::R1ToR3 => "r1->r3",
        Prototype1StateWalkAuditTransition::R2aToR3 => "r2a->r3",
        Prototype1StateWalkAuditTransition::R3ToR4a => "r3->r4a",
        Prototype1StateWalkAuditTransition::R4aToR4b => "r4a->r4b",
        Prototype1StateWalkAuditTransition::R4aToR4c => "r4a->r4c",
        Prototype1StateWalkAuditTransition::R4bToR4c => "r4b->r4c",
        Prototype1StateWalkAuditTransition::R4cToR5 => "r4c->r5",
        Prototype1StateWalkAuditTransition::R5ToR6 => "r5->r6",
        Prototype1StateWalkAuditTransition::R6ToR7 => "r6->r7",
        Prototype1StateWalkAuditTransition::R7ToR8 => "r7->r8",
        Prototype1StateWalkAuditTransition::R8ToR9 => "r8->r9",
        Prototype1StateWalkAuditTransition::R9ToR10 => "r9->r10",
        Prototype1StateWalkAuditTransition::R10ToR11a => "r10->r11a",
        Prototype1StateWalkAuditTransition::R10ToR11 => "r10->r11",
        Prototype1StateWalkAuditTransition::R11aToR12 => "r11a->r12",
        Prototype1StateWalkAuditTransition::R11ToR12 => "r11->r12",
        Prototype1StateWalkAuditTransition::R12ToR13a => "r12->r13a",
        Prototype1StateWalkAuditTransition::R12ToR13b => "r12->r13b",
        Prototype1StateWalkAuditTransition::R12ToR13c => "r12->r13c",
        Prototype1StateWalkAuditTransition::R13aToR14a => "r13a->r14a",
        Prototype1StateWalkAuditTransition::R13bToR14b => "r13b->r14b",
    }
}

fn status_label(status: DocumentStatus) -> &'static str {
    match status {
        DocumentStatus::Ok => "ok",
        DocumentStatus::Missing => "missing",
        DocumentStatus::ReadError => "read_error",
        DocumentStatus::ParseError => "parse_error",
        DocumentStatus::NotChecked => "not_checked",
    }
}

fn persistence_label(status: PersistenceStatus) -> &'static str {
    match status {
        PersistenceStatus::Ok => "✓ ok",
        PersistenceStatus::Partial => "~ partial",
        PersistenceStatus::None => "× none",
        PersistenceStatus::NotApplicable => "- n/a",
        PersistenceStatus::Error => "! error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_row_count_cannot_cover_later_unlinked_review() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (_, _, first) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let pending = test_patch_review("branch-b", 1);
        let db = load_owner_eval_database(&path).expect("load owner db");

        let coverage =
            verify_review_mirror(&db, &[first, pending.clone()]).expect("valid partial mirror");

        assert_eq!(coverage.review_count, 2);
        assert_eq!(coverage.change_count, 2);
        assert_eq!(coverage.review_rows, 1);
        assert_eq!(coverage.change_rows, 1);
        assert_eq!(coverage.pending_refs, vec![pending.citation.ref_id.clone()]);
    }

    #[test]
    fn review_mirror_rejects_orphan_change_rows() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (_, _, review) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        insert_patch_change(
            &db,
            "decision-orphan",
            "member-orphan",
            0,
            &review.changes[0],
        );

        let error =
            verify_review_mirror(&db, &[review]).expect_err("orphan change must fail closed");

        assert!(error.contains("has no candidate review row"));
    }

    #[test]
    fn review_mirror_rejects_normalized_field_mismatch() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (_, _, stored) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        let mut file = stored;
        file.evaluation_hash =
            crate::cli::prototype1_state::history::HistoryHash::of_bytes(b"changed-evaluation");

        let error =
            verify_review_mirror(&db, &[file]).expect_err("field mismatch must fail closed");

        assert!(error.contains("does not match typed filesystem evidence"));
    }

    #[test]
    fn review_mirror_accepts_repeated_exact_occurrences() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let entry = strict_test_entry();
        let (_, _, review) = write_test_receipt(&path, "campaign-a", "parent-a", entry.clone())
            .expect("write first receipt");
        write_test_receipt(&path, "campaign-b", "parent-b", entry).expect("write second receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");

        let coverage =
            verify_review_mirror(&db, &[review]).expect("repeated exact evidence is valid");

        assert_eq!(coverage.review_rows, 2);
        assert_eq!(coverage.change_rows, 2);
        assert!(coverage.pending_refs.is_empty());
    }

    #[test]
    fn review_mirror_rejects_normalized_change_hash_mismatch() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (decision_id, member_id, review) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        let mut changed = review.changes[0].clone();
        changed.proposed_content_hash = Some("different-proposed-hash".to_string());
        insert_patch_change(&db, &decision_id, &member_id, 0, &changed);

        let error =
            verify_review_mirror(&db, &[review]).expect_err("changed hash must fail closed");

        assert!(error.contains("normalized patch changes do not match"));
    }

    #[test]
    fn review_mirror_rejects_arbitrary_decision_key() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (_, member_id, review) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        insert_patch_review(&db, "decision-arbitrary", &member_id, &review);

        let error =
            verify_review_mirror(&db, &[review]).expect_err("arbitrary decision key must fail");
        assert!(error.contains("not authorized by typed receipt membership"));
    }

    #[test]
    fn review_mirror_rejects_receipt_membership_mismatch() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (decision_id, _, review) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        insert_patch_review(&db, &decision_id, "member-arbitrary", &review);

        let error =
            verify_review_mirror(&db, &[review]).expect_err("wrong receipt member must fail");
        assert!(error.contains("not authorized by typed receipt membership"));
    }

    #[test]
    fn review_mirror_rejects_missing_candidate_occurrence() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (decision_id, member_id, review) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        remove_candidate(&db, &decision_id, &member_id);

        let error =
            verify_review_mirror(&db, &[review]).expect_err("missing candidate row must fail");
        assert!(error.contains("has no matching selection candidate row"));
    }

    #[test]
    fn review_mirror_rejects_fabricated_receipt_decision_id() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (decision_id, _, review) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        rekey_receipt(&db, &decision_id, "decision-fabricated");

        let error =
            verify_review_mirror(&db, &[review]).expect_err("fabricated receipt id must fail");
        assert!(error.contains("does not match production-derived id"));
    }

    #[test]
    fn review_mirror_rejects_missing_decision_row() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let (decision_id, _, review) =
            write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
                .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        remove_decision(&db, &decision_id);

        let error =
            verify_review_mirror(&db, &[review]).expect_err("missing decision row must fail");
        assert!(error.contains("has no matching normalized decision row"));
    }

    #[test]
    fn strict_db_authority_requires_nonempty_file_inventory() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
            .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");

        let error =
            verify_review_mirror(&db, &[]).expect_err("strict receipt requires file evidence");
        assert!(error.contains("has no typed filesystem evidence"));

        let empty = PersistenceSide::not_applicable("empty inventory");
        let mut mirror = ReviewMirrorAudit {
            review_file: empty.clone(),
            review_db: empty.clone(),
            change_file: empty.clone(),
            change_db: empty,
        };
        let counts = load_relation_counts(&db).expect("relation counts");
        update_db_mirror(&mut mirror, &Ok(Vec::new()), &db, &path, &counts);
        assert_eq!(mirror.review_file.status, PersistenceStatus::Error);
        assert_eq!(mirror.change_file.status, PersistenceStatus::Error);
        assert_eq!(mirror.review_db.status, PersistenceStatus::Error);
        assert_eq!(mirror.change_db.status, PersistenceStatus::Error);
    }

    #[test]
    fn strict_receipt_only_fails_closed() {
        let tmp = tempfile::tempdir().expect("tmp");
        let path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        write_test_receipt(&path, "campaign-a", "parent-a", strict_test_entry())
            .expect("write receipt");
        let db = load_owner_eval_database(&path).expect("load owner db");
        clear_patch_rows(&db);

        let empty = PersistenceSide::not_applicable("empty inventory");
        let mut mirror = ReviewMirrorAudit {
            review_file: empty.clone(),
            review_db: empty.clone(),
            change_file: empty.clone(),
            change_db: empty,
        };
        let counts = load_relation_counts(&db).expect("relation counts");
        update_db_mirror(&mut mirror, &Ok(Vec::new()), &db, &path, &counts);

        assert_eq!(mirror.review_file.status, PersistenceStatus::Error);
        assert_eq!(mirror.change_file.status, PersistenceStatus::Error);
        assert_eq!(mirror.review_db.status, PersistenceStatus::Error);
        assert_eq!(mirror.change_db.status, PersistenceStatus::Error);
    }

    fn strict_test_entry() -> SelectionDecisionEntry {
        use crate::cli::prototype1_state::{
            history::{
                HistoryCandidates, HistoryHash, ProcedureRef, SealedEvidenceCitation,
                TraversalCandidateSource, TraversalEvidence,
            },
            parent::ChildPlanFiles,
        };
        use crate::successor_selection::traversal as traversal_selection;

        let source: SelectionDecisionEntry = serde_json::from_str(include_str!(
            "../../../tests/fixtures/prototype1-v25-selection-safety-20260717/selection-decision-entry.json"
        ))
        .expect("decode historical selection receipt");
        let plan: ChildPlanFiles = serde_json::from_str(include_str!(
            "../../../tests/fixtures/prototype1-v25-selection-safety-20260717/child-plan-node-461dba1909fb6cf7.json"
        ))
        .expect("decode historical child plan");
        let mut payload = source.considered[0].clone();
        let candidate = payload
            .selection_input
            .as_ref()
            .expect("selection input")
            .candidate
            .clone();
        let harness = plan
            .children()
            .iter()
            .find(|child| child.node_id() == candidate.node_id)
            .and_then(|child| child.harness_evidence())
            .expect("typed harness evidence")
            .clone();
        {
            let artifact = payload.artifact.as_mut().expect("candidate artifact");
            assert_eq!(
                artifact.artifact_surface.as_ref(),
                Some(harness.artifact_surface())
            );
            artifact.harness = Some(harness);
            artifact.schema_version = artifact.schema_version.max(4);
        }
        let artifact = payload.artifact.as_ref().expect("candidate artifact");
        let harness = artifact.harness.as_ref().expect("attached harness");
        assert_eq!(
            harness.changed_paths(),
            [artifact.resolved.target_relpath.clone()]
        );
        let artifact_id = artifact
            .resolved
            .branch
            .derived_artifact_id
            .as_ref()
            .expect("derived artifact")
            .clone();
        let surface_hash = HistoryHash::of_domain_json(
            "prototype1.history.artifact_surface.v1",
            artifact
                .artifact_surface
                .as_ref()
                .expect("artifact surface"),
        )
        .expect("surface hash");
        let evaluation = payload
            .sealed_evidence
            .as_ref()
            .expect("sealed evidence")
            .evaluations
            .iter()
            .find(|evaluation| evaluation.branch_id == candidate.branch_id)
            .expect("matching evaluation");
        let evaluation_hash = evaluation
            .evaluation_artifact_citation
            .as_ref()
            .and_then(|citation| citation.content_hash.as_ref())
            .expect("evaluation hash")
            .clone();
        assert_eq!(
            evaluation.primary_report_citation.content_hash.as_ref(),
            Some(&evaluation_hash)
        );
        let changes = vec![PatchChange {
            relpath: artifact.resolved.target_relpath.clone(),
            source_content_hash: Some(artifact.resolved.source_content_hash.clone()),
            proposed_content_hash: Some(artifact.resolved.branch.proposed_content_hash.clone()),
        }];
        let change_set_hash = HistoryHash::of_domain_json(
            "prototype1.history.candidate_patch_change_set.v1",
            &changes,
        )
        .expect("change set hash");
        let citation_hash =
            HistoryHash::of_domain_json("prototype1.test.walk_audit_patch_review.v1", &candidate)
                .expect("review citation hash");
        payload.patch_review = Some(PatchReview {
            schema_version: 2,
            procedure_id: crate::successor_selection::PATCH_REVIEW_PROCEDURE_ID.to_string(),
            candidate: candidate.clone(),
            artifact_id,
            artifact_surface_hash: surface_hash,
            evaluation_hash,
            config_hash: HistoryHash::of_bytes(b"walk-audit-review-config"),
            change_set_hash,
            changes,
            verdict: PatchVerdict::Admissible,
            confidence: Confidence::High,
            blocking_findings: Vec::new(),
            missing_evidence: Vec::new(),
            rationale: vec!["typed audit receipt test evidence".to_string()],
            citation: SealedEvidenceCitation {
                ref_id: crate::successor_selection::candidate_review_ref(&candidate.branch_id),
                content_hash: Some(citation_hash),
                record_name: Some(crate::successor_selection::PATCH_REVIEW_RECORD_NAME.to_string()),
            },
        });
        payload.schema_version = payload.schema_version.max(5);

        let traversal = source.traversal.clone().expect("traversal evidence");
        let strategy = traversal
            .strategy
            .with_patch_gate(PatchGate::ReviewedAdmissible);
        let candidates = traversal_selection::Candidates::from_history(HistoryCandidates {
            scope: source.scope.clone(),
            candidates: Vec::new(),
        })
        .with_current_generation(source.scope.clone(), vec![payload])
        .expect("bind current candidate");
        let attempt = traversal_selection::select_attempt_with_policy(
            candidates,
            traversal.seed,
            strategy,
            source.metrics.policy.clone(),
            &traversal.oracle_targets,
        )
        .expect("strict traversal");
        let traversal_selection::SelectionAttempt::Selected(selection) = attempt else {
            panic!("admissible review must select the only candidate")
        };
        let entry = SelectionDecisionEntry::new_with_traversal_identity_metrics(
            ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
            source.scope,
            Some(selection.selected_payload.candidate.clone()),
            selection.selected_occurrence_id(),
            selection.selected_membership_id(),
            selection.considered,
            selection.considered_sources,
            selection.projection_failures,
            Some(TraversalEvidence {
                seed: traversal.seed,
                strategy,
                oracle_targets: traversal.oracle_targets,
                selected_source: Some(TraversalCandidateSource::CurrentGeneration),
                child_counts: selection.child_counts,
            }),
            selection.metrics,
            selection.decision,
        )
        .expect("strict selection entry");
        traversal_selection::validate_patch_replay(&entry).expect("strict receipt replay");
        entry
    }

    fn write_test_receipt(
        path: &Path,
        campaign: &str,
        parent: &str,
        entry: SelectionDecisionEntry,
    ) -> Result<(String, String, PatchReview), String> {
        let payload = entry
            .considered
            .first()
            .ok_or_else(|| "strict test receipt has no payload".to_string())?;
        let review = payload
            .patch_review
            .clone()
            .ok_or_else(|| "strict test receipt has no review".to_string())?;
        let membership = entry
            .candidate_set_membership_for_payload(0, payload)
            .map_err(|error| error.to_string())?;
        let member_id =
            selection_member_id(membership, payload).map_err(|error| error.to_string())?;
        let campaign_id = CampaignId::from(campaign);
        let campaign_root = path.parent().and_then(Path::parent).ok_or_else(|| {
            format!(
                "test eval-store path has no campaign root: {}",
                path.display()
            )
        })?;
        let (seeded_path, ..) = crate::cli::prototype1_state::eval_store::seeded_patch_context(
            campaign_root,
            &campaign_id,
            PatchGate::ReviewedAdmissible,
            Some(&review.config_hash),
        );
        if seeded_path != path {
            return Err(format!(
                "seeded test eval-store path '{}' differs from receipt path '{}'",
                seeded_path.display(),
                path.display()
            ));
        }
        let receipt =
            crate::cli::prototype1_state::eval_store::write_selection_decision_to_owner_db(
                path,
                crate::cli::prototype1_state::eval_store::SelectionDecisionEvidence {
                    campaign_id,
                    parent_id: parent.to_string(),
                    entry,
                    decision_ref: None,
                    recorded_at: None,
                },
            )
            .map_err(|error| error.to_string())?;
        Ok((receipt.decision_id, member_id, review))
    }

    fn remove_candidate(db: &Database, decision_id: &str, member_id: &str) {
        let params = BTreeMap::from([
            ("decision_id".to_string(), decision_id.to_string().into()),
            ("member_id".to_string(), member_id.to_string().into()),
        ]);
        db.raw_query_mut_params(
            "?[decision_id, member_id] <- [[$decision_id, $member_id]] :rm eval_selection_candidate { decision_id, member_id }",
            params,
        )
        .expect("remove candidate");
    }

    fn remove_decision(db: &Database, decision_id: &str) {
        let params = BTreeMap::from([("decision_id".to_string(), decision_id.to_string().into())]);
        db.raw_query_mut_params(
            "?[decision_id] <- [[$decision_id]] :rm eval_selection_decision { decision_id }",
            params,
        )
        .expect("remove decision");
    }

    fn clear_patch_rows(db: &Database) {
        for script in [
            "?[decision_id, member_id, change_index] := *eval_selection_patch_change { decision_id, member_id, change_index } :rm eval_selection_patch_change { decision_id, member_id, change_index }",
            "?[decision_id, member_id] := *eval_selection_patch_review { decision_id, member_id } :rm eval_selection_patch_review { decision_id, member_id }",
            "?[decision_id] := *eval_selection_patch_gate { decision_id } :rm eval_selection_patch_gate { decision_id }",
        ] {
            db.raw_query_mut_params(script, BTreeMap::new())
                .expect("clear normalized patch rows");
        }
    }

    fn rekey_receipt(db: &Database, decision_id: &str, replacement: &str) {
        let params = BTreeMap::from([("decision_id".to_string(), decision_id.to_string().into())]);
        let result = db
            .raw_query_params(
                "?[campaign_id, parent_id, decision_hash, entry_json] := *eval_selection_receipt { decision_id: $decision_id, campaign_id, parent_id, decision_hash, entry_json }",
                params.clone(),
            )
            .expect("query receipt");
        let row = result.row_refs().next().expect("receipt row");
        let campaign_id = row.get::<String>("campaign_id").expect("campaign id");
        let parent_id = row.get::<String>("parent_id").expect("parent id");
        let decision_hash = row.get::<String>("decision_hash").expect("decision hash");
        let entry_json = row.get::<String>("entry_json").expect("entry json");
        db.raw_query_mut_params(
            "?[decision_id] <- [[$decision_id]] :rm eval_selection_receipt { decision_id }",
            params,
        )
        .expect("remove receipt");
        let params = BTreeMap::from([
            ("decision_id".to_string(), replacement.to_string().into()),
            ("campaign_id".to_string(), campaign_id.into()),
            ("parent_id".to_string(), parent_id.into()),
            ("decision_hash".to_string(), decision_hash.into()),
            ("entry_json".to_string(), entry_json.into()),
        ]);
        db.raw_query_mut_params(
            "?[decision_id, campaign_id, parent_id, decision_hash, entry_json] <- [[$decision_id, $campaign_id, $parent_id, $decision_hash, $entry_json]] :put eval_selection_receipt { decision_id => campaign_id, parent_id, decision_hash, entry_json }",
            params,
        )
        .expect("insert fabricated receipt");
    }

    fn test_patch_review(branch: &str, change_count: usize) -> PatchReview {
        let changes = (0..change_count)
            .map(|index| PatchChange {
                relpath: PathBuf::from(format!("src/{branch}-{index}.rs")),
                source_content_hash: Some(format!("source-{branch}-{index}")),
                proposed_content_hash: Some(format!("proposed-{branch}-{index}")),
            })
            .collect::<Vec<_>>();
        PatchReview {
            schema_version: 2,
            procedure_id: crate::successor_selection::PATCH_REVIEW_PROCEDURE_ID.to_string(),
            candidate: crate::successor_selection::CandidateRef {
                node_id: format!("node-{branch}"),
                branch_id: branch.to_string(),
                generation: 2,
            },
            artifact_id: crate::loop_graph::ArtifactId::new(format!("artifact:{branch}")),
            artifact_surface_hash: crate::cli::prototype1_state::history::HistoryHash::of_bytes(
                format!("surface-{branch}").as_bytes(),
            ),
            evaluation_hash: crate::cli::prototype1_state::history::HistoryHash::of_bytes(
                format!("evaluation-{branch}").as_bytes(),
            ),
            config_hash: crate::cli::prototype1_state::history::HistoryHash::of_bytes(
                b"review-config",
            ),
            change_set_hash: crate::cli::prototype1_state::history::HistoryHash::of_domain_json(
                "prototype1.history.candidate_patch_change_set.v1",
                &changes,
            )
            .expect("change set hash"),
            changes,
            verdict: PatchVerdict::Admissible,
            confidence: Confidence::High,
            blocking_findings: Vec::new(),
            missing_evidence: Vec::new(),
            rationale: vec!["exact patch is admissible".to_string()],
            citation: crate::cli::prototype1_state::history::SealedEvidenceCitation {
                ref_id: crate::successor_selection::candidate_review_ref(branch),
                content_hash: Some(
                    crate::cli::prototype1_state::history::HistoryHash::of_bytes(
                        format!("review-{branch}").as_bytes(),
                    ),
                ),
                record_name: Some(crate::successor_selection::PATCH_REVIEW_RECORD_NAME.to_string()),
            },
        }
    }

    fn insert_patch_review(
        db: &Database,
        decision_id: &str,
        member_id: &str,
        review: &PatchReview,
    ) {
        let mut params = BTreeMap::new();
        params.insert("decision_id".to_string(), decision_id.to_string().into());
        params.insert("member_id".to_string(), member_id.to_string().into());
        params.insert(
            "schema_version".to_string(),
            i64::from(review.schema_version).into(),
        );
        params.insert(
            "procedure_id".to_string(),
            review.procedure_id.clone().into(),
        );
        params.insert(
            "node_id".to_string(),
            review.candidate.node_id.clone().into(),
        );
        params.insert(
            "branch_id".to_string(),
            review.candidate.branch_id.clone().into(),
        );
        params.insert(
            "generation".to_string(),
            i64::from(review.candidate.generation).into(),
        );
        params.insert(
            "artifact_id".to_string(),
            review.artifact_id.to_string().into(),
        );
        params.insert(
            "artifact_surface_hash".to_string(),
            review.artifact_surface_hash.as_str().to_string().into(),
        );
        params.insert(
            "evaluation_hash".to_string(),
            review.evaluation_hash.as_str().to_string().into(),
        );
        params.insert(
            "config_hash".to_string(),
            review.config_hash.as_str().to_string().into(),
        );
        params.insert(
            "change_set_hash".to_string(),
            review.change_set_hash.as_str().to_string().into(),
        );
        params.insert(
            "verdict".to_string(),
            patch_verdict_label(review.verdict).to_string().into(),
        );
        params.insert(
            "confidence".to_string(),
            confidence_label(review.confidence).to_string().into(),
        );
        params.insert(
            "blocking_findings".to_string(),
            DataValue::List(
                review
                    .blocking_findings
                    .iter()
                    .cloned()
                    .map(DataValue::from)
                    .collect(),
            ),
        );
        params.insert(
            "missing_evidence".to_string(),
            DataValue::List(
                review
                    .missing_evidence
                    .iter()
                    .cloned()
                    .map(DataValue::from)
                    .collect(),
            ),
        );
        params.insert(
            "rationale".to_string(),
            DataValue::List(
                review
                    .rationale
                    .iter()
                    .cloned()
                    .map(DataValue::from)
                    .collect(),
            ),
        );
        params.insert(
            "citation_ref".to_string(),
            review.citation.ref_id.clone().into(),
        );
        params.insert(
            "citation_hash".to_string(),
            option_value(
                review
                    .citation
                    .content_hash
                    .as_ref()
                    .map(|hash| hash.as_str().to_string()),
            ),
        );
        params.insert(
            "record_name".to_string(),
            option_value(review.citation.record_name.clone()),
        );
        db.raw_query_mut_params(
            r#"?[decision_id, member_id, schema_version, procedure_id, node_id, branch_id, generation, artifact_id, artifact_surface_hash, evaluation_hash, config_hash, change_set_hash, verdict, confidence, blocking_findings, missing_evidence, rationale, citation_ref, citation_hash, record_name] <- [[$decision_id, $member_id, $schema_version, $procedure_id, $node_id, $branch_id, $generation, $artifact_id, $artifact_surface_hash, $evaluation_hash, $config_hash, $change_set_hash, $verdict, $confidence, $blocking_findings, $missing_evidence, $rationale, $citation_ref, $citation_hash, $record_name]] :put eval_selection_patch_review { decision_id, member_id => schema_version, procedure_id, node_id, branch_id, generation, artifact_id, artifact_surface_hash, evaluation_hash, config_hash, change_set_hash, verdict, confidence, blocking_findings, missing_evidence, rationale, citation_ref, citation_hash, record_name }"#,
            params,
        )
        .expect("insert review");
        for (index, change) in review.changes.iter().enumerate() {
            insert_patch_change(
                db,
                decision_id,
                member_id,
                i64::try_from(index).expect("change index"),
                change,
            );
        }
    }

    fn insert_patch_change(
        db: &Database,
        decision_id: &str,
        member_id: &str,
        change_index: i64,
        change: &PatchChange,
    ) {
        let params = BTreeMap::from([
            ("decision_id".to_string(), decision_id.to_string().into()),
            ("member_id".to_string(), member_id.to_string().into()),
            ("change_index".to_string(), change_index.into()),
            (
                "relpath".to_string(),
                change.relpath.display().to_string().into(),
            ),
            (
                "source_content_hash".to_string(),
                option_value(change.source_content_hash.clone()),
            ),
            (
                "proposed_content_hash".to_string(),
                option_value(change.proposed_content_hash.clone()),
            ),
        ]);
        db.raw_query_mut_params(
            r#"?[decision_id, member_id, change_index, relpath, source_content_hash, proposed_content_hash] <- [[$decision_id, $member_id, $change_index, $relpath, $source_content_hash, $proposed_content_hash]] :put eval_selection_patch_change { decision_id, member_id, change_index => relpath, source_content_hash, proposed_content_hash }"#,
            params,
        )
        .expect("insert change");
    }

    fn option_value(value: Option<String>) -> DataValue {
        value.map(DataValue::from).unwrap_or(DataValue::Null)
    }

    #[test]
    fn channel_counter_ignores_nested_child_worktree_fixtures() {
        let tmp = tempfile::tempdir().expect("tmp");
        let nodes = tmp.path().join("nodes");
        let channel = nodes.join("node-a/channels/runtime-a/child-to-parent.jsonl");
        let fixture = nodes.join("node-a/worktree/tests/fixtures/child-to-parent.jsonl");
        fs::create_dir_all(channel.parent().expect("channel parent")).expect("channel dir");
        fs::create_dir_all(fixture.parent().expect("fixture parent")).expect("fixture dir");
        fs::write(&channel, "{}\n{}\n").expect("channel file");
        fs::write(&fixture, "{}\n{}\n{}\n").expect("fixture file");

        assert_eq!(count_channel_lines(&nodes).expect("count channels"), 2);
    }

    #[test]
    fn continuation_counter_counts_only_selected_or_stopped_decisions() {
        let tmp = tempfile::tempdir().expect("tmp");
        let journal = tmp.path().join("transition-journal.jsonl");
        fs::write(
            &journal,
            concat!(
                r#"{"kind":"successor","state":{"selected":{}}}"#,
                "\n",
                r#"{"kind":"successor","state":{"checkout":{}}}"#,
                "\n",
                r#"{"kind":"active_checkout_advanced"}"#,
                "\n",
                r#"{"kind":"successor_handoff"}"#,
                "\n",
                r#"{"kind":"successor","state":{"stopped":{}}}"#,
                "\n",
            ),
        )
        .expect("journal");

        assert_eq!(count_continuation(&journal).expect("count continuation"), 2);
    }

    #[test]
    fn runtime_completion_does_not_hide_active_stop() {
        use crate::{
            cli::prototype1_state::event::{RecordedAt, RuntimeId},
            intervention::{Prototype1ContinuationDecision, Prototype1ContinuationDisposition},
        };

        let campaign_id = CampaignId::from("campaign");
        let active = identity::ParentIdentity::root_bootstrap(
            campaign_id.clone(),
            "node-parent",
            "instance-parent",
            "branch-parent",
            None,
        );
        let decision = Prototype1ContinuationDecision {
            disposition: Prototype1ContinuationDisposition::StopNoSelectedBranch,
            selected_next_branch_id: None,
            selected_branch_disposition: None,
            next_generation: 2,
            total_nodes_after_continue: 2,
        };
        let stopped = JournalEntry::Successor(successor::Record::stopped_without_attempt(
            campaign_id.clone(),
            "node-parent".to_string(),
            decision,
        ));
        let completed = JournalEntry::Successor(successor::Record {
            runtime_id: Some(RuntimeId(uuid::Uuid::from_u128(1))),
            recorded_at: RecordedAt(2),
            campaign_id: campaign_id.clone(),
            node_id: "node-parent".to_string(),
            state: successor::State::Completed {
                status:
                    crate::cli::prototype1_state::invocation::SuccessorCompletionStatus::Succeeded,
                completion_path: PathBuf::from("/tmp/completion.json"),
                trace_path: None,
                detail: None,
            },
        });
        let started = JournalEntry::ParentStarted(
            crate::cli::prototype1_state::journal::ParentStartedEntry {
                recorded_at: RecordedAt(1),
                campaign_id: campaign_id.clone(),
                parent_identity: active.clone(),
                repo_root: PathBuf::from("/tmp/repo"),
                handoff_runtime_id: Some(RuntimeId(uuid::Uuid::from_u128(1))),
                pid: 42,
            },
        );
        let entries = vec![started, stopped, completed];

        assert!(has_turn_stop(&entries, &active));
        assert!(!has_turn_stop(
            &entries,
            &identity::ParentIdentity::root_bootstrap(
                campaign_id,
                "node-other",
                "instance-other",
                "branch-other",
                None,
            )
        ));
    }

    #[test]
    fn incomplete_outcome_accepts_timeout_or_exit() {
        use crate::cli::prototype1_state::{
            event::{RecordedAt, RuntimeId},
            identity::ParentIdentity,
            journal::{ActiveCheckoutAdvancedEntry, Streams},
        };

        let tmp = tempfile::tempdir().expect("tmp");
        let journal = tmp.path().join("transition-journal.jsonl");
        let campaign_id = CampaignId::from("campaign");
        let predecessor = ParentIdentity::root_bootstrap(
            campaign_id.clone(),
            "node-parent",
            "instance",
            "branch-parent",
            None,
        );
        let selected = ParentIdentity::root_bootstrap(
            campaign_id.clone(),
            "node-successor",
            "instance",
            "branch-successor",
            None,
        );
        let checkout = JournalEntry::ActiveCheckoutAdvanced(ActiveCheckoutAdvancedEntry {
            recorded_at: RecordedAt(1),
            campaign_id: campaign_id.clone(),
            previous_parent_identity: Some(predecessor),
            selected_parent_identity: selected,
            active_parent_root: PathBuf::from("/tmp/repo"),
            selected_branch: "branch-successor".to_string(),
            installed_commit: "abc123".to_string(),
        });
        let runtime = RuntimeId(uuid::Uuid::from_u128(1));
        let spawned = |node: &str, runtime_id| {
            JournalEntry::Successor(successor::Record {
                runtime_id: Some(runtime_id),
                recorded_at: RecordedAt(2),
                campaign_id: campaign_id.clone(),
                node_id: node.to_string(),
                state: successor::State::Spawned {
                    pid: 42,
                    incarnation: None,
                    active_parent_root: PathBuf::from("/tmp/repo"),
                    binary_path: PathBuf::from("/tmp/ploke-eval"),
                    invocation_path: PathBuf::from("/tmp/invocation.json"),
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                    streams: Streams {
                        stdout: PathBuf::from("/tmp/stdout"),
                        stderr: PathBuf::from("/tmp/stderr"),
                    },
                },
            })
        };
        let successor_outcome = |node: &str, runtime_id, state| {
            JournalEntry::Successor(successor::Record {
                runtime_id: Some(runtime_id),
                recorded_at: RecordedAt(3),
                campaign_id: campaign_id.clone(),
                node_id: node.to_string(),
                state,
            })
        };
        let write_entries = |entries: &[JournalEntry]| {
            let text = entries
                .iter()
                .map(|entry| serde_json::to_string(entry).expect("serialize journal entry"))
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(&journal, format!("{text}\n")).expect("journal");
        };

        for label in ["timed_out", "exited_before_ready"] {
            let state = match label {
                "timed_out" => successor::State::TimedOut {
                    waited_ms: 1,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                },
                "exited_before_ready" => successor::State::ExitedBeforeReady { exit_code: Some(1) },
                _ => unreachable!(),
            };
            write_entries(&[
                checkout.clone(),
                spawned("node-successor", runtime),
                successor_outcome("node-successor", runtime, state),
            ]);

            assert_eq!(
                count_latest_outcome(&journal, &["timed_out", "exited_before_ready"])
                    .expect("count incomplete outcome"),
                1,
                "{label} alone must satisfy the alternative terminal evidence"
            );
        }

        let timed_out = successor_outcome(
            "node-successor",
            runtime,
            successor::State::TimedOut {
                waited_ms: 1,
                ready_path: PathBuf::from("/tmp/ready.jsonl"),
            },
        );
        let unrelated = RuntimeId(uuid::Uuid::from_u128(2));
        write_entries(&[
            checkout.clone(),
            spawned("node-successor", runtime),
            timed_out,
            spawned("node-unrelated", unrelated),
            successor_outcome(
                "node-unrelated",
                unrelated,
                successor::State::Ready {
                    pid: 43,
                    ready_path: PathBuf::from("/tmp/unrelated-ready.jsonl"),
                    controller: None,
                },
            ),
        ]);
        assert_eq!(
            count_latest_outcome(&journal, &["timed_out", "exited_before_ready"])
                .expect("count latest outcome"),
            1,
            "an unrelated later attempt must not replace the selected successor outcome"
        );

        let retry = RuntimeId(uuid::Uuid::from_u128(3));
        write_entries(&[
            checkout,
            spawned("node-successor", runtime),
            successor_outcome(
                "node-successor",
                runtime,
                successor::State::TimedOut {
                    waited_ms: 1,
                    ready_path: PathBuf::from("/tmp/ready.jsonl"),
                },
            ),
            spawned("node-successor", retry),
            successor_outcome(
                "node-successor",
                retry,
                successor::State::Ready {
                    pid: 44,
                    ready_path: PathBuf::from("/tmp/retry-ready.jsonl"),
                    controller: None,
                },
            ),
        ]);
        assert_eq!(
            count_latest_outcome(&journal, &["timed_out", "exited_before_ready"])
                .expect("count retry outcome"),
            0,
            "a newer attempt for the selected node owns the audit outcome"
        );
    }
}
