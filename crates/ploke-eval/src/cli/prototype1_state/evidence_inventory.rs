//! Prototype 1 evidence inventory schema.
//!
//! This module defines the row schema used by human-facing documentation and
//! machine-facing CLI output (embedded in `ploke-eval history preview`, including
//! `--format json`; a dedicated `history inventory` subcommand may alias the same).
//!
//! The inventory is a *reporting* surface: it must not upgrade authority. It
//! names evidence surfaces, where they live, how they are discovered, what
//! their current authority treatment is, and whether successor-selection
//! material can be **replayed from sealed History** versus **projection-only**
//! filesystem state.

use serde::Serialize;

use super::evidence_class::EvidenceClass;

/// How an evidence surface participates in sealed History versus mutable projections.
///
/// This distinguishes “decision bundles committed under Crown / block append” from
/// operator projections such as [`AuthorityTreatment::Projection`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HistoryCommitmentLane {
    /// Scheduler, branch catalogs, and similar: must not justify selection alone from sealed History.
    ProjectionOnly,
    /// Append-only sealed blocks (e.g. `SelectionDecisionEntry` payloads in the block segment).
    SealedHistory,
    /// Typed on-disk/importer inputs; preview may ingest; a ruling Parent may mirror into
    /// `SelectionDecisionEntry` / inlined evaluation payloads at seal time.
    IngressMaySealInDecision,
}

impl HistoryCommitmentLane {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ProjectionOnly => "projection_only",
            Self::SealedHistory => "sealed_history",
            Self::IngressMaySealInDecision => "ingress_may_seal_in_decision",
        }
    }
}

