//! Read-only Prototype 1 child evidence grouping over typed store records.
//!
//! The boundary invariant is intentionally loud: persisted Prototype 1 evidence
//! records that cross into this child-evidence store path are `Stored<T>` where
//! `T: Serialize + DeserializeOwned + EvidenceRecord`. Deserialization happens
//! in `history_preview::FsEvidenceStore` before this module assembles anything.
//! If a declared persisted record cannot deserialize as its typed record, that
//! is a store-boundary error and evidence assembly aborts.
//!
//! This module does not parse arbitrary `serde_json::Value` to discover
//! `node_id`, `branch_id`, `runtime_id`, compared-run paths, or metrics. It does
//! not recover semantic identity from file names or directory names. Paths,
//! hashes, and ref ids are provenance for successfully typed records only.
//!
//! The concrete typed inputs grouped here are:
//!
//! - `Stored<JournalEntry>` from `prototype1/transition-journal.jsonl`;
//! - `Stored<Prototype1NodeRecord>` from `nodes/<node-id>/node.json`;
//! - `Stored<Prototype1RunnerRequest>` from `nodes/<node-id>/runner-request.json`;
//! - `Stored<Prototype1RunnerResult>` from latest runner-result projections and
//!   attempt-scoped `results/<runtime-id>.json`;
//! - `Stored<Invocation>` from `invocations/<runtime-id>.json`;
//! - `Stored<SuccessorReadyRecord>` and `Stored<SuccessorCompletionRecord>`;
//! - `Stored<Prototype1BranchEvaluationReport>` from `evaluations/<branch-id>.json`.
//!
//! A [`ChildEvidenceSet`] is a grouped projection over these typed records,
//! source pointers, hashes, classes, and conservative diagnostics. It is not an
//! abstract score object and it is not sealed History authority.
//!
//! `history_preview` still keeps a loose [`Document`] surface for `history
//! preview` catalog/projection compatibility. That surface is outside this
//! child-evidence invariant and must not feed child evidence, metrics,
//! selection, future scoring, or authority semantics.
//!
//! The grouping is intentionally conservative. Ambiguous runtime or branch joins
//! are diagnosed and are not reused for later indirect placement. Conflicting
//! child identity, branch metadata, or evaluation evidence remains visible as
//! diagnostics so downstream consumers can fail closed instead of silently using
//! first/last writer wins.
//!
//! This module is an evidence-layer view, not an authority or scoring layer. A
//! [`ChildEvidenceSet`] may feed operator metrics, report projections, or a
//! fallible projection into the current generation-local successor selector, but
//! assembling the set does not:
//!
//! - select a successor;
//! - score or rank children;
//! - admit an [`Entry`](super::history::Entry) or claim into History;
//! - upgrade degraded/projection sources into sealed authority.
//!
//! Sealed authority remains on the History/Crown path. If selected evidence
//! later becomes admissible, it must be cited and admitted through the existing
//! History algebra (`Locator`, `Verifiable`, `Witnessed`, `Admitted`, claims, or
//! entries) rather than by treating this preview grouping as authority.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::cli_facing::{
    Prototype1BranchEvaluationReport, Prototype1EvalSetIdentity, Prototype1EvaluatorIdentity,
};
use super::evidence_class::EvidenceClass;
use super::history::{
    CandidateCoordinate, CandidateLifecycle, SealedBranchEvidence, SealedCandidateEvidence,
    SealedComparedRunEvidence, SealedEvalSetIdentity, SealedEvaluationEvidence,
    SealedEvaluatorIdentity, SealedEvidenceCitation, SealedProtocolArtifactEvidence,
    SealedRunEvidence, SealedRunProtocolEvidence, SealedRuntimeEvidence,
};
use super::history_preview::{EvidencePointer, EvidenceRecord, Stored};
use super::invocation::{Invocation, SuccessorCompletionRecord, SuccessorReadyRecord};
use super::journal::{JournalEntry, SpawnPhase};
use crate::inner::core::RegisteredRunRole;
use crate::inner::registry::{RunArtifactRefs, RunRegistration, RunRegistrationError};
use crate::intervention::{Prototype1NodeRecord, Prototype1RunnerRequest, Prototype1RunnerResult};
use crate::metric;
use crate::protocol::protocol_aggregate::load_protocol_aggregate;
use crate::protocol_artifacts::{
    PROTOCOL_ARTIFACT_SCHEMA_VERSION, StoredProtocolArtifactFile, load_protocol_artifact,
};
use crate::run_registry::load_registration_for_record_path;
use crate::successor_selection::{CandidateRef, RunComparison, SelectionInput};
use crate::{BranchDisposition, OperationalRunMetrics};

const SCHEMA_VERSION: &str = "prototype1-child-evidence.v1";
pub(crate) const PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID: &str =
    "prototype1.branch_evaluation.operational_metrics.v1";

/// Read-only child evidence grouped from classified preview sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChildEvidenceSet {
    pub(crate) schema_version: String,
    pub(crate) children: Vec<ChildEvidence>,
    pub(crate) unplaced: Vec<EvidenceSource>,
    pub(crate) diagnostics: Vec<EvidenceDiagnostic>,
}

impl ChildEvidenceSet {
    pub(crate) fn from_records(records: &ChildEvidenceRecords) -> Self {
        let mut assembly = Assembly::new(&records.journal);
        let typed = prepare_records(records);
        assembly.index_records(&typed);
        assembly.attach_records(&typed);
        assembly.attach_journal(&records.journal);
        assembly.finish()
    }

    /// No grouped children — used after the filesystem child-evidence assembly failed; callers record a sealed decision diagnostic.
    pub(crate) fn empty_after_unreadable_store() -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            children: Vec::new(),
            unplaced: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn selection_inputs(&self) -> SelectionProjectionSet {
        let mut inputs = Vec::new();
        let mut failures = Vec::new();

        for child in &self.children {
            match child.selection_input() {
                Ok(input) => inputs.push(input),
                Err(error) => failures.push(error),
            }
        }

        SelectionProjectionSet { inputs, failures }
    }

