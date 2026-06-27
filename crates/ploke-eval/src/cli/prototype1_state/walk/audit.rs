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
use cozo::{DataValue, ScriptMutability};
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
use crate::cli::prototype1_state::eval_store::prototype1_eval_store_db_path;

const SCHEMA_VERSION: &str = "prototype1.walk.audit.v1";

const EVAL_RELS: &[(&str, &str)] = &[
    ("eval_campaign", "campaign_id"),
    ("eval_campaign_eval_policy", "campaign_id"),
    ("eval_campaign_eval_budget", "campaign_id"),
    ("eval_campaign_protocol_policy", "campaign_id"),
    ("eval_profile_commitment", "profile_ref_id"),
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
    ("eval_selection_candidate", "decision_id"),
    ("eval_selection_finding", "finding_id"),
    ("eval_selection_score", "decision_id"),
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
pub(crate) struct WalkAuditReport {
    pub(crate) schema_version: String,
    pub(crate) generated_at: String,
    pub(crate) scope: Prototype1StateWalkAuditScope,
    pub(crate) transition_filter: Option<Prototype1StateWalkAuditTransition>,
    pub(crate) phase: WalkPhase,
    pub(crate) verbose: bool,
    pub(crate) with_note: bool,
    pub(crate) repo_root: PathBuf,
    pub(crate) campaign: CampaignAudit,
    pub(crate) documents: Vec<DocumentAudit>,
    pub(crate) database: DatabaseAudit,
    pub(crate) transition: TransitionAudit,
    pub(crate) transitions: Vec<TransitionChecklist>,
    pub(crate) summary: AuditSummary,
    pub(crate) notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TransitionChecklist {
    pub(crate) transition: String,
    pub(crate) from: WalkPhase,
    pub(crate) to: WalkPhase,
    pub(crate) file_status: PersistenceStatus,
    pub(crate) db_status: PersistenceStatus,
    pub(crate) overall_status: PersistenceStatus,
    pub(crate) items: Vec<PersistenceItemAudit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistenceItemAudit {
    pub(crate) name: String,
    pub(crate) file: PersistenceSide,
    pub(crate) database: PersistenceSide,
    pub(crate) overall_status: PersistenceStatus,
    pub(crate) note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PersistenceSide {
    pub(crate) status: PersistenceStatus,
    pub(crate) count: Option<i64>,
    pub(crate) path: Option<PathBuf>,
    pub(crate) relation: Option<String>,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PersistenceStatus {
    Ok,
    Partial,
    None,
    NotApplicable,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CampaignAudit {
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) source: CampaignSource,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CampaignSource {
    Explicit,
    ParentIdentity,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DocumentAudit {
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) role: DocumentRole,
    pub(crate) expectation: DocumentExpectation,
    pub(crate) expected_at: ExpectedAt,
    pub(crate) exists: bool,
    pub(crate) status: DocumentStatus,
    pub(crate) sha256: Option<String>,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentRole {
    Authority,
    Projection,
    DerivedCache,
    Pointer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentExpectation {
    Required,
    Optional,
    DerivedIfMissing,
    WrittenByTransition,
    PathOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedAt {
    Precondition,
    TransitionOutput,
    TransitionPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DocumentStatus {
    Ok,
    Missing,
    ReadError,
    ParseError,
    NotChecked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DatabaseAudit {
    pub(crate) path: Option<PathBuf>,
    pub(crate) exists: bool,
    pub(crate) status: DatabaseStatus,
    pub(crate) relation_counts: Vec<RelationCount>,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DatabaseStatus {
    Ok,
    Missing,
    OpenError,
    CampaignUnresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RelationCount {
    pub(crate) relation: String,
    pub(crate) exists: bool,
    pub(crate) count: Option<i64>,
    pub(crate) detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TransitionAudit {
    pub(crate) transition: String,
    pub(crate) from: WalkPhase,
    pub(crate) to: WalkPhase,
    pub(crate) code_symbol: String,
    pub(crate) expected_file_writes: Vec<ExpectedPersistence>,
    pub(crate) expected_db_writes: Vec<ExpectedPersistence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExpectedPersistence {
    pub(crate) surface: String,
    pub(crate) expectation: DocumentExpectation,
    pub(crate) status: ExpectedStatus,
    pub(crate) detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedStatus {
    Expected,
    NotExpected,
    Conditional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AuditSummary {
    pub(crate) required_documents: usize,
    pub(crate) required_ok: usize,
    pub(crate) missing_required: usize,
    pub(crate) parse_errors: usize,
    pub(crate) db_rows_total: i64,
    pub(crate) expected_transition_db_rows: i64,
    pub(crate) observed_transition_db_rows: i64,
    pub(crate) verdict: AuditVerdict,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditVerdict {
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

    let database = database_audit(campaign_audit.campaign_id.as_ref());
    let transition = r0_to_r1_expectations();
    let mut transitions =
        transition_checklist(campaign_audit.campaign_id.as_ref(), &docs, &database);
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
    pub(crate) fn render_table(&self) -> String {
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

fn database_audit(campaign_id: Option<&CampaignId>) -> DatabaseAudit {
    let Some(campaign_id) = campaign_id else {
        return DatabaseAudit {
            path: None,
            exists: false,
            status: DatabaseStatus::CampaignUnresolved,
            relation_counts: Vec::new(),
            detail: Some(
                "campaign unresolved; cannot derive prototype1/eval-store.cozo.sqlite".to_string(),
            ),
        };
    };

    let manifest = match campaign_manifest_path(campaign_id) {
        Ok(path) => path,
        Err(error) => {
            return DatabaseAudit {
                path: None,
                exists: false,
                status: DatabaseStatus::OpenError,
                relation_counts: Vec::new(),
                detail: Some(error.to_string()),
            };
        }
    };
    let path = prototype1_eval_store_db_path(&manifest);
    if !path.exists() {
        return DatabaseAudit {
            path: Some(path),
            exists: false,
            status: DatabaseStatus::Missing,
            relation_counts: relation_count_missing(),
            detail: None,
        };
    }

    match load_relation_counts(&path) {
        Ok(relation_counts) => DatabaseAudit {
            path: Some(path),
            exists: true,
            status: DatabaseStatus::Ok,
            relation_counts,
            detail: None,
        },
        Err(error) => DatabaseAudit {
            path: Some(path),
            exists: true,
            status: DatabaseStatus::OpenError,
            relation_counts: Vec::new(),
            detail: Some(error),
        },
    }
}

fn load_relation_counts(path: &std::path::Path) -> Result<Vec<RelationCount>, String> {
    let db = cozo::new_cozo_mem().map_err(|source| source.to_string())?;
    db.restore_backup(path)
        .map_err(|source| source.to_string())?;
    let relations = db
        .run_script(
            "::relations",
            Default::default(),
            ScriptMutability::Immutable,
        )
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
        match db.run_script(&query, Default::default(), ScriptMutability::Immutable) {
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
}