/// One inventory row describing an evidence surface (file/directory/pattern).
///
/// This is intentionally a narrow, stable schema suitable for both:
/// - markdown table rendering (humans)
/// - `--format json` output (diffing / tooling)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InventoryRow {
    /// Stable logical identifier for the surface (diff-friendly).
    pub(crate) id: &'static str,

    /// Prototype 1 evidence class when the surface is part of the campaign-local
    /// `prototype1/` importer inventory.
    ///
    /// External referenced evidence may not map to an `EvidenceClass`.
    pub(crate) class: Option<EvidenceClass>,

    /// One or more location patterns for the same logical surface.
    pub(crate) locations: Vec<InventoryLocation>,

    /// Record container/encoding (JSON, JSONL, directory of JSON, etc.).
    pub(crate) record_format: RecordFormat,

    /// Record schema/version identifier where known.
    pub(crate) schema: Option<&'static str>,

    /// Current authority treatment category.
    pub(crate) treatment: AuthorityTreatment,

    /// Sealed History lane: committed block payload vs ingress that may mirror vs projection-only.
    pub(crate) history_commitment: HistoryCommitmentLane,

    /// How this surface is discovered/enumerated by current read-only paths.
    pub(crate) discovery: DiscoveryMethod,

    /// Producers (writers) known to create the surface.
    pub(crate) producers: Vec<&'static str>,

    /// Consumers (readers) known to use the surface.
    pub(crate) consumers: Vec<&'static str>,

    /// Minimum provenance keys expected in the payload or recoverable from the
    /// surrounding location.
    pub(crate) provenance_keys: Vec<&'static str>,

    /// Known gaps and diagnostics notes (kept short; detailed prose belongs in
    /// the doc generator).
    pub(crate) notes: Vec<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct InventoryLocation {
    /// Which logical root the path pattern is relative to.
    pub(crate) root: InventoryRoot,
    /// A display pattern (may include placeholders like `<campaign-id>`).
    pub(crate) pattern: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InventoryRoot {
    /// `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`
    CampaignPrototype1,
    /// `~/.ploke-eval/campaigns/<campaign-id>/`
    CampaignRoot,
    /// The active checkout (repo root) currently hosting `.ploke/...` state.
    ActiveCheckout,
    /// `~/.ploke-eval/instances/prototype1/<campaign-id>/...`
    InstancesPrototype1,
    /// A referenced location outside the above roots (rare; keep explicit).
    OtherExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RecordFormat {
    Json,
    Jsonl,
    DirOfJson,
    DirTree,
    Log,
    Archive,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuthorityTreatment {
    /// Candidate for admission under explicit policy (not automatically authority).
    IngressCandidate,
    /// Read-only admitted evidence (preview/import boundary) with clear provenance.
    AdmittedPreview,
    /// Mutable projection; must not be treated as sealed authority.
    Projection,
    /// A catalog that primarily references other evidence surfaces.
    RefOnly,
    /// Mixed or conditional treatment (e.g. class-conditional fallbacks).
    Conditional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiscoveryMethod {
    /// A single explicit path.
    ExplicitPath,
    /// Enumerated by scanning a directory.
    DirectoryScan,
    /// Enumerated by scanning nested per-node directories.
    NestedDirectoryScan,
    /// Recovered/inferred from other records (e.g. node id inferred from path).
    Inferred,
    /// Documented but not currently discoverable via code enumeration.
    DocumentedOnly,
}

/// Canonical Prototype 1 evidence surface catalog for reporting and JSON export.
///
/// [`HistoryCommitmentLane`] marks whether selection-grade material is **sealed**
/// (`SealedHistory`), may be **copied into** a sealed decision at Parent lock
/// (`IngressMaySealInDecision`), or remains **projection-only** (`ProjectionOnly`).
pub(crate) fn prototype1_evidence_inventory_rows() -> Vec<InventoryRow> {
    vec![
        InventoryRow {
            id: "prototype1.transition_journal",
            class: Some(EvidenceClass::TransitionJournal),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "transition-journal.jsonl",
            }],
            record_format: RecordFormat::Jsonl,
            schema: Some("prototype1-transition-journal.jsonl"),
            treatment: AuthorityTreatment::AdmittedPreview,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::ExplicitPath,
            producers: vec!["prototype1-state journal", "parent/child transitions"],
            consumers: vec!["history preview", "metrics", "transition replay"],
            provenance_keys: vec!["campaign_id", "recorded_at", "line_hash"],
            notes: vec![
                "Imported line-by-line into History-shaped preview entries",
                "Citations may also appear inside sealed selection mirrors when present",
            ],
        },
        InventoryRow {
            id: "prototype1.scheduler_json",
            class: Some(EvidenceClass::Scheduler),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "scheduler.json",
            }],
            record_format: RecordFormat::Json,
            schema: Some("prototype1-scheduler"),
            treatment: AuthorityTreatment::Projection,
            history_commitment: HistoryCommitmentLane::ProjectionOnly,
            discovery: DiscoveryMethod::ExplicitPath,
            producers: vec!["prototype1-state"],
            consumers: vec!["history preview (deferred)", "metrics", "branch planning"],
            provenance_keys: vec!["campaign_id", "schema_version"],
            notes: vec![
                "Mutable operator dashboard input; not a sealed selection authority",
                "Selection replay must not depend on this file alone",
            ],
        },
        InventoryRow {
            id: "prototype1.branch_registry",
            class: Some(EvidenceClass::BranchRegistry),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "branches.json",
            }],
            record_format: RecordFormat::Json,
            schema: Some("prototype1-branch-registry"),
            treatment: AuthorityTreatment::RefOnly,
            history_commitment: HistoryCommitmentLane::ProjectionOnly,
            discovery: DiscoveryMethod::ExplicitPath,
            producers: vec!["prototype1-state", "intervention synthesis"],
            consumers: vec![
                "history preview (deferred)",
                "metrics",
                "branch materialization",
            ],
            provenance_keys: vec!["campaign_id", "branch_id"],
            notes: vec![
                "Mutable catalog of branch refs; projection-plus-evidence-refs treatment",
                "Decision-grade selection uses sealed payloads, not this registry alone",
            ],
        },
        InventoryRow {
            id: "prototype1.evaluations_dir",
            class: Some(EvidenceClass::Evaluation),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "evaluations/**/*.json",
            }],
            record_format: RecordFormat::DirOfJson,
            schema: Some("prototype1-evaluation"),
            treatment: AuthorityTreatment::IngressCandidate,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::DirectoryScan,
            producers: vec!["eval runner", "prototype1-state"],
            consumers: vec!["history preview", "child evidence", "successor selection"],
            provenance_keys: vec!["node_id", "branch_id", "runtime_id", "evaluator"],
            notes: vec![
                "Grouped into ChildEvidenceSet and may be mirrored in EvaluationPayload::sealed_evidence",
            ],
        },
        InventoryRow {
            id: "prototype1.invocation_records",
            class: Some(EvidenceClass::Invocation),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "nodes/<node-id>/invocations/*.json",
            }],
            record_format: RecordFormat::DirOfJson,
            schema: Some("prototype1-invocation"),
            treatment: AuthorityTreatment::IngressCandidate,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::NestedDirectoryScan,
            producers: vec!["prototype1-state"],
            consumers: vec!["history preview", "child evidence", "successor selection"],
            provenance_keys: vec!["node_id", "runtime_id", "generation"],
            notes: vec!["May be cited from sealed candidate evidence snapshots"],
        },
        InventoryRow {
            id: "prototype1.attempt_results",
            class: Some(EvidenceClass::AttemptResult),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "nodes/<node-id>/results/*.json",
            }],
            record_format: RecordFormat::DirOfJson,
            schema: Some("prototype1-attempt-result"),
            treatment: AuthorityTreatment::IngressCandidate,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::NestedDirectoryScan,
            producers: vec!["prototype1-state child"],
            consumers: vec!["history preview", "child evidence", "successor selection"],
            provenance_keys: vec!["node_id", "runtime_id"],
            notes: vec!["May be cited from sealed candidate evidence snapshots"],
        },
        InventoryRow {
            id: "prototype1.successor_ready",
            class: Some(EvidenceClass::SuccessorReady),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "nodes/<node-id>/successor-ready/*.json",
            }],
            record_format: RecordFormat::DirOfJson,
            schema: Some("prototype1-successor-ready"),
            treatment: AuthorityTreatment::Conditional,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::NestedDirectoryScan,
            producers: vec!["prototype1-state successor handoff"],
            consumers: vec!["history preview", "child evidence"],
            provenance_keys: vec!["node_id", "runtime_id"],
            notes: vec!["May be cited from sealed candidate evidence snapshots"],
        },
        InventoryRow {
            id: "prototype1.successor_completion",
            class: Some(EvidenceClass::SuccessorCompletion),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "nodes/<node-id>/successor-completion/*.json",
            }],
            record_format: RecordFormat::DirOfJson,
            schema: Some("prototype1-successor-completion"),
            treatment: AuthorityTreatment::IngressCandidate,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::NestedDirectoryScan,
            producers: vec!["prototype1-state successor"],
            consumers: vec!["history preview", "child evidence"],
            provenance_keys: vec!["node_id", "runtime_id"],
            notes: vec!["May be cited from sealed candidate evidence snapshots"],
        },
        InventoryRow {
            id: "prototype1.node_record",
            class: Some(EvidenceClass::NodeRecord),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "nodes/<node-id>/node.json",
            }],
            record_format: RecordFormat::Json,
            schema: Some("prototype1-node-record"),
            treatment: AuthorityTreatment::Projection,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::NestedDirectoryScan,
            producers: vec!["prototype1-setup", "prototype1-state"],
            consumers: vec!["history preview", "metrics"],
            provenance_keys: vec!["node_id", "branch_id", "generation"],
            notes: vec![
                "Planner-facing projection that may still be mirrored inside sealed aggregates",
            ],
        },
        InventoryRow {
            id: "prototype1.runner_request",
            class: Some(EvidenceClass::RunnerRequest),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "nodes/<node-id>/runner-request.json",
            }],
            record_format: RecordFormat::Json,
            schema: Some("prototype1-runner-request"),
            treatment: AuthorityTreatment::IngressCandidate,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::NestedDirectoryScan,
            producers: vec!["prototype1-state"],
            consumers: vec!["history preview", "metrics"],
            provenance_keys: vec!["node_id", "runtime_id"],
            notes: vec!["May be cited from sealed runtime evidence rows"],
        },
        InventoryRow {
            id: "prototype1.runner_result",
            class: Some(EvidenceClass::RunnerResult),
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "nodes/<node-id>/runner-result.json",
            }],
            record_format: RecordFormat::Json,
            schema: Some("prototype1-runner-result"),
            treatment: AuthorityTreatment::Conditional,
            history_commitment: HistoryCommitmentLane::IngressMaySealInDecision,
            discovery: DiscoveryMethod::NestedDirectoryScan,
            producers: vec!["eval runner"],
            consumers: vec!["history preview", "metrics"],
            provenance_keys: vec!["node_id", "runtime_id"],
            notes: vec![
                "Projection unless attempt_result is missing; may be cited from sealed evidence",
            ],
        },
        InventoryRow {
            id: "prototype1.history.block_segment",
            class: None,
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "history/blocks/segment-*.jsonl",
            }],
            record_format: RecordFormat::Jsonl,
            schema: Some("prototype1-history-block-segment"),
            treatment: AuthorityTreatment::AdmittedPreview,
            history_commitment: HistoryCommitmentLane::SealedHistory,
            discovery: DiscoveryMethod::DirectoryScan,
            producers: vec!["prototype1-state parent seal/append"],
            consumers: vec![
                "history preview sealed selection projections",
                "lineage tooling",
            ],
            provenance_keys: vec!["lineage_id", "block_height", "entry_hashes"],
            notes: vec![
                "Append-only verified blocks; successor SelectionDecisionEntry payloads live here",
            ],
        },
        InventoryRow {
            id: "prototype1.history.selection_decision_entry",
            class: None,
            locations: vec![InventoryLocation {
                root: InventoryRoot::CampaignPrototype1,
                pattern: "history/blocks/segment-*.jsonl (EntryPayload::SelectionDecision)",
            }],
            record_format: RecordFormat::Jsonl,
            schema: Some("prototype1-history-selection-decision"),
            treatment: AuthorityTreatment::AdmittedPreview,
            history_commitment: HistoryCommitmentLane::SealedHistory,
            discovery: DiscoveryMethod::Inferred,
            producers: vec!["parent pre-seal path (successor selection)"],
            consumers: vec![
                "sealed_selection_commitments projection",
                "cross-machine selection replay",
            ],
            provenance_keys: vec![
                "procedure_or_policy",
                "scope",
                "considered_order_hash",
                "candidate_set.root",
                "candidate_set.memberships[].proof",
                "decision_hash",
            ],
            notes: vec![
                "Inline-first EvaluationPayload list plus authenticated candidate-set root/proofs for all considered candidates when Parent seals",
                "Not a separate filesystem row; committed inside sealed block JSON",
            ],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_and_registry_are_projection_only_lanes() {
        let rows = prototype1_evidence_inventory_rows();
        let sched = rows
            .iter()
            .find(|r| r.id == "prototype1.scheduler_json")
            .expect("scheduler row");
        let reg = rows
            .iter()
            .find(|r| r.id == "prototype1.branch_registry")
            .expect("branch registry row");
        assert_eq!(
            sched.history_commitment,
            HistoryCommitmentLane::ProjectionOnly
        );
        assert_eq!(
            reg.history_commitment,
            HistoryCommitmentLane::ProjectionOnly
        );
    }

    #[test]
    fn sealed_history_rows_exist() {
        let rows = prototype1_evidence_inventory_rows();
        assert!(rows.iter().any(|r| {
            r.id == "prototype1.history.block_segment"
                && r.history_commitment == HistoryCommitmentLane::SealedHistory
        }));
        assert!(rows.iter().any(|r| {
            r.id == "prototype1.history.selection_decision_entry"
                && r.history_commitment == HistoryCommitmentLane::SealedHistory
        }));
    }

    #[test]
    fn evaluation_is_ingress_may_seal() {
        let rows = prototype1_evidence_inventory_rows();
        let ev = rows
            .iter()
            .find(|r| r.id == "prototype1.evaluations_dir")
            .expect("evaluations row");
        assert_eq!(
            ev.history_commitment,
            HistoryCommitmentLane::IngressMaySealInDecision
        );
    }
}