    pub(crate) fn branch_placement(&self) -> BranchPlacement {
        let mut placement = BranchPlacement::default();
        for child in &self.children {
            if let Some(branch_id) = child.branch_id.as_deref() {
                placement.observe(branch_id, &child.node_id);
            }
            for branch in &child.branches {
                placement.observe(&branch.branch_id, &child.node_id);
            }
            for evaluation in &child.evaluations {
                placement.observe(&evaluation.branch_id, &child.node_id);
            }
        }
        placement
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChildEvidenceRecords {
    pub(crate) journal: Vec<Stored<JournalEntry>>,
    pub(crate) nodes: Vec<Stored<Prototype1NodeRecord>>,
    pub(crate) runner_requests: Vec<Stored<Prototype1RunnerRequest>>,
    pub(crate) runner_results: Vec<Stored<Prototype1RunnerResult>>,
    pub(crate) invocations: Vec<Stored<Invocation>>,
    pub(crate) attempt_results: Vec<Stored<Prototype1RunnerResult>>,
    pub(crate) successor_ready: Vec<Stored<SuccessorReadyRecord>>,
    pub(crate) successor_completion: Vec<Stored<SuccessorCompletionRecord>>,
    pub(crate) evaluations: Vec<Stored<Prototype1BranchEvaluationReport>>,
}

impl ChildEvidenceRecords {
    pub(crate) fn sort(&mut self) {
        self.journal
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.nodes
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.runner_requests
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.runner_results
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.invocations
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.attempt_results
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.successor_ready
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.successor_completion
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
        self.evaluations
            .sort_by(|left, right| left.pointer().ref_id().cmp(right.pointer().ref_id()));
    }
}

/// Evidence associated with one scheduler child node when a node join exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ChildEvidence {
    pub(crate) node_id: String,
    pub(crate) parent_node_id: Option<String>,
    pub(crate) generation: Option<u32>,
    pub(crate) branch_id: Option<String>,
    pub(crate) runtimes: Vec<RuntimeEvidence>,
    pub(crate) branches: Vec<BranchEvidence>,
    pub(crate) evaluations: Vec<EvaluationEvidence>,
    pub(crate) documents: Vec<EvidenceSource>,
    pub(crate) journal: Vec<EvidenceSource>,
    pub(crate) diagnostics: Vec<EvidenceDiagnostic>,
}

impl ChildEvidence {
    pub(crate) fn selection_input(&self) -> Result<SelectionInput, SelectionProjectionError> {
        let mut diagnostics = Vec::new();
        append_conflict_diagnostics(self, &mut diagnostics);

        let node_id = require_text(
            Some(self.node_id.as_str()),
            SelectionProjectionField::Node,
            "child evidence has no node_id",
            &mut diagnostics,
        );
        let branch_id = require_text(
            self.branch_id.as_deref(),
            SelectionProjectionField::Branch,
            "child evidence has no branch_id",
            &mut diagnostics,
        );
        let generation = match self.generation {
            Some(generation) => Some(generation),
            None => {
                diagnostics.push(SelectionProjectionDiagnostic::missing(
                    SelectionProjectionField::Generation,
                    "child evidence has no generation",
                ));
                None
            }
        };

        let matching_evaluations = branch_id
            .as_deref()
            .map(|branch_id| {
                self.evaluations
                    .iter()
                    .filter(|eval| eval.branch_id == branch_id)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let evaluation = match matching_evaluations.as_slice() {
            [evaluation] => Some(*evaluation),
            [] => None,
            evaluations => {
                diagnostics.push(SelectionProjectionDiagnostic::invalid(
                    SelectionProjectionField::Evaluation,
                    format!(
                        "child evidence has {} evaluation records for branch_id '{}'",
                        evaluations.len(),
                        branch_id.as_deref().unwrap_or("-")
                    ),
                ));
                None
            }
        };
        if evaluation.is_none() && matching_evaluations.is_empty() {
            let evaluation_message = if branch_id.is_some() {
                "child evidence has no evaluation for its branch_id"
            } else {
                "child evidence has no branch_id for evaluation lookup"
            };
            diagnostics.push(SelectionProjectionDiagnostic::missing(
                SelectionProjectionField::Evaluation,
                evaluation_message,
            ));
            diagnostics.push(SelectionProjectionDiagnostic::missing(
                SelectionProjectionField::Disposition,
                "evaluation evidence has no overall_disposition",
            ));
            diagnostics.push(SelectionProjectionDiagnostic::missing(
                SelectionProjectionField::EvaluationArtifact,
                "evaluation evidence has no evaluation_artifact_path",
            ));
            diagnostics.push(SelectionProjectionDiagnostic::missing(
                SelectionProjectionField::Comparisons,
                "evaluation evidence has no compared instances",
            ));
        }

        let branch_disposition = evaluation.and_then(|evaluation| {
            let Some(disposition) = evaluation.overall_disposition.as_deref() else {
                diagnostics.push(SelectionProjectionDiagnostic::missing(
                    SelectionProjectionField::Disposition,
                    "evaluation evidence has no overall_disposition",
                ));
                return None;
            };
            match parse_disposition(disposition) {
                Some(disposition) => Some(disposition),
                None => {
                    diagnostics.push(SelectionProjectionDiagnostic::missing(
                        SelectionProjectionField::Disposition,
                        format!("unknown overall_disposition '{disposition}'"),
                    ));
                    None
                }
            }
        });

        let evaluation_artifact_path = evaluation.and_then(|evaluation| {
            let Some(path) = evaluation.evaluation_artifact_path.as_ref() else {
                diagnostics.push(SelectionProjectionDiagnostic::missing(
                    SelectionProjectionField::EvaluationArtifact,
                    "evaluation evidence has no evaluation_artifact_path",
                ));
                return None;
            };
            if path.as_os_str().is_empty() {
                diagnostics.push(SelectionProjectionDiagnostic::missing(
                    SelectionProjectionField::EvaluationArtifact,
                    "evaluation evidence has an empty evaluation_artifact_path",
                ));
                None
            } else {
                Some(path.clone())
            }
        });

        let comparisons = evaluation
            .map(|evaluation| project_comparisons(evaluation, &mut diagnostics))
            .unwrap_or_default();

        if diagnostics.is_empty() {
            Ok(SelectionInput::new(
                CandidateRef {
                    node_id: node_id.expect("node_id checked"),
                    branch_id: branch_id.expect("branch_id checked"),
                    generation: generation.expect("generation checked"),
                },
                branch_disposition.expect("disposition checked"),
                evaluation_artifact_path.expect("evaluation path checked"),
                comparisons,
            ))
        } else {
            Err(SelectionProjectionError {
                node_id: node_id.or_else(|| {
                    if self.node_id.is_empty() {
                        None
                    } else {
                        Some(self.node_id.clone())
                    }
                }),
                diagnostics,
            })
        }
    }
}

pub(crate) fn seal_candidate_evidence_for_history(
    node_id: &str,
    plan_index: usize,
    planner_outcome: &str,
    node_status_label: &str,
    child: Option<&ChildEvidence>,
) -> SealedCandidateEvidence {
    let coordinate = if let Some(child) = child {
        CandidateCoordinate {
            node_id: node_id.to_string(),
            parent_node_id: child.parent_node_id.clone(),
            branch_id: child.branch_id.clone(),
            generation: child.generation,
            plan_index: Some(plan_index as u32),
            primary_runtime_id: child
                .runtimes
                .first()
                .map(|runtime| runtime.runtime_id.clone()),
        }
    } else {
        CandidateCoordinate {
            node_id: node_id.to_string(),
            parent_node_id: None,
            branch_id: None,
            generation: None,
            plan_index: Some(plan_index as u32),
            primary_runtime_id: None,
        }
    };

    let lifecycle = CandidateLifecycle {
        planner_outcome: planner_outcome.to_string(),
        node_status: node_status_label.to_string(),
    };

    let evaluations: Vec<SealedEvaluationEvidence> = child
        .map(|child| {
            child
                .evaluations
                .iter()
                .map(|evaluation| SealedEvaluationEvidence {
                    branch_id: evaluation.branch_id.clone(),
                    evaluation_procedure_id: evaluation.evaluation_procedure_id.clone(),
                    evaluator_identity: evaluation
                        .evaluator_identity
                        .as_ref()
                        .map(seal_evaluator_identity),
                    eval_set_identity: evaluation
                        .eval_set_identity
                        .as_ref()
                        .map(seal_eval_set_identity),
                    evaluation_artifact_citation: evaluation.evaluation_artifact_path.as_ref().map(
                        |path| SealedEvidenceCitation {
                            ref_id: format!("opaque_evaluation_artifact:{}", path.display()),
                            content_hash: None,
                            record_name: None,
                        },
                    ),
                    overall_disposition: evaluation.overall_disposition.clone(),
                    primary_report_citation: citation_from_evidence_source(&evaluation.source),
                    compared_runs: evaluation
                        .compared
                        .iter()
                        .map(seal_compared_run_evidence)
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();

    let runtimes: Vec<SealedRuntimeEvidence> = child
        .map(|child| {
            child
                .runtimes
                .iter()
                .map(|runtime| SealedRuntimeEvidence {
                    runtime_id: runtime.runtime_id.clone(),
                    document_citations: runtime
                        .documents
                        .iter()
                        .map(citation_from_evidence_source)
                        .collect(),
                    journal_citations: runtime
                        .journal
                        .iter()
                        .map(citation_from_evidence_source)
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();

    let branches: Vec<SealedBranchEvidence> = child
        .map(|child| {
            child
                .branches
                .iter()
                .map(|branch| SealedBranchEvidence {
                    branch_id: branch.branch_id.clone(),
                    candidate_id: branch.candidate_id.clone(),
                    source_state_id: branch.source_state_id.clone(),
                    branch_evidence_citations: branch
                        .sources
                        .iter()
                        .map(citation_from_evidence_source)
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();

    let extra_document_citations = child
        .map(|child| {
            child
                .documents
                .iter()
                .map(citation_from_evidence_source)
                .collect()
        })
        .unwrap_or_default();

    let extra_journal_citations = child
        .map(|child| {
            child
                .journal
                .iter()
                .map(citation_from_evidence_source)
                .collect()
        })
        .unwrap_or_default();

    let child_diagnostics = child
        .map(|child| {
            child
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect()
        })
        .unwrap_or_default();

    SealedCandidateEvidence {
        schema_version: 2,
        coordinate,
        lifecycle,
        evaluations,
        runtimes,
        branches,
        extra_document_citations,
        extra_journal_citations,
        child_diagnostics,
    }
}

fn citation_from_evidence_source(source: &EvidenceSource) -> SealedEvidenceCitation {
    SealedEvidenceCitation {
        ref_id: source.pointer.ref_id().to_string(),
        content_hash: Some(source.pointer.hash().clone()),
        record_name: source.record.as_ref().map(|record| record.name.clone()),
    }
}

fn seal_compared_run_evidence(row: &ComparedRunEvidence) -> SealedComparedRunEvidence {
    let baseline_citation =
        compared_run_registration_citation(&row.baseline_run, &row.baseline_registration_path);
    let treatment_citation =
        compared_run_registration_citation(&row.treatment_run, &row.treatment_registration_path);
    let mut diagnostics: Vec<String> = row
        .diagnostics
        .iter()
        .map(|diagnostic| {
            format!(
                "{}:{}:{}",
                diagnostic.severity, diagnostic.field, diagnostic.message
            )
        })
        .collect();
    let baseline_run = row.baseline_run.as_ref().map(seal_run_evidence);
    let treatment_run = row.treatment_run.as_ref().map(seal_run_evidence);
    let baseline_protocol =
        seal_protocol_aggregate("baseline", row.baseline_run.as_ref(), &mut diagnostics);
    let treatment_protocol =
        seal_protocol_aggregate("treatment", row.treatment_run.as_ref(), &mut diagnostics);
    SealedComparedRunEvidence {
        instance_id: row.instance_id.clone(),
        status: row.status.clone(),
        baseline_citation,
        treatment_citation,
        baseline_metrics: row.baseline_metrics.clone(),
        treatment_metrics: row.treatment_metrics.clone(),
        baseline_protocol,
        treatment_protocol,
        oracle_evaluation: row.oracle_evaluation.clone(),
        diagnostics,
        baseline_run,
        treatment_run,
    }
}

fn seal_evaluator_identity(identity: &Prototype1EvaluatorIdentity) -> SealedEvaluatorIdentity {
    SealedEvaluatorIdentity {
        id: identity.id.clone(),
        version: identity.version.clone(),
    }
}

fn seal_eval_set_identity(identity: &Prototype1EvalSetIdentity) -> SealedEvalSetIdentity {
    SealedEvalSetIdentity {
        id: identity.id.clone(),
        kind: identity.kind.clone(),
        authority: identity.authority.clone(),
        explicit: identity.explicit,
        benchmark_family: serde_json::to_value(identity.benchmark_family)
            .ok()
            .and_then(|value| value.as_str().map(str::to_string)),
        dataset_source_count: identity.dataset_sources.len(),
        instance_ids: identity.instance_ids.clone(),
        missing_treatment_instance_ids: identity.missing_treatment_instance_ids.clone(),
        note: identity.note.clone(),
    }
}

fn seal_run_evidence(run: &RunEvidence) -> SealedRunEvidence {
    SealedRunEvidence {
        run_id: run.run_id.clone(),
        task_id: Some(run.task_id.clone()),
        run_role: Some(format!("{:?}", run.run_role)),
        spec_fingerprint: Some(run.spec_fingerprint.clone()),
        model_id: run.model_id.clone(),
        provider_slug: run.provider_slug.clone(),
        protocol: SealedRunProtocolEvidence {
            anchor_path: run.protocol.anchor_path.clone(),
            artifacts: run
                .protocol
                .artifacts
                .iter()
                .map(|artifact| SealedProtocolArtifactEvidence {
                    procedure_name: artifact.procedure_name.clone(),
                    path: Some(artifact.path.clone()),
                    schema_version: Some(artifact.schema_version.clone()),
                    subject_id: Some(artifact.subject_id.clone()),
                })
                .collect(),
            diagnostics: run
                .protocol
                .diagnostics
                .iter()
                .map(|diagnostic| {
                    format!(
                        "{}:{}:{}",
                        diagnostic.severity, diagnostic.field, diagnostic.message
                    )
                })
                .collect(),
        },
    }
}

fn seal_protocol_aggregate(
    arm: &'static str,
    run: Option<&RunEvidence>,
    diagnostics: &mut Vec<String>,
) -> Option<metric::Protocol> {
    let run = run?;
    match load_protocol_aggregate(&run.artifacts.record_path) {
        Ok(aggregate) => Some(metric::Protocol::from(&aggregate)),
        Err(error) => {
            diagnostics.push(format!("{arm}_protocol:aggregate_unavailable:{error}"));
            None
        }
    }
}

fn compared_run_registration_citation(
    run: &Option<RunEvidence>,
    fallback: &Option<PathBuf>,
) -> Option<SealedEvidenceCitation> {
    run.as_ref()
        .and_then(|value| value.registration_path.as_ref())
        .or(fallback.as_ref())
        .map(|path| SealedEvidenceCitation {
            ref_id: format!("opaque_registration:{}", path.display()),
            content_hash: None,
            record_name: Some("run_registration".to_string()),
        })
}

/// Evidence associated with one runtime id inside a child node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuntimeEvidence {
    pub(crate) runtime_id: String,
    pub(crate) documents: Vec<EvidenceSource>,
    pub(crate) journal: Vec<EvidenceSource>,
}

/// Evidence associated with one candidate branch inside a child node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BranchEvidence {
    pub(crate) branch_id: String,
    pub(crate) candidate_id: Option<String>,
    pub(crate) source_state_id: Option<String>,
    pub(crate) target_relpath: Option<PathBuf>,
    pub(crate) sources: Vec<EvidenceSource>,
}

/// Evaluation report evidence for one branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvaluationEvidence {
    pub(crate) branch_id: String,
    pub(crate) evaluation_procedure_id: Option<String>,
    pub(crate) evaluator_identity: Option<Prototype1EvaluatorIdentity>,
    pub(crate) eval_set_identity: Option<Prototype1EvalSetIdentity>,
    pub(crate) evaluation_artifact_path: Option<PathBuf>,
    pub(crate) overall_disposition: Option<String>,
    pub(crate) compared: Vec<ComparedRunEvidence>,
    pub(crate) source: EvidenceSource,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BranchPlacement {
    pub(crate) nodes: BTreeMap<String, String>,
    pub(crate) ambiguous: BTreeSet<String>,
}

impl BranchPlacement {
    fn observe(&mut self, branch_id: &str, node_id: &str) {
        if self.ambiguous.contains(branch_id) {
            return;
        }
        if let Some(existing) = self.nodes.get(branch_id) {
            if existing != node_id {
                self.nodes.remove(branch_id);
                self.ambiguous.insert(branch_id.to_string());
            }
        } else {
            self.nodes
                .insert(branch_id.to_string(), node_id.to_string());
        }
    }
}

/// Run-record paths preserved from one compared evaluation instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ComparedRunEvidence {
    pub(crate) instance_id: Option<String>,
    pub(crate) baseline_registration_path: Option<PathBuf>,
    pub(crate) treatment_registration_path: Option<PathBuf>,
    pub(crate) baseline_record_path: Option<PathBuf>,
    pub(crate) treatment_record_path: Option<PathBuf>,
    pub(crate) baseline_run: Option<RunEvidence>,
    pub(crate) treatment_run: Option<RunEvidence>,
    pub(crate) baseline_metrics: Option<OperationalRunMetrics>,
    pub(crate) treatment_metrics: Option<OperationalRunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_evaluation: Option<crate::mbe::OracleEvaluation>,
    pub(crate) status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) diagnostics: Vec<ComparedRunDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunEvidence {
    pub(crate) registration_path: Option<PathBuf>,
    pub(crate) run_id: String,
    pub(crate) task_id: String,
    pub(crate) run_role: RegisteredRunRole,
    pub(crate) spec_fingerprint: String,
    pub(crate) model_id: Option<String>,
    pub(crate) provider_slug: Option<String>,
    pub(crate) artifacts: RunArtifactRefs,
    #[serde(default)]
    pub(crate) protocol: protocol::Artifacts,
}

impl RunEvidence {
    fn from_registration(
        registration: RunRegistration,
        registration_path: Option<PathBuf>,
    ) -> Self {
        let protocol = protocol::Artifacts::from_run(
            &registration.run_id,
            &registration.frozen_spec.task_id,
            &registration.artifacts,
        );
        Self {
            registration_path,
            run_id: registration.run_id,
            task_id: registration.frozen_spec.task_id,
            run_role: registration.frozen_spec.run_role,
            spec_fingerprint: registration.spec_fingerprint,
            model_id: registration.frozen_spec.model_id,
            provider_slug: registration.frozen_spec.provider_slug,
            artifacts: registration.artifacts,
            protocol,
        }
    }
}

mod protocol {
    use super::*;

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub(crate) struct Artifacts {
        pub(crate) artifacts_dir: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) anchor_path: Option<PathBuf>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub(crate) artifacts: Vec<Artifact>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        pub(crate) diagnostics: Vec<Diagnostic>,
    }

    impl Artifacts {
        pub(crate) fn from_run(
            run_id: &str,
            subject_id: &str,
            artifacts: &RunArtifactRefs,
        ) -> Self {
            let mut evidence = Self {
                artifacts_dir: artifacts.protocol_artifacts_dir.clone(),
                anchor_path: artifacts.protocol_anchor.clone(),
                artifacts: Vec::new(),
                diagnostics: Vec::new(),
            };

            if !artifacts.protocol_artifacts_dir.exists() {
                if artifacts.protocol_anchor.is_some() {
                    let message = format!(
                        "registered protocol anchor exists but protocol artifacts dir '{}' is missing",
                        artifacts.protocol_artifacts_dir.display()
                    );
                    evidence
                        .diagnostics
                        .push(Diagnostic::warning("protocol_artifacts_dir", message));
                }
                return evidence;
            }

            let entries = match fs::read_dir(&artifacts.protocol_artifacts_dir) {
                Ok(entries) => entries,
                Err(error) => {
                    evidence.diagnostics.push(Diagnostic::warning(
                        "protocol_artifacts_dir",
                        format!(
                            "could not read registered protocol artifacts dir '{}': {error}",
                            artifacts.protocol_artifacts_dir.display()
                        ),
                    ));
                    return evidence;
                }
            };

            for entry in entries {
                let path = match entry {
                    Ok(entry) => entry.path(),
                    Err(error) => {
                        evidence.diagnostics.push(Diagnostic::warning(
                            "protocol_artifacts_dir",
                            format!(
                                "could not read an entry from protocol artifacts dir '{}': {error}",
                                artifacts.protocol_artifacts_dir.display()
                            ),
                        ));
                        continue;
                    }
                };
                if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                    continue;
                }

                let loaded = match load_protocol_artifact(&path) {
                    Ok(loaded) => loaded,
                    Err(error) => {
                        evidence.diagnostics.push(Diagnostic::warning(
                            "protocol_artifact",
                            format!(
                                "could not load protocol artifact envelope '{}': {error}",
                                path.display()
                            ),
                        ));
                        continue;
                    }
                };

                if loaded.stored.schema_version != PROTOCOL_ARTIFACT_SCHEMA_VERSION {
                    evidence.diagnostics.push(Diagnostic::warning(
                        "protocol_artifact.schema_version",
                        format!(
                            "protocol artifact '{}' has schema_version '{}' but expected '{}'",
                            loaded.path.display(),
                            loaded.stored.schema_version,
                            PROTOCOL_ARTIFACT_SCHEMA_VERSION
                        ),
                    ));
                    continue;
                }
                if loaded.stored.run_id != run_id {
                    evidence.diagnostics.push(Diagnostic::warning(
                        "protocol_artifact.run_id",
                        format!(
                            "protocol artifact '{}' has run_id '{}' but typed run id is '{}'",
                            loaded.path.display(),
                            loaded.stored.run_id,
                            run_id
                        ),
                    ));
                    continue;
                }
                if loaded.stored.subject_id != subject_id {
                    let message = format!(
                        "protocol artifact '{}' has subject_id '{}' but typed run task id is '{}'",
                        loaded.path.display(),
                        loaded.stored.subject_id,
                        subject_id
                    );
                    evidence
                        .diagnostics
                        .push(Diagnostic::warning("protocol_artifact.subject_id", message));
                    continue;
                }

                evidence.artifacts.push(Artifact::from_file(loaded));
            }

            evidence.artifacts.sort_by(|left, right| {
                right
                    .created_at_ms
                    .cmp(&left.created_at_ms)
                    .then_with(|| right.path.cmp(&left.path))
            });

            if let Some(anchor_path) = artifacts.protocol_anchor.as_ref() {
                let anchor_loaded = evidence
                    .artifacts
                    .iter()
                    .any(|artifact| same_protocol_path(&artifact.path, anchor_path));
                if !anchor_loaded {
                    let message = format!(
                        "registered protocol anchor '{}' was not loaded as typed protocol artifact evidence",
                        anchor_path.display()
                    );
                    evidence
                        .diagnostics
                        .push(Diagnostic::warning("protocol_anchor", message));
                }
            }

            evidence
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(crate) struct Artifact {
        pub(crate) path: PathBuf,
        pub(crate) schema_version: String,
        pub(crate) procedure_name: String,
        pub(crate) subject_id: String,
        pub(crate) run_id: String,
        pub(crate) created_at_ms: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) model_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub(crate) provider_slug: Option<String>,
    }

    impl Artifact {
        fn from_file(file: StoredProtocolArtifactFile) -> Self {
            Self {
                path: file.path,
                schema_version: file.stored.schema_version,
                procedure_name: file.stored.procedure_name,
                subject_id: file.stored.subject_id,
                run_id: file.stored.run_id,
                created_at_ms: file.stored.created_at_ms,
                model_id: file.stored.model_id,
                provider_slug: file.stored.provider_slug,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub(crate) struct Diagnostic {
        pub(crate) severity: String,
        pub(crate) field: String,
        pub(crate) message: String,
    }

    impl Diagnostic {
        fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
            Self {
                severity: "warning".to_string(),
                field: field.into(),
                message: message.into(),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ComparedRunDiagnostic {
    pub(crate) severity: String,
    pub(crate) field: String,
    pub(crate) message: String,
}

impl ComparedRunDiagnostic {
    fn missing(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "missing".to_string(),
            field: field.into(),
            message: message.into(),
        }
    }

    fn invalid(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "invalid".to_string(),
            field: field.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SelectionProjectionSet {
    pub(crate) inputs: Vec<SelectionInput>,
    pub(crate) failures: Vec<SelectionProjectionError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SelectionProjectionError {
    pub(crate) node_id: Option<String>,
    pub(crate) diagnostics: Vec<SelectionProjectionDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SelectionProjectionDiagnostic {
    pub(crate) field: SelectionProjectionField,
    pub(crate) message: String,
}

impl SelectionProjectionDiagnostic {
    fn missing(field: SelectionProjectionField, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }

    fn invalid(field: SelectionProjectionField, message: impl Into<String>) -> Self {
        Self {
            field,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionProjectionField {
    Node,
    Parent,
    Branch,
    BranchMetadata,
    Generation,
    Evaluation,
    Disposition,
    EvaluationArtifact,
    Comparisons,
}

/// Source citation for a grouped document or journal line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvidenceSource {
    pub(crate) class: EvidenceClass,
    pub(crate) kind: String,
    /// How the preview importer labels this class (legacy JSON used `treatment`).
    #[serde(alias = "treatment")]
    pub(crate) preview_import_treatment: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) record: Option<EvidenceRecordSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) facts: Vec<EvidenceFact>,
    pub(crate) pointer: EvidencePointer,
}

impl EvidenceSource {
    fn record<T>(stored: &Stored<T>, kind: impl Into<String>, facts: Vec<EvidenceFact>) -> Self
    where
        T: EvidenceRecord,
    {
        let class = stored.pointer().class();
        Self {
            class,
            kind: kind.into(),
            preview_import_treatment: class.preview_import_treatment().to_string(),
            record: Some(EvidenceRecordSource {
                name: T::RECORD_NAME.to_string(),
                schema: T::SCHEMA.to_string(),
            }),
            facts,
            pointer: stored.pointer().clone(),
        }
    }

    fn journal(pointer: &EvidencePointer, entry: &JournalEntry) -> Self {
        Self {
            class: EvidenceClass::TransitionJournal,
            kind: journal_kind(entry).to_string(),
            preview_import_treatment: EvidenceClass::TransitionJournal
                .preview_import_treatment()
                .to_string(),
            record: Some(EvidenceRecordSource {
                name: <JournalEntry as EvidenceRecord>::RECORD_NAME.to_string(),
                schema: <JournalEntry as EvidenceRecord>::SCHEMA.to_string(),
            }),
            facts: Vec::new(),
            pointer: pointer.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EvidenceRecordSource {
    pub(crate) name: String,
    pub(crate) schema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EvidenceFact {
    pub(crate) field: String,
    pub(crate) origin: EvidenceFactOrigin,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceFactOrigin {
    Typed,
}

/// Non-fatal issue found while grouping degraded evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EvidenceDiagnostic {
    pub(crate) severity: String,
    pub(crate) source_ref: Option<String>,
    pub(crate) message: String,
}

#[derive(Debug, Default)]
struct Coordinates {
    node_id: Option<String>,
    parent_node_id: Option<String>,
    generation: Option<u32>,
    runtime_id: Option<String>,
    branch_id: Option<String>,
    candidate_id: Option<String>,
    source_state_id: Option<String>,
    target_relpath: Option<PathBuf>,
}

struct PreparedRecord {
    source: EvidenceSource,
    coords: Coordinates,
    evaluation: Option<EvaluationEvidenceParts>,
}

struct EvaluationEvidenceParts {
    branch_id: String,
    evaluation_procedure_id: Option<String>,
    evaluator_identity: Option<Prototype1EvaluatorIdentity>,
    eval_set_identity: Option<Prototype1EvalSetIdentity>,
    evaluation_artifact_path: Option<PathBuf>,
    overall_disposition: Option<String>,
    compared: Vec<ComparedRunEvidence>,
}

struct Assembly {
    children: BTreeMap<String, ChildEvidence>,
    runtime_to_node: BTreeMap<String, String>,
    branch_to_node: BTreeMap<String, String>,
    conflicted_runtimes: BTreeSet<String>,
    conflicted_branches: BTreeSet<String>,
    diagnostics: Vec<EvidenceDiagnostic>,
    unplaced: Vec<EvidenceSource>,
}

enum Placement {
    Node(String),
    Missing,
    Ambiguous { label: &'static str, key: String },
}

impl Assembly {
    fn new(journal: &[Stored<JournalEntry>]) -> Self {
        let mut assembly = Self {
            children: BTreeMap::new(),
            runtime_to_node: BTreeMap::new(),
            branch_to_node: BTreeMap::new(),
            conflicted_runtimes: BTreeSet::new(),
            conflicted_branches: BTreeSet::new(),
            diagnostics: Vec::new(),
            unplaced: Vec::new(),
        };
        assembly.index_journal(journal);
        assembly
    }

    fn index_records(&mut self, records: &[PreparedRecord]) {
        for prepared in records {
            self.index_coords(&prepared.coords, &prepared.source);
            if prepared.source.class == EvidenceClass::NodeRecord {
                if let Some(node_id) = prepared.coords.node_id.as_deref() {
                    self.merge_child(node_id, &prepared.coords, &prepared.source);
                }
            }
        }
    }

    fn index_journal(&mut self, journal: &[Stored<JournalEntry>]) {
        for stored in journal {
            let source = EvidenceSource::journal(stored.pointer(), stored.item());
            let coords = journal_coordinates(stored.item());
            self.index_coords(&coords, &source);
        }
    }

    fn index_coords(&mut self, coords: &Coordinates, source: &EvidenceSource) {
        if let (Some(runtime_id), Some(node_id)) = (&coords.runtime_id, &coords.node_id) {
            self.insert_join("runtime", runtime_id, node_id, source);
        }
        if let (Some(branch_id), Some(node_id)) = (&coords.branch_id, &coords.node_id) {
            self.insert_join("branch", branch_id, node_id, source);
        }
    }

    fn insert_join(&mut self, label: &str, key: &str, node_id: &str, source: &EvidenceSource) {
        let map = match label {
            "runtime" => &mut self.runtime_to_node,
            "branch" => &mut self.branch_to_node,
            _ => unreachable!("known join label"),
        };
        let conflicted = match label {
            "runtime" => &mut self.conflicted_runtimes,
            "branch" => &mut self.conflicted_branches,
            _ => unreachable!("known join label"),
        };
        if conflicted.contains(key) {
            return;
        }
        if let Some(existing) = map.get(key) {
            if existing != node_id {
                self.diagnostics.push(EvidenceDiagnostic {
                    severity: "warning".to_string(),
                    source_ref: Some(source.pointer.ref_id().to_string()),
                    message: format!(
                        "conflicting {label} join for '{key}': '{existing}' vs '{node_id}'"
                    ),
                });
                map.remove(key);
                conflicted.insert(key.to_string());
            }
        } else {
            map.insert(key.to_string(), node_id.to_string());
        }
    }

    fn attach_records(&mut self, records: &[PreparedRecord]) {
        for prepared in records {
            let source = prepared.source.clone();
            let coords = &prepared.coords;
            let node_id = match self.node_for(coords) {
                Placement::Node(node_id) => node_id,
                placement => {
                    self.unplaced.push(source.clone());
                    self.push_placement_diagnostic(
                        placement,
                        &source,
                        &format!("{} source", prepared.source.class.as_str()),
                    );
                    continue;
                }
            };
            let child = self.merge_child(&node_id, coords, &source);
            child.documents.push(source.clone());
            if let Some(runtime_id) = coords.runtime_id.as_deref() {
                runtime_for(child, runtime_id)
                    .documents
                    .push(source.clone());
            }
            if let Some(branch_id) = coords.branch_id.as_deref() {
                merge_branch(child, branch_id, coords, &source)
                    .sources
                    .push(source.clone());
            }
            if prepared.source.class == EvidenceClass::Evaluation {
                if let Some(evaluation) = prepared.evaluation.as_ref() {
                    child.evaluations.push(EvaluationEvidence {
                        branch_id: evaluation.branch_id.clone(),
                        evaluation_procedure_id: evaluation.evaluation_procedure_id.clone(),
                        evaluator_identity: evaluation.evaluator_identity.clone(),
                        eval_set_identity: evaluation.eval_set_identity.clone(),
                        evaluation_artifact_path: evaluation.evaluation_artifact_path.clone(),
                        overall_disposition: evaluation.overall_disposition.clone(),
                        compared: evaluation.compared.clone(),
                        source: source.clone(),
                    });
                }
            }
        }
    }

    fn attach_journal(&mut self, journal: &[Stored<JournalEntry>]) {
        for stored in journal {
            let source = EvidenceSource::journal(stored.pointer(), stored.item());
            let coords = journal_coordinates(stored.item());
            let node_id = match self.node_for(&coords) {
                Placement::Node(node_id) => node_id,
                placement => {
                    if is_child_related(stored.item()) {
                        self.unplaced.push(source.clone());
                        self.push_placement_diagnostic(
                            placement,
                            &source,
                            &format!("{} journal source", source.kind),
                        );
                    }
                    continue;
                }
            };
            let child = self.merge_child(&node_id, &coords, &source);
            child.journal.push(source.clone());
            if let Some(runtime_id) = coords.runtime_id.as_deref() {
                runtime_for(child, runtime_id).journal.push(source.clone());
            }
            if let Some(branch_id) = coords.branch_id.as_deref() {
                merge_branch(child, branch_id, &coords, &source)
                    .sources
                    .push(source);
            }
        }
    }

    fn push_placement_diagnostic(
        &mut self,
        placement: Placement,
        source: &EvidenceSource,
        source_label: &str,
    ) {
        match placement {
            Placement::Ambiguous { label, key } => self.diagnostics.push(EvidenceDiagnostic {
                severity: "warning".to_string(),
                source_ref: Some(source.pointer.ref_id().to_string()),
                message: format!("{source_label} refused ambiguous {label} join for '{key}'"),
            }),
            Placement::Missing => self.diagnostics.push(EvidenceDiagnostic {
                severity: "info".to_string(),
                source_ref: Some(source.pointer.ref_id().to_string()),
                message: format!("{source_label} could not be joined to a child node"),
            }),
            Placement::Node(_) => {}
        }
    }

    fn node_for(&self, coords: &Coordinates) -> Placement {
        if let Some(node_id) = coords.node_id.as_ref() {
            return Placement::Node(node_id.clone());
        }
        if let Some(runtime_id) = coords.runtime_id.as_deref() {
            if self.conflicted_runtimes.contains(runtime_id) {
                return Placement::Ambiguous {
                    label: "runtime",
                    key: runtime_id.to_string(),
                };
            }
            if let Some(node_id) = self.runtime_to_node.get(runtime_id) {
                return Placement::Node(node_id.clone());
            }
        }
        if let Some(branch_id) = coords.branch_id.as_deref() {
            if self.conflicted_branches.contains(branch_id) {
                return Placement::Ambiguous {
                    label: "branch",
                    key: branch_id.to_string(),
                };
            }
            if let Some(node_id) = self.branch_to_node.get(branch_id) {
                return Placement::Node(node_id.clone());
            }
        }
        Placement::Missing
    }

    fn merge_child(
        &mut self,
        node_id: &str,
        coords: &Coordinates,
        source: &EvidenceSource,
    ) -> &mut ChildEvidence {
        let child = self
            .children
            .entry(node_id.to_string())
            .or_insert_with(|| ChildEvidence {
                node_id: node_id.to_string(),
                parent_node_id: None,
                generation: None,
                branch_id: None,
                runtimes: Vec::new(),
                branches: Vec::new(),
                evaluations: Vec::new(),
                documents: Vec::new(),
                journal: Vec::new(),
                diagnostics: Vec::new(),
            });
        merge_option(
            &mut child.parent_node_id,
            coords.parent_node_id.clone(),
            "parent_node_id",
            source,
            &mut child.diagnostics,
        );
        merge_option(
            &mut child.generation,
            coords.generation,
            "generation",
            source,
            &mut child.diagnostics,
        );
        merge_option(
            &mut child.branch_id,
            coords.branch_id.clone(),
            "branch_id",
            source,
            &mut child.diagnostics,
        );
        child
    }

    fn finish(self) -> ChildEvidenceSet {
        let mut children = self.children.into_values().collect::<Vec<_>>();
        for child in &mut children {
            child
                .runtimes
                .sort_by(|left, right| left.runtime_id.cmp(&right.runtime_id));
            child
                .branches
                .sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
            child.evaluations.sort_by(|left, right| {
                left.branch_id.cmp(&right.branch_id).then_with(|| {
                    left.source
                        .pointer
                        .ref_id()
                        .cmp(right.source.pointer.ref_id())
                })
            });
        }
        ChildEvidenceSet {
            schema_version: SCHEMA_VERSION.to_string(),
            children,
            unplaced: self.unplaced,
            diagnostics: self.diagnostics,
        }
    }
}

fn runtime_for<'a>(child: &'a mut ChildEvidence, runtime_id: &str) -> &'a mut RuntimeEvidence {
    if let Some(index) = child
        .runtimes
        .iter()
        .position(|runtime| runtime.runtime_id == runtime_id)
    {
        return &mut child.runtimes[index];
    }
    child.runtimes.push(RuntimeEvidence {
        runtime_id: runtime_id.to_string(),
        documents: Vec::new(),
        journal: Vec::new(),
    });
    child.runtimes.last_mut().expect("runtime just pushed")
}

fn merge_branch<'a>(
    child: &'a mut ChildEvidence,
    branch_id: &str,
    coords: &Coordinates,
    source: &EvidenceSource,
) -> &'a mut BranchEvidence {
    if let Some(index) = child
        .branches
        .iter()
        .position(|branch| branch.branch_id == branch_id)
    {
        let branch = &mut child.branches[index];
        merge_option(
            &mut branch.candidate_id,
            coords.candidate_id.clone(),
            "branch candidate_id",
            source,
            &mut child.diagnostics,
        );
        merge_option(
            &mut branch.source_state_id,
            coords.source_state_id.clone(),
            "branch source_state_id",
            source,
            &mut child.diagnostics,
        );
        merge_path_option(
            &mut branch.target_relpath,
            coords.target_relpath.clone(),
            "branch target_relpath",
            source,
            &mut child.diagnostics,
        );
        return branch;
    }
    child.branches.push(BranchEvidence {
        branch_id: branch_id.to_string(),
        candidate_id: coords.candidate_id.clone(),
        source_state_id: coords.source_state_id.clone(),
        target_relpath: coords.target_relpath.clone(),
        sources: Vec::new(),
    });
    child.branches.last_mut().expect("branch just pushed")
}

fn merge_option<T>(
    slot: &mut Option<T>,
    value: Option<T>,
    label: &str,
    source: &EvidenceSource,
    diagnostics: &mut Vec<EvidenceDiagnostic>,
) where
    T: Clone + PartialEq + std::fmt::Display,
{
    let Some(value) = value else {
        return;
    };
    match slot {
        Some(existing) if existing != &value => diagnostics.push(EvidenceDiagnostic {
            severity: "warning".to_string(),
            source_ref: Some(source.pointer.ref_id().to_string()),
            message: format!("conflicting {label}: '{existing}' vs '{value}'"),
        }),
        Some(_) => {}
        None => *slot = Some(value),
    }
}

fn merge_path_option(
    slot: &mut Option<PathBuf>,
    value: Option<PathBuf>,
    label: &str,
    source: &EvidenceSource,
    diagnostics: &mut Vec<EvidenceDiagnostic>,
) {
    let Some(value) = value else {
        return;
    };
    match slot {
        Some(existing) if existing != &value => diagnostics.push(EvidenceDiagnostic {
            severity: "warning".to_string(),
            source_ref: Some(source.pointer.ref_id().to_string()),
            message: format!(
                "conflicting {label}: '{}' vs '{}'",
                existing.display(),
                value.display()
            ),
        }),
        Some(_) => {}
        None => *slot = Some(value),
    }
}

fn require_text(
    value: Option<&str>,
    field: SelectionProjectionField,
    message: &str,
    diagnostics: &mut Vec<SelectionProjectionDiagnostic>,
) -> Option<String> {
    value
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            diagnostics.push(SelectionProjectionDiagnostic::missing(field, message));
            None
        })
}

fn append_conflict_diagnostics(
    child: &ChildEvidence,
    diagnostics: &mut Vec<SelectionProjectionDiagnostic>,
) {
    for diagnostic in &child.diagnostics {
        if diagnostic.severity != "warning" || !diagnostic.message.contains("conflicting ") {
            continue;
        }
        let field = if diagnostic.message.contains("parent_node_id") {
            SelectionProjectionField::Parent
        } else if diagnostic.message.contains("generation") {
            SelectionProjectionField::Generation
        } else if diagnostic.message.contains("branch candidate_id")
            || diagnostic.message.contains("branch source_state_id")
            || diagnostic.message.contains("branch target_relpath")
        {
            SelectionProjectionField::BranchMetadata
        } else if diagnostic.message.contains("branch_id") {
            SelectionProjectionField::Branch
        } else {
            continue;
        };
        let source = diagnostic
            .source_ref
            .as_deref()
            .map(|source_ref| format!(" at {source_ref}"))
            .unwrap_or_default();
        diagnostics.push(SelectionProjectionDiagnostic::invalid(
            field,
            format!(
                "selection projection refused conflicted child evidence{source}: {}",
                diagnostic.message
            ),
        ));
    }
}

fn parse_disposition(value: &str) -> Option<BranchDisposition> {
    match value {
        "keep" | "Keep" => Some(BranchDisposition::Keep),
        "reject" | "Reject" => Some(BranchDisposition::Reject),
        _ => None,
    }
}

fn project_comparisons(
    evaluation: &EvaluationEvidence,
    diagnostics: &mut Vec<SelectionProjectionDiagnostic>,
) -> Vec<RunComparison> {
    if evaluation.compared.is_empty() {
        diagnostics.push(SelectionProjectionDiagnostic::missing(
            SelectionProjectionField::Comparisons,
            "evaluation evidence has no compared instances",
        ));
        return Vec::new();
    }

    let mut comparisons = Vec::new();
    for compared in &evaluation.compared {
        let Some(instance_id) = compared
            .instance_id
            .as_deref()
            .filter(|instance_id| !instance_id.is_empty())
        else {
            diagnostics.push(SelectionProjectionDiagnostic::missing(
                SelectionProjectionField::Comparisons,
                "compared instance has no instance_id",
            ));
            continue;
        };
        let Some(status) = compared
            .status
            .as_deref()
            .filter(|status| !status.is_empty())
        else {
            diagnostics.push(SelectionProjectionDiagnostic::missing(
                SelectionProjectionField::Comparisons,
                format!("compared instance '{instance_id}' has no status"),
            ));
            continue;
        };

        comparisons.push(RunComparison {
            instance_id: instance_id.to_string(),
            parent_metrics: compared.baseline_metrics.clone(),
            child_metrics: compared.treatment_metrics.clone(),
            oracle_evaluation: compared.oracle_evaluation.clone(),
            status: status.to_string(),
        });
    }

    if comparisons.is_empty() {
        diagnostics.push(SelectionProjectionDiagnostic::missing(
            SelectionProjectionField::Comparisons,
            "evaluation evidence has no usable compared instances",
        ));
    }

    comparisons
}

fn prepare_records(records: &ChildEvidenceRecords) -> Vec<PreparedRecord> {
    let mut prepared = Vec::new();

    prepared.extend(records.nodes.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = node_coordinates(stored.item(), &mut facts);
        PreparedRecord {
            source: EvidenceSource::record(stored, EvidenceClass::NodeRecord.as_str(), facts),
            coords,
            evaluation: None,
        }
    }));
    prepared.extend(records.runner_requests.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = request_coordinates(stored.item(), &mut facts);
        PreparedRecord {
            source: EvidenceSource::record(stored, EvidenceClass::RunnerRequest.as_str(), facts),
            coords,
            evaluation: None,
        }
    }));
    prepared.extend(records.runner_results.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = result_coordinates(stored.item(), &mut facts);
        PreparedRecord {
            source: EvidenceSource::record(stored, EvidenceClass::RunnerResult.as_str(), facts),
            coords,
            evaluation: None,
        }
    }));
    prepared.extend(records.invocations.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = invocation_coordinates(stored.item(), &mut facts);
        PreparedRecord {
            source: EvidenceSource::record(stored, EvidenceClass::Invocation.as_str(), facts),
            coords,
            evaluation: None,
        }
    }));
    prepared.extend(records.attempt_results.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = result_coordinates(stored.item(), &mut facts);
        PreparedRecord {
            source: EvidenceSource::record(stored, EvidenceClass::AttemptResult.as_str(), facts),
            coords,
            evaluation: None,
        }
    }));
    prepared.extend(records.successor_ready.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = successor_ready_coordinates(stored.item(), &mut facts);
        PreparedRecord {
            source: EvidenceSource::record(stored, EvidenceClass::SuccessorReady.as_str(), facts),
            coords,
            evaluation: None,
        }
    }));
    prepared.extend(records.successor_completion.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = successor_completion_coordinates(stored.item(), &mut facts);
        PreparedRecord {
            source: EvidenceSource::record(
                stored,
                EvidenceClass::SuccessorCompletion.as_str(),
                facts,
            ),
            coords,
            evaluation: None,
        }
    }));
    prepared.extend(records.evaluations.iter().map(|stored| {
        let mut facts = Vec::new();
        let coords = evaluation_coordinates(stored.item(), &mut facts);
        let evaluation = Some(typed_evaluation_parts(stored.item(), &mut facts));
        PreparedRecord {
            source: EvidenceSource::record(stored, EvidenceClass::Evaluation.as_str(), facts),
            coords,
            evaluation,
        }
    }));

    prepared
}

fn node_coordinates(node: &Prototype1NodeRecord, facts: &mut Vec<EvidenceFact>) -> Coordinates {
    push_fact(facts, "node_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "generation", EvidenceFactOrigin::Typed);
    push_fact(facts, "branch_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "candidate_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "source_state_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "target_relpath", EvidenceFactOrigin::Typed);
    if node.parent_node_id.is_some() {
        push_fact(facts, "parent_node_id", EvidenceFactOrigin::Typed);
    }
    Coordinates {
        node_id: nonempty(node.node_id.as_str()).map(ToOwned::to_owned),
        parent_node_id: node
            .parent_node_id
            .as_deref()
            .and_then(nonempty)
            .map(ToOwned::to_owned),
        generation: Some(node.generation),
        branch_id: nonempty(node.branch_id.as_str()).map(ToOwned::to_owned),
        candidate_id: nonempty(node.candidate_id.as_str()).map(ToOwned::to_owned),
        source_state_id: nonempty(node.source_state_id.as_str()).map(ToOwned::to_owned),
        target_relpath: nonempty_path(&node.target_relpath),
        ..Coordinates::default()
    }
}

fn evaluation_coordinates(
    report: &Prototype1BranchEvaluationReport,
    facts: &mut Vec<EvidenceFact>,
) -> Coordinates {
    if nonempty(report.branch_id.as_str()).is_some() {
        push_fact(facts, "branch_id", EvidenceFactOrigin::Typed);
    }
    Coordinates {
        branch_id: nonempty(report.branch_id.as_str()).map(ToOwned::to_owned),
        ..Coordinates::default()
    }
}

fn request_coordinates(
    request: &Prototype1RunnerRequest,
    facts: &mut Vec<EvidenceFact>,
) -> Coordinates {
    push_fact(facts, "node_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "generation", EvidenceFactOrigin::Typed);
    push_fact(facts, "branch_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "source_state_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "target_relpath", EvidenceFactOrigin::Typed);
    Coordinates {
        node_id: nonempty(request.node_id.as_str()).map(ToOwned::to_owned),
        generation: Some(request.generation),
        branch_id: nonempty(request.branch_id.as_str()).map(ToOwned::to_owned),
        source_state_id: nonempty(request.source_state_id.as_str()).map(ToOwned::to_owned),
        target_relpath: nonempty_path(&request.target_relpath),
        ..Coordinates::default()
    }
}

fn result_coordinates(
    result: &Prototype1RunnerResult,
    facts: &mut Vec<EvidenceFact>,
) -> Coordinates {
    push_fact(facts, "node_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "generation", EvidenceFactOrigin::Typed);
    push_fact(facts, "branch_id", EvidenceFactOrigin::Typed);
    Coordinates {
        node_id: nonempty(result.node_id.as_str()).map(ToOwned::to_owned),
        generation: Some(result.generation),
        branch_id: nonempty(result.branch_id.as_str()).map(ToOwned::to_owned),
        ..Coordinates::default()
    }
}

fn invocation_coordinates(invocation: &Invocation, facts: &mut Vec<EvidenceFact>) -> Coordinates {
    push_fact(facts, "node_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "runtime_id", EvidenceFactOrigin::Typed);
    Coordinates {
        node_id: nonempty(invocation.node_id.as_str()).map(ToOwned::to_owned),
        runtime_id: Some(invocation.runtime_id.to_string()),
        ..Coordinates::default()
    }
}

fn successor_ready_coordinates(
    record: &SuccessorReadyRecord,
    facts: &mut Vec<EvidenceFact>,
) -> Coordinates {
    push_fact(facts, "node_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "runtime_id", EvidenceFactOrigin::Typed);
    Coordinates {
        node_id: nonempty(record.node_id.as_str()).map(ToOwned::to_owned),
        runtime_id: Some(record.runtime_id.to_string()),
        ..Coordinates::default()
    }
}

fn successor_completion_coordinates(
    record: &SuccessorCompletionRecord,
    facts: &mut Vec<EvidenceFact>,
) -> Coordinates {
    push_fact(facts, "node_id", EvidenceFactOrigin::Typed);
    push_fact(facts, "runtime_id", EvidenceFactOrigin::Typed);
    Coordinates {
        node_id: nonempty(record.node_id.as_str()).map(ToOwned::to_owned),
        runtime_id: Some(record.runtime_id.to_string()),
        ..Coordinates::default()
    }
}

fn typed_evaluation_parts(
    report: &Prototype1BranchEvaluationReport,
    facts: &mut Vec<EvidenceFact>,
) -> EvaluationEvidenceParts {
    push_fact(facts, "evaluation.branch_id", EvidenceFactOrigin::Typed);
    push_fact(
        facts,
        "evaluation.evaluation_artifact_path",
        EvidenceFactOrigin::Typed,
    );
    push_fact(
        facts,
        "evaluation.overall_disposition",
        EvidenceFactOrigin::Typed,
    );
    push_fact(
        facts,
        "evaluation.compared_instances",
        EvidenceFactOrigin::Typed,
    );
    if report.evaluation_procedure_id.is_some() {
        push_fact(
            facts,
            "evaluation.evaluation_procedure_id",
            EvidenceFactOrigin::Typed,
        );
    }
    if report.evaluator_identity.is_some() {
        push_fact(
            facts,
            "evaluation.evaluator_identity",
            EvidenceFactOrigin::Typed,
        );
    }
    if report.eval_set_identity.is_some() {
        push_fact(
            facts,
            "evaluation.eval_set_identity",
            EvidenceFactOrigin::Typed,
        );
    }
    EvaluationEvidenceParts {
        branch_id: report.branch_id.clone(),
        evaluation_procedure_id: report.evaluation_procedure_id.clone(),
        evaluator_identity: report.evaluator_identity.clone(),
        eval_set_identity: report.eval_set_identity.clone(),
        evaluation_artifact_path: nonempty_path(&report.evaluation_artifact_path),
        overall_disposition: Some(disposition_text(&report.overall_disposition).to_string()),
        compared: report
            .compared_instances
            .iter()
            .map(|instance| {
                push_fact(
                    facts,
                    "evaluation.compared_instances[].instance_id",
                    EvidenceFactOrigin::Typed,
                );
                push_fact(
                    facts,
                    "evaluation.compared_instances[].status",
                    EvidenceFactOrigin::Typed,
                );
                if instance.baseline_record_path.is_some() {
                    push_fact(
                        facts,
                        "evaluation.compared_instances[].baseline_record_path",
                        EvidenceFactOrigin::Typed,
                    );
                }
                if instance.baseline_registration_path.is_some() {
                    push_fact(
                        facts,
                        "evaluation.compared_instances[].baseline_registration_path",
                        EvidenceFactOrigin::Typed,
                    );
                }
                if instance.treatment_record_path.is_some() {
                    push_fact(
                        facts,
                        "evaluation.compared_instances[].treatment_record_path",
                        EvidenceFactOrigin::Typed,
                    );
                }
                if instance.treatment_registration_path.is_some() {
                    push_fact(
                        facts,
                        "evaluation.compared_instances[].treatment_registration_path",
                        EvidenceFactOrigin::Typed,
                    );
                }
                if instance.baseline_metrics.is_some() {
                    push_fact(
                        facts,
                        "evaluation.compared_instances[].baseline_metrics",
                        EvidenceFactOrigin::Typed,
                    );
                }
                if instance.treatment_metrics.is_some() {
                    push_fact(
                        facts,
                        "evaluation.compared_instances[].treatment_metrics",
                        EvidenceFactOrigin::Typed,
                    );
                }
                let mut diagnostics = Vec::new();
                let baseline_run = load_run_evidence(
                    "baseline",
                    instance.baseline_registration_path.as_deref(),
                    instance.baseline_record_path.as_deref(),
                    &mut diagnostics,
                );
                let treatment_run = load_run_evidence(
                    "treatment",
                    instance.treatment_registration_path.as_deref(),
                    instance.treatment_record_path.as_deref(),
                    &mut diagnostics,
                );
                ComparedRunEvidence {
                    instance_id: nonempty(instance.instance_id.as_str()).map(ToOwned::to_owned),
                    baseline_registration_path: instance.baseline_registration_path.clone(),
                    treatment_registration_path: instance.treatment_registration_path.clone(),
                    baseline_record_path: instance.baseline_record_path.clone(),
                    treatment_record_path: instance.treatment_record_path.clone(),
                    baseline_run,
                    treatment_run,
                    baseline_metrics: instance.baseline_metrics.clone(),
                    treatment_metrics: instance.treatment_metrics.clone(),
                    oracle_evaluation: instance.oracle_evaluation.clone(),
                    status: nonempty(instance.status.as_str()).map(ToOwned::to_owned),
                    diagnostics,
                }
            })
            .collect(),
    }
}

fn load_run_evidence(
    arm: &'static str,
    registration_path: Option<&Path>,
    record_path: Option<&Path>,
    diagnostics: &mut Vec<ComparedRunDiagnostic>,
) -> Option<RunEvidence> {
    if let Some(path) = registration_path {
        return match RunRegistration::load(path) {
            Ok(registration) => Some(RunEvidence::from_registration(
                registration,
                Some(path.to_path_buf()),
            )),
            Err(RunRegistrationError::Read { .. }) => {
                diagnostics.push(ComparedRunDiagnostic::missing(
                    format!("{arm}_run_registration"),
                    format!(
                        "declared {arm} run registration is missing at '{}'",
                        path.display()
                    ),
                ));
                None
            }
            Err(error) => {
                diagnostics.push(ComparedRunDiagnostic::invalid(
                    format!("{arm}_run_registration"),
                    format!(
                        "declared {arm} run registration could not be loaded as RunRegistration: {error}"
                    ),
                ));
                None
            }
        };
    }

    let Some(path) = record_path else {
        return None;
    };
    match load_registration_for_record_path(path) {
        Ok(Some(registration)) => Some(RunEvidence::from_registration(registration, None)),
        Ok(None) => None,
        Err(error) => {
            diagnostics.push(ComparedRunDiagnostic::invalid(
                format!("{arm}_run_registration"),
                format!(
                    "could not use {arm} record path as a RunRegistration locator bridge: {error}"
                ),
            ));
            None
        }
    }
}

fn push_fact(facts: &mut Vec<EvidenceFact>, field: &str, origin: EvidenceFactOrigin) {
    if facts
        .iter()
        .any(|fact| fact.field == field && fact.origin == origin)
    {
        return;
    }
    facts.push(EvidenceFact {
        field: field.to_string(),
        origin,
    });
}

fn nonempty(value: &str) -> Option<&str> {
    if value.is_empty() { None } else { Some(value) }
}

fn nonempty_path(path: &Path) -> Option<PathBuf> {
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path.to_path_buf())
    }
}

fn same_protocol_path(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    let normalize =
        |path: &Path| std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    normalize(left) == normalize(right)
}

fn disposition_text(disposition: &BranchDisposition) -> &'static str {
    match disposition {
        BranchDisposition::Keep => "keep",
        BranchDisposition::Reject => "reject",
    }
}

fn journal_coordinates(entry: &JournalEntry) -> Coordinates {
    match entry {
        JournalEntry::ChildArtifactCommitted(entry) => Coordinates {
            node_id: Some(entry.node_id.clone()),
            parent_node_id: entry.child_identity.parent_node_id().map(str::to_string),
            generation: Some(entry.generation),
            branch_id: Some(entry.child_identity.branch_id().to_string()),
            ..Coordinates::default()
        },
        JournalEntry::ActiveCheckoutAdvanced(entry) => Coordinates {
            node_id: Some(entry.selected_parent_identity.node_id().to_string()),
            parent_node_id: entry
                .selected_parent_identity
                .parent_node_id()
                .map(str::to_string),
            generation: Some(entry.selected_parent_identity.generation()),
            branch_id: Some(entry.selected_branch.clone()),
            ..Coordinates::default()
        },
        JournalEntry::SuccessorHandoff(entry) => Coordinates {
            node_id: Some(entry.node_id.clone()),
            runtime_id: Some(entry.runtime_id.to_string()),
            ..Coordinates::default()
        },
        JournalEntry::Successor(entry) => successor_coordinates(entry),
        JournalEntry::MaterializeBranch(entry) => refs_coordinates(
            entry.generation,
            &entry.refs,
            Some(entry.paths.target_relpath.clone()),
            None,
        ),
        JournalEntry::BuildChild(entry) => refs_coordinates(
            entry.generation,
            &entry.refs,
            Some(entry.paths.target_relpath.clone()),
            None,
        ),
        JournalEntry::SpawnChild(entry) => refs_coordinates(
            entry.generation,
            &entry.refs,
            Some(entry.paths.target_relpath.clone()),
            Some(entry.runtime_id.to_string()),
        ),
        JournalEntry::Child(entry) => Coordinates {
            runtime_id: Some(entry.runtime_id().to_string()),
            ..Coordinates::default()
        },
        JournalEntry::ChildReady(entry) => refs_coordinates(
            entry.generation,
            &entry.refs,
            Some(entry.paths.target_relpath.clone()),
            Some(entry.runtime_id.to_string()),
        ),
        JournalEntry::ObserveChild(entry) => refs_coordinates(
            entry.generation,
            &entry.refs,
            Some(entry.paths.target_relpath.clone()),
            Some(entry.runtime_id.to_string()),
        ),
        JournalEntry::ParentStarted(_) | JournalEntry::Resource(_) => Coordinates::default(),
    }
}

fn refs_coordinates(
    generation: u32,
    refs: &super::event::Refs,
    target_relpath: Option<PathBuf>,
    runtime_id: Option<String>,
) -> Coordinates {
    Coordinates {
        node_id: Some(refs.node_id.clone()),
        generation: Some(generation),
        runtime_id,
        branch_id: Some(refs.branch_id.clone()),
        candidate_id: Some(refs.candidate_id.clone()),
        source_state_id: Some(refs.source_state_id.clone()),
        target_relpath,
        ..Coordinates::default()
    }
}

fn successor_coordinates(entry: &super::successor::Record) -> Coordinates {
    let branch_id = match &entry.state {
        super::successor::State::Selected { decision, .. } => {
            decision.selected_next_branch_id.clone()
        }
        super::successor::State::Checkout {
            selected_branch, ..
        } => Some(selected_branch.clone()),
        _ => None,
    };
    Coordinates {
        node_id: Some(entry.node_id.clone()),
        runtime_id: entry.runtime_id.map(|runtime_id| runtime_id.to_string()),
        branch_id,
        ..Coordinates::default()
    }
}

fn is_child_related(entry: &JournalEntry) -> bool {
    matches!(
        entry,
        JournalEntry::ChildArtifactCommitted(_)
            | JournalEntry::SuccessorHandoff(_)
            | JournalEntry::Successor(_)
            | JournalEntry::MaterializeBranch(_)
            | JournalEntry::BuildChild(_)
            | JournalEntry::SpawnChild(_)
            | JournalEntry::Child(_)
            | JournalEntry::ChildReady(_)
            | JournalEntry::ObserveChild(_)
    )
}

fn journal_kind(entry: &JournalEntry) -> &'static str {
    match entry {
        JournalEntry::ParentStarted(_) => "parent_started",
        JournalEntry::Resource(_) => "resource",
        JournalEntry::ChildArtifactCommitted(_) => "artifact.committed",
        JournalEntry::ActiveCheckoutAdvanced(_) => "checkout.advanced",
        JournalEntry::SuccessorHandoff(_) => "successor.handoff",
        JournalEntry::Successor(entry) => entry.entry_kind(),
        JournalEntry::MaterializeBranch(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "materialize.before",
            crate::intervention::CommitPhase::After => "materialize.after",
        },
        JournalEntry::BuildChild(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "build.before",
            crate::intervention::CommitPhase::After => "build.after",
        },
        JournalEntry::SpawnChild(entry) => match entry.phase {
            SpawnPhase::Starting => "child.starting",
            SpawnPhase::Spawned => "child.spawned",
            SpawnPhase::Observed => "child.observed",
        },
        JournalEntry::Child(entry) => entry.entry_kind(),
        JournalEntry::ChildReady(_) => "child.ready",
        JournalEntry::ObserveChild(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "observe.before",
            crate::intervention::CommitPhase::After => "observe.after",
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{
        ChildEvidence, EvidenceDiagnostic, EvidenceFactOrigin, EvidenceSource,
        SelectionProjectionError, SelectionProjectionField, seal_candidate_evidence_for_history,
    };
    use crate::cli::prototype1_state::evidence_class::EvidenceClass;
    use crate::cli::prototype1_state::history_preview::FsEvidenceStore;
    use crate::record::SubmissionArtifactState;
    use crate::{BranchDisposition, OperationalRunMetrics, PatchApplyState};

    #[test]
    fn groups_child_documents_with_source_refs() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = tmp.path().join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        let node_dir = tmp.path().join("prototype1/nodes/node-a");
        fs::create_dir_all(node_dir.join("results")).expect("node dirs");
        fs::create_dir_all(tmp.path().join("prototype1/evaluations")).expect("eval dir");
        fs::write(
            node_dir.join("node.json"),
            typed_node_value(tmp.path(), "node-a", "branch-a").to_string(),
        )
        .expect("node");
        write_invocation(tmp.path(), "node-a", "11111111-1111-4111-8111-111111111111");
        write_typed_evaluation(tmp.path(), "branch-a", "keep");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");

        assert_eq!(evidence.children.len(), 1);
        let child = &evidence.children[0];
        assert_eq!(child.node_id, "node-a");
        assert_eq!(child.parent_node_id.as_deref(), Some("node-parent"));
        assert_eq!(child.generation, Some(2));
        assert_eq!(child.branch_id.as_deref(), Some("branch-a"));
        assert_eq!(
            child.runtimes[0].runtime_id,
            "11111111-1111-4111-8111-111111111111"
        );
        assert_eq!(child.evaluations[0].compared.len(), 1);
        assert!(child.documents.iter().all(|source| {
            !source.pointer.ref_id().is_empty() && !source.pointer.hash().as_str().is_empty()
        }));
    }

    #[test]
    fn sealed_candidate_evidence_uses_current_schema_version() {
        let sealed = seal_candidate_evidence_for_history("node-a", 0, "done", "completed", None);

        assert_eq!(sealed.schema_version, 2);
    }

    #[test]
    fn typed_node_and_evaluation_decode_record_fact_origins() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        let node_dir = tmp.path().join("prototype1/nodes/node-a");
        fs::create_dir_all(&node_dir).expect("node dir");
        fs::write(
            node_dir.join("node.json"),
            typed_node_value(tmp.path(), "node-a", "branch-a").to_string(),
        )
        .expect("node");
        write_typed_evaluation(tmp.path(), "branch-a", "keep");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");

        assert_eq!(evidence.children.len(), 1);
        let child = &evidence.children[0];
        let node_source = child
            .documents
            .iter()
            .find(|source| source.class == EvidenceClass::NodeRecord)
            .expect("node source");
        assert_eq!(
            node_source
                .record
                .as_ref()
                .map(|record| record.name.as_str()),
            Some("Prototype1NodeRecord"),
        );
        assert!(source_has_fact(
            node_source,
            "node_id",
            EvidenceFactOrigin::Typed
        ));
        assert!(source_has_fact(
            node_source,
            "generation",
            EvidenceFactOrigin::Typed
        ));

        let evaluation_source = &child.evaluations[0].source;
        assert_eq!(
            evaluation_source
                .record
                .as_ref()
                .map(|record| record.name.as_str()),
            Some("Prototype1BranchEvaluationReport"),
        );
        assert!(source_has_fact(
            evaluation_source,
            "evaluation.compared_instances[].baseline_record_path",
            EvidenceFactOrigin::Typed
        ));
        assert!(!diagnostics_contain(
            &evidence.diagnostics,
            "typed decode as"
        ));
    }

    #[test]
    fn malformed_declared_node_record_fails_typed_boundary() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        let node_dir = tmp.path().join("prototype1/nodes/node-a");
        fs::create_dir_all(&node_dir).expect("node dir");
        fs::write(
            node_dir.join("node.json"),
            serde_json::json!({
                "node_id": "node-a",
                "generation": 2,
                "branch_id": "branch-a"
            })
            .to_string(),
        )
        .expect("node");

        let error = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect_err("typed boundary should fail");

        assert!(error.to_string().contains("typed evidence boundary error"));
        assert!(error.to_string().contains("Prototype1NodeRecord"));
    }

    #[test]
    fn projects_selection_input_from_child_evidence() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        let parent_metrics = metrics(false, false, 3);
        let child_metrics = metrics(true, true, 1);

        write_node(
            tmp.path(),
            "node-a",
            serde_json::json!({
                "node_id": "node-a",
                "parent_node_id": "node-parent",
                "generation": 2,
                "branch_id": "branch-a"
            }),
        );
        fs::create_dir_all(tmp.path().join("prototype1/evaluations")).expect("eval dir");
        fs::write(
            tmp.path().join("prototype1/evaluations/branch-a.json"),
            serde_json::json!({
                "baseline_campaign_id": "baseline-campaign",
                "branch_id": "branch-a",
                "treatment_campaign_id": "treatment-campaign",
                "branch_registry_path": tmp.path().join("prototype1/branches.json"),
                "evaluation_artifact_path": tmp.path().join("prototype1/evaluations/branch-a.json"),
                "treatment_campaign_manifest": tmp.path().join("campaign.json"),
                "treatment_closure_state_path": tmp.path().join("closure-state.json"),
                "overall_disposition": "keep",
                "reasons": ["test"],
                "compared_instances": [{
                    "instance_id": "instance-a",
                    "baseline_record_path": "/tmp/baseline/record.json.gz",
                    "treatment_record_path": "/tmp/treatment/record.json.gz",
                    "baseline_metrics": parent_metrics,
                    "treatment_metrics": child_metrics,
                    "evaluation": null,
                    "status": "compared"
                }]
            })
            .to_string(),
        )
        .expect("evaluation");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");
        let projection = evidence.selection_inputs();

        assert!(projection.failures.is_empty());
        assert_eq!(projection.inputs.len(), 1);
        let input = &projection.inputs[0];
        assert_eq!(input.candidate.node_id, "node-a");
        assert_eq!(input.candidate.branch_id, "branch-a");
        assert_eq!(input.candidate.generation, 2);
        assert_eq!(input.branch_disposition, BranchDisposition::Keep);
        assert_eq!(input.comparisons.len(), 1);
        assert_eq!(input.comparisons[0].instance_id, "instance-a");
        assert_eq!(input.comparisons[0].parent_metrics, Some(parent_metrics));
        assert_eq!(input.comparisons[0].child_metrics, Some(child_metrics));

        let compared = &evidence.children[0].evaluations[0].compared[0];
        assert_eq!(
            compared.baseline_record_path.as_deref(),
            Some(Path::new("/tmp/baseline/record.json.gz"))
        );
        assert_eq!(
            compared.treatment_record_path.as_deref(),
            Some(Path::new("/tmp/treatment/record.json.gz"))
        );
    }

    #[test]
    fn selection_projection_reports_missing_required_evidence() {
        let child = ChildEvidence {
            node_id: String::new(),
            parent_node_id: None,
            generation: None,
            branch_id: None,
            runtimes: Vec::new(),
            branches: Vec::new(),
            evaluations: Vec::new(),
            documents: Vec::new(),
            journal: Vec::new(),
            diagnostics: Vec::new(),
        };

        let error = child.selection_input().expect_err("missing projection");

        assert!(projection_error_contains(
            &error,
            SelectionProjectionField::Node
        ));
        assert!(projection_error_contains(
            &error,
            SelectionProjectionField::Branch
        ));
        assert!(projection_error_contains(
            &error,
            SelectionProjectionField::Generation
        ));
        assert!(projection_error_contains(
            &error,
            SelectionProjectionField::Disposition
        ));
        assert!(projection_error_contains(
            &error,
            SelectionProjectionField::EvaluationArtifact
        ));
        assert!(projection_error_contains(
            &error,
            SelectionProjectionField::Comparisons
        ));
    }

    #[test]
    fn selection_projection_rejects_evaluation_without_comparisons() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());

        write_node(
            tmp.path(),
            "node-a",
            serde_json::json!({
                "node_id": "node-a",
                "generation": 2,
                "branch_id": "branch-a"
            }),
        );
        fs::create_dir_all(tmp.path().join("prototype1/evaluations")).expect("eval dir");
        fs::write(
            tmp.path().join("prototype1/evaluations/branch-a.json"),
            serde_json::json!({
                "baseline_campaign_id": "baseline-campaign",
                "branch_id": "branch-a",
                "treatment_campaign_id": "treatment-campaign",
                "branch_registry_path": tmp.path().join("prototype1/branches.json"),
                "evaluation_artifact_path": tmp.path().join("prototype1/evaluations/branch-a.json"),
                "treatment_campaign_manifest": tmp.path().join("campaign.json"),
                "treatment_closure_state_path": tmp.path().join("closure-state.json"),
                "overall_disposition": "keep",
                "reasons": ["test"],
                "compared_instances": []
            })
            .to_string(),
        )
        .expect("evaluation");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");
        let projection = evidence.selection_inputs();

        assert!(projection.inputs.is_empty());
        assert_eq!(projection.failures.len(), 1);
        assert!(projection_error_contains(
            &projection.failures[0],
            SelectionProjectionField::Comparisons
        ));
    }

    #[test]
    fn selection_projection_rejects_conflicted_identity_fields() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());

        write_node(
            tmp.path(),
            "node-a",
            serde_json::json!({
                "node_id": "node-a",
                "parent_node_id": "parent-a",
                "generation": 2,
                "branch_id": "branch-a"
            }),
        );
        write_runner_result(tmp.path(), "node-a", 3, "branch-b");
        write_evaluation(tmp.path(), "branch-a", "branch-a", "keep");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");
        let projection = evidence.selection_inputs();

        assert!(projection.inputs.is_empty());
        assert_eq!(projection.failures.len(), 1);
        assert!(projection_error_contains(
            &projection.failures[0],
            SelectionProjectionField::Generation
        ));
        assert!(projection_error_contains(
            &projection.failures[0],
            SelectionProjectionField::Branch
        ));
    }

    #[test]
    fn selection_projection_rejects_duplicate_branch_evaluations() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());

        write_node(
            tmp.path(),
            "node-a",
            serde_json::json!({
                "node_id": "node-a",
                "generation": 2,
                "branch_id": "branch-a"
            }),
        );
        write_evaluation(tmp.path(), "branch-a-first", "branch-a", "keep");
        write_evaluation(tmp.path(), "branch-a-second", "branch-a", "reject");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");
        let projection = evidence.selection_inputs();

        assert!(projection.inputs.is_empty());
        assert_eq!(projection.failures.len(), 1);
        assert!(projection_error_contains(
            &projection.failures[0],
            SelectionProjectionField::Evaluation
        ));
        assert!(projection.failures[0].diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("2 evaluation records for branch_id 'branch-a'")
        }));
    }

    #[test]
    fn ambiguous_runtime_and_branch_joins_are_not_reused() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        let runtime_id = "11111111-1111-4111-8111-111111111111";

        write_node(
            tmp.path(),
            "node-a",
            serde_json::json!({
                "node_id": "node-a",
                "generation": 1,
                "runtime_id": runtime_id,
                "branch_id": "branch-shared"
            }),
        );
        write_node(
            tmp.path(),
            "node-b",
            serde_json::json!({
                "node_id": "node-b",
                "generation": 1,
                "runtime_id": runtime_id,
                "branch_id": "branch-shared"
            }),
        );
        write_invocation(tmp.path(), "node-a", runtime_id);
        write_invocation(tmp.path(), "node-b", runtime_id);
        fs::create_dir_all(tmp.path().join("prototype1/evaluations")).expect("eval dir");
        write_typed_evaluation(tmp.path(), "branch-shared", "keep");
        write_child_journal(tmp.path(), runtime_id);

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");

        assert_eq!(evidence.children.len(), 2);
        assert_eq!(evidence.unplaced.len(), 2);
        assert!(diagnostics_contain(
            &evidence.diagnostics,
            "conflicting runtime join for"
        ));
        assert!(diagnostics_contain(
            &evidence.diagnostics,
            "conflicting branch join for"
        ));
        assert!(diagnostics_contain(
            &evidence.diagnostics,
            "refused ambiguous runtime join"
        ));
        assert!(diagnostics_contain(
            &evidence.diagnostics,
            "refused ambiguous branch join"
        ));
    }

    #[test]
    fn branch_metadata_conflicts_are_diagnostic() {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());

        write_node(
            tmp.path(),
            "node-a",
            serde_json::json!({
                "node_id": "node-a",
                "generation": 2,
                "branch_id": "branch-a",
                "candidate_id": "candidate-a",
                "source_state_id": "source-a",
                "target_relpath": "crates/ploke-core/tool_text/first.md"
            }),
        );
        write_runner_request(
            tmp.path(),
            "node-a",
            "branch-a",
            "candidate-b",
            "source-b",
            "crates/ploke-core/tool_text/second.md",
        );
        write_evaluation(tmp.path(), "branch-a", "branch-a", "keep");

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("evidence");

        assert_eq!(evidence.children.len(), 1);
        let child = &evidence.children[0];
        assert!(diagnostics_contain(
            &child.diagnostics,
            "conflicting branch source_state_id"
        ));
        assert!(diagnostics_contain(
            &child.diagnostics,
            "conflicting branch target_relpath"
        ));

        let projection = evidence.selection_inputs();
        assert!(projection.inputs.is_empty());
        assert_eq!(projection.failures.len(), 1);
        assert!(projection_error_contains(
            &projection.failures[0],
            SelectionProjectionField::BranchMetadata
        ));
    }

    fn campaign_manifest(root: &Path) -> std::path::PathBuf {
        let manifest = root.join("campaign.json");
        fs::write(&manifest, "{}").expect("manifest");
        manifest
    }

    fn write_node(root: &Path, node_id: &str, value: serde_json::Value) {
        let node_dir = root.join("prototype1/nodes").join(node_id);
        fs::create_dir_all(&node_dir).expect("node dir");
        let branch_id = value
            .get("branch_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("branch-a");
        let mut typed = typed_node_value(root, node_id, branch_id);
        let typed_object = typed.as_object_mut().expect("typed node object");
        for (key, value) in value.as_object().expect("node overrides") {
            typed_object.insert(key.clone(), value.clone());
        }
        fs::write(node_dir.join("node.json"), typed.to_string()).expect("node");
    }

    fn write_evaluation(root: &Path, file_stem: &str, branch_id: &str, disposition: &str) {
        let eval_dir = root.join("prototype1/evaluations");
        fs::create_dir_all(&eval_dir).expect("eval dir");
        fs::write(
            eval_dir.join(format!("{file_stem}.json")),
            serde_json::json!({
                "baseline_campaign_id": "baseline-campaign",
                "branch_id": branch_id,
                "treatment_campaign_id": "treatment-campaign",
                "branch_registry_path": root.join("prototype1/branches.json"),
                "evaluation_artifact_path": eval_dir.join(format!("{file_stem}.json")),
                "treatment_campaign_manifest": root.join("campaign.json"),
                "treatment_closure_state_path": root.join("closure-state.json"),
                "overall_disposition": disposition,
                "reasons": ["test"],
                "compared_instances": [{
                    "instance_id": "instance-a",
                    "baseline_record_path": "/tmp/baseline/record.json.gz",
                    "treatment_record_path": "/tmp/treatment/record.json.gz",
                    "baseline_metrics": null,
                    "treatment_metrics": null,
                    "evaluation": null,
                    "status": "compared"
                }]
            })
            .to_string(),
        )
        .expect("evaluation");
    }

    fn typed_node_value(root: &Path, node_id: &str, branch_id: &str) -> serde_json::Value {
        let node_dir = root.join("prototype1/nodes").join(node_id);
        serde_json::json!({
            "schema_version": "prototype1-treatment-node.v1",
            "node_id": node_id,
            "parent_node_id": "node-parent",
            "generation": 2,
            "instance_id": "instance-a",
            "source_state_id": "source-a",
            "branch_id": branch_id,
            "candidate_id": "candidate-a",
            "target_relpath": "crates/ploke-core/tool_text/non_semantic_patch.md",
            "node_dir": node_dir,
            "workspace_root": root.join("workspace"),
            "binary_path": root.join("target/debug/ploke-eval"),
            "runner_request_path": node_dir.join("runner-request.json"),
            "runner_result_path": node_dir.join("runner-result.json"),
            "status": "planned",
            "created_at": "2026-05-06T00:00:00Z",
            "updated_at": "2026-05-06T00:00:00Z"
        })
    }

    fn write_typed_evaluation(root: &Path, branch_id: &str, disposition: &str) {
        let eval_dir = root.join("prototype1/evaluations");
        fs::create_dir_all(&eval_dir).expect("eval dir");
        fs::write(
            eval_dir.join(format!("{branch_id}.json")),
            serde_json::json!({
                    "baseline_campaign_id": "baseline-campaign",
                    "branch_id": branch_id,
                    "treatment_campaign_id": "treatment-campaign",
                    "branch_registry_path": root.join("prototype1/branches.json"),
                    "evaluation_artifact_path": eval_dir.join(format!("{branch_id}.json")),
                    "treatment_campaign_manifest": root.join("campaign.json"),
                    "treatment_closure_state_path": root.join("closure-state.json"),
                    "overall_disposition": disposition,
                    "reasons": ["test"],
                    "compared_instances": [{
                        "instance_id": "instance-a",
                    "baseline_record_path": "/tmp/baseline/record.json.gz",
                    "treatment_record_path": "/tmp/treatment/record.json.gz",
                    "baseline_metrics": null,
                    "treatment_metrics": null,
                    "evaluation": null,
                    "status": "compared"
                }]
            })
            .to_string(),
        )
        .expect("evaluation");
    }

    fn write_runner_request(
        root: &Path,
        node_id: &str,
        branch_id: &str,
        candidate_id: &str,
        source_state_id: &str,
        target_relpath: &str,
    ) {
        let node_dir = root.join("prototype1/nodes").join(node_id);
        fs::write(
            node_dir.join("runner-request.json"),
            serde_json::json!({
                "schema_version": "prototype1-treatment-node.v1",
                "campaign_id": "campaign",
                "node_id": node_id,
                "generation": 2,
                "instance_id": "instance-a",
                "source_state_id": source_state_id,
                "branch_id": branch_id,
                "candidate_id": candidate_id,
                "target_relpath": target_relpath,
                "workspace_root": root.join("workspace"),
                "binary_path": root.join("target/debug/ploke-eval"),
                "stop_on_error": false,
                "runner_args": []
            })
            .to_string(),
        )
        .expect("runner request");
    }

    fn write_runner_result(root: &Path, node_id: &str, generation: u32, branch_id: &str) {
        let node_dir = root.join("prototype1/nodes").join(node_id);
        fs::write(
            node_dir.join("runner-result.json"),
            serde_json::json!({
                "schema_version": "prototype1-treatment-node.v1",
                "campaign_id": "campaign",
                "node_id": node_id,
                "generation": generation,
                "branch_id": branch_id,
                "status": "succeeded",
                "disposition": "succeeded",
                "treatment_campaign_id": "treatment-campaign",
                "evaluation_artifact_path": root.join("prototype1/evaluations").join(format!("{branch_id}.json")),
                "recorded_at": "2026-05-06T00:00:00Z"
            })
            .to_string(),
        )
        .expect("runner result");
    }

    fn write_invocation(root: &Path, node_id: &str, runtime_id: &str) {
        let invocation_dir = root
            .join("prototype1/nodes")
            .join(node_id)
            .join("invocations");
        fs::create_dir_all(&invocation_dir).expect("invocation dir");
        fs::write(
            invocation_dir.join(format!("{runtime_id}.json")),
            serde_json::json!({
                "schema_version": "prototype1-invocation.v1",
                "role": "child",
                "campaign_id": "campaign",
                "node_id": node_id,
                "runtime_id": runtime_id,
                "journal_path": root.join("prototype1/transition-journal.jsonl"),
                "channel_root": root.join("prototype1/nodes").join(node_id).join("channels").join(runtime_id),
                "created_at": "2026-05-06T00:00:00Z"
            })
            .to_string(),
        )
        .expect("invocation");
    }

    fn write_child_journal(root: &Path, runtime_id: &str) {
        let journal_path = root.join("prototype1/transition-journal.jsonl");
        fs::write(
            journal_path,
            serde_json::json!({
                "kind": "child",
                "runtime_id": runtime_id,
                "recorded_at": 0,
                "generation": 1,
                "refs": {
                    "campaign_id": "campaign",
                    "node_id": "ignored-node",
                    "instance_id": "instance",
                    "source_state_id": "source",
                    "branch_id": "branch",
                    "candidate_id": "candidate",
                    "branch_label": "label",
                    "spec_id": "spec"
                },
                "paths": {
                    "repo_root": "/tmp/repo",
                    "workspace_root": "/tmp/workspace",
                    "binary_path": "/tmp/bin/ploke",
                    "target_relpath": "crates/ploke-core/tool_text/non_semantic_patch.md",
                    "absolute_path": "/tmp/workspace/crates/ploke-core/tool_text/non_semantic_patch.md"
                },
                "pid": 123,
                "state": "ready"
            })
            .to_string()
                + "\n",
        )
        .expect("journal");
    }

    fn source_has_fact(source: &EvidenceSource, field: &str, origin: EvidenceFactOrigin) -> bool {
        source
            .facts
            .iter()
            .any(|fact| fact.field == field && fact.origin == origin)
    }

    fn diagnostics_contain(diagnostics: &[EvidenceDiagnostic], needle: &str) -> bool {
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(needle))
    }

    fn projection_error_contains(
        error: &SelectionProjectionError,
        field: SelectionProjectionField,
    ) -> bool {
        error
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == field)
    }

    fn metrics(
        oracle_eligible: bool,
        convergence: bool,
        failed_tool_calls: usize,
    ) -> OperationalRunMetrics {
        OperationalRunMetrics {
            tool_calls_total: 5,
            tool_calls_failed: failed_tool_calls,
            patch_attempted: true,
            patch_apply_state: if convergence {
                PatchApplyState::Applied
            } else {
                PatchApplyState::No
            },
            submission_artifact_state: if oracle_eligible {
                SubmissionArtifactState::Nonempty
            } else {
                SubmissionArtifactState::Missing
            },
            patch_projection_check_state: if oracle_eligible {
                ploke_records::evaluation::PatchProjectionCheckState::Passed
            } else {
                ploke_records::evaluation::PatchProjectionCheckState::NotApplicable
            },
            partial_patch_failures: 0,
            same_file_patch_retry_count: 0,
            same_file_patch_max_streak: 0,
            aborted: false,
            aborted_repair_loop: false,
            nonempty_valid_patch: convergence,
            convergence,
            oracle_eligible,
        }
    }
}
