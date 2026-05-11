use std::fs;
use std::path::Path;
use std::sync::MutexGuard;

use super::{
    ChildScore, ScoreDirection, ScoreProfile, ScoreSelectionReview, ScoreSelectionStatus, ScoreSet,
    ScoreState,
};
use crate::BranchDisposition;
use crate::cli::prototype1_state::history_preview::FsEvidenceStore;
use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
use crate::inner::registry::RunRegistration;
use crate::record::SubmissionArtifactState;
use crate::spec::EvalBudget;
use crate::successor_selection::{CandidateRef, RunComparison, SelectionInput};
use crate::{OperationalRunMetrics, PatchApplyState};

#[test]
fn scores_typed_evaluation_metrics_with_provenance_and_identity() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    let parent_metrics = metrics(false, false, 3);
    let child_metrics = metrics(true, true, 1);
    write_node(tmp.path(), "node-a", "branch-a");
    write_evaluation(
        tmp.path(),
        "branch-a",
        "keep",
        parent_metrics,
        child_metrics,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);

    assert_eq!(scores.schema_version, "prototype1-score-projection.v1");
    assert_eq!(scores.children.len(), 1);
    let child = &scores.children[0];
    assert_eq!(child.node_id, "node-a");
    assert_eq!(child.state, ScoreState::Complete);
    assert_eq!(child.score, Some(4));
    assert_eq!(child.comparable_score(), Some(4));

    let evaluation = &child.evaluations[0];
    assert_eq!(evaluation.branch_id, "branch-a");
    assert_eq!(evaluation.state, ScoreState::Complete);
    assert_eq!(evaluation.disposition.as_deref(), Some("keep"));
    assert_eq!(evaluation.comparable_runs, 1);
    assert_eq!(evaluation.missing_metric_runs, 0);
    assert_eq!(evaluation.score, Some(4));
    assert_eq!(
        evaluation.identity.procedure_id.value.as_deref(),
        Some("prototype1.branch_evaluation.operational_metrics.v1")
    );
    assert_eq!(
        evaluation.identity.evaluator.id.as_deref(),
        Some("prototype1.branch_evaluation.mechanized")
    );
    assert_eq!(evaluation.identity.evaluator.version.as_deref(), Some("v1"));
    assert_eq!(evaluation.identity.eval_set.kind, "closure_instance_slice");
    assert_eq!(evaluation.identity.eval_set.explicit, true);
    assert_eq!(
        evaluation.identity.eval_set.instance_ids,
        vec!["instance-a"]
    );
    assert_eq!(
        evaluation
            .provenance
            .source
            .pointer
            .ref_id()
            .ends_with("prototype1/evaluations/branch-a.json"),
        true
    );
    assert!(!diagnostic_field(&evaluation.diagnostics, "procedure_id"));
    assert!(!diagnostic_field(&evaluation.diagnostics, "evaluator"));
    assert!(!diagnostic_field(&evaluation.diagnostics, "eval_set"));

    let run = &evaluation.runs[0];
    assert_eq!(run.state, ScoreState::Complete);
    assert_eq!(
        run.provenance.baseline_record_path.as_deref(),
        run.provenance
            .baseline_run
            .as_ref()
            .map(|run| run.artifacts.record_path.as_path())
    );
    assert_eq!(
        run.provenance.treatment_record_path.as_deref(),
        run.provenance
            .treatment_run
            .as_ref()
            .map(|run| run.artifacts.record_path.as_path())
    );
    assert_eq!(
        run.provenance
            .baseline_run
            .as_ref()
            .map(|run| run.run_id.as_str()),
        Some("run-baseline")
    );
    assert_eq!(
        run.provenance
            .treatment_run
            .as_ref()
            .map(|run| run.run_id.as_str()),
        Some("run-treatment")
    );
    assert!(run.metrics.iter().any(|metric| {
        metric.field == "tool_calls_failed"
            && metric.direction == ScoreDirection::Improved
            && metric.points == 1
    }));
    assert!(run.metrics.iter().any(|metric| {
        metric.field == "oracle_eligible"
            && metric.direction == ScoreDirection::Improved
            && metric.points == 1
    }));
}

#[test]
fn malformed_typed_eval_set_identity_keeps_score_incomplete() {
    let cases = [
        (
            "blank-id",
            serde_json::json!({
                "id": "",
                "kind": "closure_instance_slice",
                "authority": "typed_closure_context",
                "explicit": true,
                "benchmark_family": "multi_swe_bench_rust",
                "dataset_sources": [{
                    "path": "dataset.jsonl",
                    "label": "sample"
                }],
                "eval_policy": {},
                "instance_ids": ["instance-a"]
            }),
            "blank required id",
        ),
        (
            "blank-kind",
            serde_json::json!({
                "id": "prototype1.eval_set.closure_instance_slice.v1:test",
                "kind": "   ",
                "authority": "typed_closure_context",
                "explicit": true,
                "benchmark_family": "multi_swe_bench_rust",
                "dataset_sources": [{
                    "path": "dataset.jsonl",
                    "label": "sample"
                }],
                "eval_policy": {},
                "instance_ids": ["instance-a"]
            }),
            "blank required kind",
        ),
        (
            "blank-authority",
            serde_json::json!({
                "id": "prototype1.eval_set.closure_instance_slice.v1:test",
                "kind": "closure_instance_slice",
                "authority": "\t",
                "explicit": true,
                "benchmark_family": "multi_swe_bench_rust",
                "dataset_sources": [{
                    "path": "dataset.jsonl",
                    "label": "sample"
                }],
                "eval_policy": {},
                "instance_ids": ["instance-a"]
            }),
            "blank required authority",
        ),
    ];

    for (case, eval_set_identity, expected_message) in cases {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        write_evaluation_with_eval_set_identity(
            tmp.path(),
            "branch-a",
            "keep",
            metrics(false, false, 3),
            metrics(true, true, 1),
            eval_set_identity,
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let child = &scores.children[0];
        let evaluation = &child.evaluations[0];

        assert_eq!(child.state, ScoreState::Incomplete, "{case}");
        assert_eq!(child.comparable_score(), None, "{case}");
        assert_eq!(evaluation.state, ScoreState::Incomplete, "{case}");
        assert_eq!(evaluation.score, Some(4), "{case}");
        assert!(diagnostic_message(
            &evaluation.diagnostics,
            "eval_set",
            expected_message
        ));
    }
}

#[test]
fn keeps_missing_metrics_diagnostic_instead_of_scoring_paths() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    write_evaluation_without_metrics(tmp.path(), "branch-a");

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);
    let evaluation = &scores.children[0].evaluations[0];

    assert_eq!(scores.children[0].state, ScoreState::Incomplete);
    assert_eq!(scores.children[0].score, None);
    assert_eq!(evaluation.state, ScoreState::Incomplete);
    assert_eq!(evaluation.score, None);
    assert_eq!(evaluation.comparable_runs, 0);
    assert_eq!(evaluation.missing_metric_runs, 1);
    assert_eq!(evaluation.runs[0].score, None);
    assert!(diagnostic_field(
        &evaluation.runs[0].diagnostics,
        "operational_metrics"
    ));
}

#[test]
fn missing_run_registration_keeps_score_incomplete_and_non_comparable() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    write_evaluation_named_inner(
        tmp.path(),
        "branch-a",
        "branch-a",
        "keep",
        metrics(false, false, 3),
        metrics(true, true, 1),
        true,
        false,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);
    let child = &scores.children[0];
    let evaluation = &child.evaluations[0];
    let run = &evaluation.runs[0];

    assert_eq!(child.state, ScoreState::Incomplete);
    assert_eq!(child.score, Some(4));
    assert_eq!(child.comparable_score(), None);
    assert_eq!(evaluation.state, ScoreState::Incomplete);
    assert_eq!(evaluation.score, Some(4));
    assert_eq!(evaluation.comparable_runs, 0);
    assert_eq!(evaluation.missing_metric_runs, 0);
    assert_eq!(run.state, ScoreState::Incomplete);
    assert_eq!(run.score, Some(4));
    assert!(diagnostic_field(
        &run.diagnostics,
        "baseline_run_registration"
    ));
    assert!(diagnostic_field(
        &run.diagnostics,
        "treatment_run_registration"
    ));
}

#[test]
fn attaches_protocol_artifact_envelopes_to_run_provenance() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    let mut baseline_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-baseline",
        RegisteredRunRole::Control,
    );
    let baseline_anchor = write_protocol_artifact_envelope(
        &baseline_registration,
        "tool_call_intent_segmentation",
        1_000,
        Some("protocol-model"),
        Some("protocol-provider"),
    );
    baseline_registration.update_protocol_anchor(Some(baseline_anchor.clone()));
    baseline_registration.persist().expect("persist baseline");
    let treatment_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-treatment",
        RegisteredRunRole::Treatment,
    );
    write_evaluation_for_registrations(
        tmp.path(),
        "branch-a",
        metrics(false, false, 3),
        metrics(true, true, 1),
        &baseline_registration,
        &treatment_registration,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);
    let run = &scores.children[0].evaluations[0].runs[0];
    let baseline = run
        .provenance
        .baseline_run
        .as_ref()
        .expect("baseline run evidence");

    assert_eq!(run.state, ScoreState::Complete);
    assert_eq!(run.score, Some(4));
    assert_eq!(
        baseline.protocol.anchor_path.as_deref(),
        Some(baseline_anchor.as_path())
    );
    assert_eq!(
        baseline.protocol.artifacts_dir,
        baseline_registration.artifacts.protocol_artifacts_dir
    );
    assert_eq!(baseline.protocol.artifacts.len(), 1);
    let artifact = &baseline.protocol.artifacts[0];
    assert_eq!(artifact.path, baseline_anchor);
    assert_eq!(artifact.procedure_name, "tool_call_intent_segmentation");
    assert_eq!(artifact.run_id, "run-baseline");
    assert_eq!(artifact.subject_id, "instance-a");
    assert_eq!(artifact.model_id.as_deref(), Some("protocol-model"));
    assert_eq!(artifact.provider_slug.as_deref(), Some("protocol-provider"));
    assert!(baseline.protocol.diagnostics.is_empty());
}

#[test]
fn missing_protocol_artifact_refs_do_not_break_complete_operational_score() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    let mut baseline_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-baseline",
        RegisteredRunRole::Control,
    );
    let missing_anchor = baseline_registration
        .artifacts
        .protocol_artifacts_dir
        .join("missing-anchor.json");
    baseline_registration.update_protocol_anchor(Some(missing_anchor));
    baseline_registration.persist().expect("persist baseline");
    let treatment_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-treatment",
        RegisteredRunRole::Treatment,
    );
    write_evaluation_for_registrations(
        tmp.path(),
        "branch-a",
        metrics(false, false, 3),
        metrics(true, true, 1),
        &baseline_registration,
        &treatment_registration,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);
    let child = &scores.children[0];
    let run = &child.evaluations[0].runs[0];
    let baseline = run
        .provenance
        .baseline_run
        .as_ref()
        .expect("baseline run evidence");

    assert_eq!(child.state, ScoreState::Complete);
    assert_eq!(child.comparable_score(), Some(4));
    assert_eq!(run.state, ScoreState::Complete);
    assert_eq!(baseline.protocol.artifacts.len(), 0);
    assert!(
        baseline
            .protocol
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.field == "protocol_artifacts_dir")
    );
    assert!(!diagnostic_field(
        &run.diagnostics,
        "protocol_artifacts_dir"
    ));
}

#[test]
fn rejects_protocol_artifact_envelopes_with_wrong_identity() {
    let cases = [
        (
            "wrong schema version",
            "protocol_artifact.schema_version",
            Some("protocol-artifact.v0"),
            None,
            None,
        ),
        (
            "wrong run id",
            "protocol_artifact.run_id",
            None,
            Some("run-other"),
            None,
        ),
        (
            "wrong subject id",
            "protocol_artifact.subject_id",
            None,
            None,
            Some("instance-other"),
        ),
    ];

    for (case, expected_field, schema_version, run_id, subject_id) in cases {
        let tmp = tempfile::tempdir().expect("tmp");
        let manifest = campaign_manifest(tmp.path());
        write_node(tmp.path(), "node-a", "branch-a");
        let baseline_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-baseline",
            RegisteredRunRole::Control,
        );
        write_protocol_artifact_envelope_with_payload(
            &baseline_registration,
            "tool_call_intent_segmentation",
            1_000,
            ProtocolEnvelopeOverrides {
                schema_version,
                run_id,
                subject_id,
                ..ProtocolEnvelopeOverrides::default()
            },
            serde_json::json!({"fixture": "ignored"}),
            serde_json::json!({"fixture": "ignored"}),
            serde_json::json!({"fixture": "ignored"}),
        );
        let treatment_registration = write_run_registration(
            tmp.path(),
            "instance-a",
            "run-treatment",
            RegisteredRunRole::Treatment,
        );
        write_evaluation_for_registrations(
            tmp.path(),
            "branch-a",
            metrics(false, false, 3),
            metrics(true, true, 1),
            &baseline_registration,
            &treatment_registration,
        );

        let evidence = FsEvidenceStore::new(&manifest)
            .child_evidence()
            .expect("child evidence");
        let scores = ScoreSet::from_evidence(&evidence);
        let child = &scores.children[0];
        let run = &child.evaluations[0].runs[0];
        let baseline = run
            .provenance
            .baseline_run
            .as_ref()
            .expect("baseline run evidence");

        assert_eq!(child.state, ScoreState::Complete, "{case}");
        assert_eq!(child.comparable_score(), Some(4), "{case}");
        assert_eq!(run.state, ScoreState::Complete, "{case}");
        assert_eq!(baseline.protocol.artifacts.len(), 0, "{case}");
        assert!(
            baseline
                .protocol
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.field == expected_field),
            "{case}"
        );
        assert!(
            !diagnostic_field(&run.diagnostics, expected_field),
            "{case}"
        );
    }
}

#[test]
fn protocol_artifact_attachment_ignores_payload_identity_fields() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    let baseline_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-baseline",
        RegisteredRunRole::Control,
    );
    let artifact_path = write_protocol_artifact_envelope_with_payload(
        &baseline_registration,
        "tool_call_intent_segmentation",
        1_000,
        ProtocolEnvelopeOverrides {
            model_id: Some("protocol-model"),
            provider_slug: Some("protocol-provider"),
            ..ProtocolEnvelopeOverrides::default()
        },
        serde_json::json!({
            "schema_version": "payload-schema",
            "run_id": "payload-run",
            "subject_id": "payload-subject",
            "protocol_anchor": "/tmp/payload/anchor.json",
            "baseline_record_path": "/tmp/payload/baseline.json.gz"
        }),
        serde_json::json!({
            "run_id": "payload-output-run",
            "subject_id": "payload-output-subject",
            "protocol_artifacts_dir": "/tmp/payload/protocol-artifacts"
        }),
        serde_json::json!({
            "procedure_name": "payload_procedure",
            "run_id": "payload-artifact-run",
            "subject_id": "payload-artifact-subject",
            "path": "/tmp/payload/path-shaped-ref.json"
        }),
    );
    let treatment_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-treatment",
        RegisteredRunRole::Treatment,
    );
    write_evaluation_for_registrations(
        tmp.path(),
        "branch-a",
        metrics(false, false, 3),
        metrics(true, true, 1),
        &baseline_registration,
        &treatment_registration,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);
    let run = &scores.children[0].evaluations[0].runs[0];
    let baseline = run
        .provenance
        .baseline_run
        .as_ref()
        .expect("baseline run evidence");

    assert_eq!(run.state, ScoreState::Complete);
    assert_eq!(run.score, Some(4));
    assert_eq!(baseline.protocol.artifacts.len(), 1);
    assert!(baseline.protocol.diagnostics.is_empty());

    let artifact = &baseline.protocol.artifacts[0];
    assert_eq!(artifact.path, artifact_path);
    assert_eq!(artifact.schema_version, "protocol-artifact.v1");
    assert_eq!(artifact.procedure_name, "tool_call_intent_segmentation");
    assert_eq!(artifact.run_id, "run-baseline");
    assert_eq!(artifact.subject_id, "instance-a");
    assert_eq!(artifact.model_id.as_deref(), Some("protocol-model"));
    assert_eq!(artifact.provider_slug.as_deref(), Some("protocol-provider"));
}

#[test]
fn operational_default_unchanged_with_protocol_aggregate_payloads() {
    let tmp = tempfile::tempdir().expect("tmp");
    let env_guard = eval_home_guard(tmp.path());
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    let (baseline_registration, treatment_registration) =
        write_protocol_scoring_registrations(tmp.path());
    write_evaluation_for_registrations(
        tmp.path(),
        "branch-a",
        metrics(false, false, 3),
        metrics(true, true, 1),
        &baseline_registration,
        &treatment_registration,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);
    let child = &scores.children[0];
    let run = &child.evaluations[0].runs[0];

    assert_eq!(scores.profile, ScoreProfile::operational());
    assert_eq!(child.state, ScoreState::Complete);
    assert_eq!(child.score, Some(4));
    assert_eq!(child.comparable_score(), Some(4));
    assert_eq!(run.state, ScoreState::Complete);
    assert_eq!(run.score, Some(4));
    assert_eq!(run.components.len(), 1);
    assert_eq!(
        run.components[0].component_id,
        super::OPERATIONAL_COMPONENT_ID
    );
    assert!(
        !run.components
            .iter()
            .any(|component| component.component_id == super::PROTOCOL_COMPONENT_ID)
    );
    assert!(!diagnostic_field(
        &run.diagnostics,
        "treatment.protocol.protocol_anchor"
    ));
    drop(env_guard);
}

#[test]
fn protocol_enabled_profile_creates_protocol_component() {
    let tmp = tempfile::tempdir().expect("tmp");
    let env_guard = eval_home_guard(tmp.path());
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    let (baseline_registration, treatment_registration) =
        write_protocol_scoring_registrations(tmp.path());
    write_evaluation_for_registrations(
        tmp.path(),
        "branch-a",
        metrics(false, false, 3),
        metrics(true, true, 1),
        &baseline_registration,
        &treatment_registration,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let profile = ScoreProfile::operational_with_protocol();
    let scores = ScoreSet::from_evidence_with_profile(&evidence, &profile);
    let child = &scores.children[0];
    let run = &child.evaluations[0].runs[0];
    let protocol = run
        .components
        .iter()
        .find(|component| component.component_id == super::PROTOCOL_COMPONENT_ID)
        .expect("protocol component");

    assert_eq!(scores.profile, profile);
    assert_eq!(child.state, ScoreState::Complete);
    assert_eq!(child.comparable_score(), child.score);
    assert!(child.score.expect("score") > 4);
    assert_eq!(protocol.state, ScoreState::Complete);
    assert!(protocol.score.expect("protocol score") > 0);
    assert!(protocol.metrics.iter().any(|metric| {
        metric.field == "protocol.reviewed_call_count"
            && metric.direction == ScoreDirection::Improved
    }));
    assert!(!protocol.provenance.baseline_protocol_artifacts.is_empty());
    assert!(!protocol.provenance.treatment_protocol_artifacts.is_empty());
    assert!(diagnostic_field(
        &protocol.diagnostics,
        "treatment.protocol.protocol_anchor"
    ));
    drop(env_guard);
}

#[test]
fn missing_protocol_evidence_affects_only_protocol_profile_component() {
    let tmp = tempfile::tempdir().expect("tmp");
    let env_guard = eval_home_guard(tmp.path());
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    let mut baseline_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-baseline",
        RegisteredRunRole::Control,
    );
    let missing_anchor = baseline_registration
        .artifacts
        .protocol_artifacts_dir
        .join("missing-anchor.json");
    baseline_registration.update_protocol_anchor(Some(missing_anchor));
    baseline_registration.persist().expect("persist baseline");
    let treatment_registration = write_run_registration(
        tmp.path(),
        "instance-a",
        "run-treatment",
        RegisteredRunRole::Treatment,
    );
    write_evaluation_for_registrations(
        tmp.path(),
        "branch-a",
        metrics(false, false, 3),
        metrics(true, true, 1),
        &baseline_registration,
        &treatment_registration,
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let operational = ScoreSet::from_evidence(&evidence);
    let operational_child = &operational.children[0];
    assert_eq!(operational_child.state, ScoreState::Complete);
    assert_eq!(operational_child.comparable_score(), Some(4));

    let protocol_scores =
        ScoreSet::from_evidence_with_profile(&evidence, &ScoreProfile::operational_with_protocol());
    let child = &protocol_scores.children[0];
    let run = &child.evaluations[0].runs[0];
    let operational_component = run
        .components
        .iter()
        .find(|component| component.component_id == super::OPERATIONAL_COMPONENT_ID)
        .expect("operational component");
    let protocol_component = run
        .components
        .iter()
        .find(|component| component.component_id == super::PROTOCOL_COMPONENT_ID)
        .expect("protocol component");

    assert_eq!(operational_component.state, ScoreState::Complete);
    assert_eq!(operational_component.score, Some(4));
    assert_eq!(protocol_component.state, ScoreState::Incomplete);
    assert!(diagnostic_field(
        &protocol_component.diagnostics,
        "baseline_protocol_anchor"
    ));
    assert!(!diagnostic_field(
        &run.diagnostics,
        "baseline_protocol_anchor"
    ));
    assert_eq!(child.state, ScoreState::Incomplete);
    assert_eq!(child.comparable_score(), None);
    drop(env_guard);
}

#[test]
fn branch_mismatch_invalidates_evaluation_score() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    write_evaluation_without_identity(
        tmp.path(),
        "branch-a",
        "keep",
        metrics(false, false, 3),
        metrics(true, true, 1),
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let mut child_evidence = evidence.children[0].clone();
    child_evidence.evaluations[0].branch_id = "branch-b".to_string();
    let child = ChildScore::from_child(&child_evidence);
    let evaluation = &child.evaluations[0];

    assert_eq!(child.state, ScoreState::Invalid);
    assert_eq!(child.score, None);
    assert_eq!(child.comparable_score(), None);
    assert_eq!(evaluation.state, ScoreState::Invalid);
    assert_eq!(evaluation.score, None);
    assert!(diagnostic_field(&evaluation.diagnostics, "branch_id"));
}

#[test]
fn complete_child_score_is_comparable_only_through_gate() {
    let child = ChildScore {
        node_id: "node-a".to_string(),
        parent_node_id: Some("node-parent".to_string()),
        generation: Some(2),
        branch_id: Some("branch-a".to_string()),
        state: ScoreState::Complete,
        score: Some(7),
        evaluations: Vec::new(),
        diagnostics: Vec::new(),
    };

    assert_eq!(child.score, Some(7));
    assert_eq!(child.comparable_score(), Some(7));
}

#[test]
fn score_selection_review_keeps_incomplete_score_out_of_local_alpha() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    write_evaluation_without_identity(
        tmp.path(),
        "branch-a",
        "keep",
        metrics(false, false, 3),
        metrics(true, true, 1),
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let selection = evidence.selection_inputs();
    let expected_individual = crate::successor_selection::decide(selection.inputs[0].clone());
    let expected_generation = crate::successor_selection::operator_projection::generation_summary(
        selection.inputs.clone(),
    );
    let review = ScoreSelectionReview::from_evidence(&evidence);

    let row = review
        .rows
        .iter()
        .find(|row| row.node_id == "node-a" && row.branch_id.as_deref() == Some("branch-a"))
        .expect("review row");

    assert_eq!(row.selection_status, ScoreSelectionStatus::Projected);
    assert_eq!(row.child_score_state, Some(ScoreState::Incomplete));
    assert_eq!(row.diagnostic_score, Some(4));
    assert_eq!(row.local_alpha_candidate, None);
    assert_eq!(row.individual_decision.as_ref(), Some(&expected_individual));
    assert_eq!(row.generation_decision, expected_generation);
    assert!(review_diagnostic_field(row, "score_identity"));
}

#[test]
fn score_selection_review_invalid_score_has_no_local_alpha() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    write_evaluation_named(
        tmp.path(),
        "branch-a-first",
        "branch-a",
        "keep",
        metrics(false, false, 3),
        metrics(true, true, 1),
    );
    write_evaluation_named(
        tmp.path(),
        "branch-a-second",
        "branch-a",
        "keep",
        metrics(false, false, 4),
        metrics(true, true, 1),
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let review = ScoreSelectionReview::from_evidence(&evidence);

    let row = review
        .rows
        .iter()
        .find(|row| row.node_id == "node-a" && row.branch_id.as_deref() == Some("branch-a"))
        .expect("score row");

    assert_eq!(row.selection_status, ScoreSelectionStatus::MissingSelection);
    assert_eq!(row.child_score_state, Some(ScoreState::Invalid));
    assert_eq!(row.diagnostic_score, None);
    assert_eq!(row.local_alpha_candidate, None);
    assert!(review_diagnostic_field(row, "child_score"));
    assert!(review.rows.iter().any(|row| {
        row.node_id == "node-a" && row.selection_status == ScoreSelectionStatus::FailedProjection
    }));
}

#[test]
fn score_selection_review_reports_selection_input_without_score_row() {
    let review = ScoreSelectionReview::from_parts(
        empty_scores(),
        super::SelectionProjectionSet {
            inputs: vec![selection_input(
                "node-a",
                "branch-a",
                2,
                BranchDisposition::Keep,
            )],
            failures: Vec::new(),
        },
    );

    let row = &review.rows[0];
    assert_eq!(row.selection_status, ScoreSelectionStatus::Projected);
    assert_eq!(row.child_score_state, None);
    assert_eq!(row.local_alpha_candidate, None);
    assert!(review_diagnostic_field(row, "score_row"));
}

#[test]
fn score_selection_review_reports_score_row_without_selection_input() {
    let review = ScoreSelectionReview::from_parts(
        ScoreSet {
            schema_version: super::SCHEMA_VERSION.to_string(),
            procedure_id: super::PROCEDURE_ID.to_string(),
            profile: super::ScoreProfile::operational(),
            children: vec![complete_score("node-a", "branch-a", 2, 7)],
            diagnostics: Vec::new(),
        },
        super::SelectionProjectionSet {
            inputs: Vec::new(),
            failures: Vec::new(),
        },
    );

    let row = &review.rows[0];
    assert_eq!(row.selection_status, ScoreSelectionStatus::MissingSelection);
    assert_eq!(row.child_score_state, Some(ScoreState::Complete));
    assert_eq!(row.diagnostic_score, Some(7));
    assert_eq!(row.local_alpha_candidate, None);
    assert!(review_diagnostic_field(row, "selection_input"));
}

#[test]
fn duplicate_branch_evaluations_invalidate_child_score() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    write_node(tmp.path(), "node-a", "branch-a");
    write_evaluation_named(
        tmp.path(),
        "branch-a-first",
        "branch-a",
        "keep",
        metrics(false, false, 3),
        metrics(true, true, 1),
    );
    write_evaluation_named(
        tmp.path(),
        "branch-a-second",
        "branch-a",
        "keep",
        metrics(false, false, 4),
        metrics(true, true, 1),
    );

    let evidence = FsEvidenceStore::new(&manifest)
        .child_evidence()
        .expect("child evidence");
    let scores = ScoreSet::from_evidence(&evidence);
    let child = &scores.children[0];

    assert_eq!(child.evaluations.len(), 2);
    assert_eq!(child.state, ScoreState::Invalid);
    assert_eq!(child.score, None);
    assert_eq!(child.comparable_score(), None);
    assert!(diagnostic_field(&child.diagnostics, "evaluations"));
}

#[test]
fn score_build_aborts_on_malformed_declared_typed_record() {
    let tmp = tempfile::tempdir().expect("tmp");
    let manifest = campaign_manifest(tmp.path());
    let node_dir = tmp.path().join("prototype1/nodes/node-a");
    fs::create_dir_all(&node_dir).expect("node dir");
    fs::write(node_dir.join("node.json"), "{ malformed").expect("node");

    let err = super::build("campaign-a", &manifest).expect_err("malformed record aborts");

    match err {
        super::PreviewError::ParseRecord { path, record, .. } => {
            assert_eq!(path, node_dir.join("node.json"));
            assert_eq!(record, "Prototype1NodeRecord");
        }
        other => panic!("expected ParseRecord, got {other:?}"),
    }
}

fn eval_home_guard(root: &Path) -> MutexGuard<'static, ()> {
    let guard = crate::test_support::env_lock().lock().expect("env lock");
    unsafe {
        std::env::set_var("PLOKE_EVAL_HOME", root);
    }
    guard
}

fn write_protocol_scoring_registrations(root: &Path) -> (RunRegistration, RunRegistration) {
    let mut baseline_registration = write_run_registration(
        root,
        "instance-a",
        "run-baseline",
        RegisteredRunRole::Control,
    );
    let baseline_anchor = write_protocol_anchor(&baseline_registration, 1_000);
    baseline_registration.update_protocol_anchor(Some(baseline_anchor));
    baseline_registration.persist().expect("persist baseline");

    let mut treatment_registration = write_run_registration(
        root,
        "instance-a",
        "run-treatment",
        RegisteredRunRole::Treatment,
    );
    write_protocol_anchor(&treatment_registration, 1_000);
    write_protocol_call_review(&treatment_registration, 2_000);
    treatment_registration.update_protocol_anchor(Some(
        treatment_registration
            .artifacts
            .protocol_artifacts_dir
            .join("stale-anchor-ref.json"),
    ));
    treatment_registration.persist().expect("persist treatment");

    (baseline_registration, treatment_registration)
}

fn write_protocol_anchor(registration: &RunRegistration, created_at_ms: u64) -> std::path::PathBuf {
    let output = serde_json::json!({
        "coverage": {
            "ambiguous_calls": 0,
            "ambiguous_segments": 0,
            "labeled_calls": 2,
            "labeled_segments": 1,
            "total_calls": 2,
            "uncovered_calls": 0
        },
        "segments": [{
            "calls": [
                protocol_call_json(0, 1, "rg", "search", "find target"),
                protocol_call_json(1, 1, "sed", "read", "inspect file")
            ],
            "confidence": "high",
            "end_index": 1,
            "label": "locate_target",
            "rationale": "locate",
            "segment_index": 0,
            "start_index": 0,
            "status": "labeled",
            "turns": [1]
        }],
        "sequence": {
            "subject_id": registration.frozen_spec.task_id.as_str(),
            "total_turns": 1,
            "total_calls_in_run": 2,
            "turns": [protocol_turn_json()],
            "calls": [
                protocol_call_json(0, 1, "rg", "search", "find target"),
                protocol_call_json(1, 1, "sed", "read", "inspect file")
            ]
        },
        "signals": {
            "browse_calls": 0,
            "directory_pivots": 0,
            "edit_calls": 0,
            "execute_calls": 0,
            "failed_calls": 0,
            "read_calls": 1,
            "repeated_search_runs": 0,
            "search_calls": 1,
            "search_terms_seen": [],
            "total_calls": 2,
            "total_turns": 1
        },
        "overall_rationale": "one labeled segment"
    });
    let input = output["sequence"].clone();
    let artifact = protocol_anchor_artifact_json(input.clone(), output.clone());
    write_protocol_artifact_envelope_with_payload(
        registration,
        "tool_call_intent_segmentation",
        created_at_ms,
        ProtocolEnvelopeOverrides::default(),
        input,
        output,
        artifact,
    )
}

fn write_protocol_call_review(
    registration: &RunRegistration,
    created_at_ms: u64,
) -> std::path::PathBuf {
    let input = serde_json::json!({
        "subject_id": registration.frozen_spec.task_id.as_str(),
        "total_calls_in_run": 2,
        "total_calls_in_turn": 2,
        "turn": protocol_turn_json(),
        "before": [protocol_call_json(0, 1, "rg", "search", "find target")],
        "focal": protocol_call_json(1, 1, "sed", "read", "inspect file"),
        "after": []
    });
    let output = serde_json::json!({
        "overall": "focused_progress",
        "overall_confidence": "high",
        "synthesis_rationale": "focused review",
        "packet": {
            "calls": [protocol_call_json(1, 1, "sed", "read", "inspect file")],
            "focal_call_index": 1,
            "scope_summary": "focal call 1",
            "subject_id": registration.frozen_spec.task_id.as_str(),
            "target_id": "call:1",
            "target_kind": "focal_call",
            "total_calls_in_run": 2,
            "total_calls_in_scope": 1,
            "turn_span": [1]
        },
        "recoverability": {
            "confidence": "high",
            "rationale": "recover",
            "verdict": "clear_next_step"
        },
        "redundancy": {
            "confidence": "high",
            "rationale": "redundant",
            "verdict": "distinct"
        },
        "signals": {
            "browse_calls_in_scope": 0,
            "candidate_concerns": [],
            "directory_pivots": 0,
            "distinct_tool_count": 1,
            "edit_calls_in_scope": 0,
            "execute_calls_in_scope": 0,
            "failed_calls_in_scope": 0,
            "read_calls_in_scope": 1,
            "repeated_tool_name_count": 0,
            "scope_turn_count": 1,
            "search_calls_in_scope": 0,
            "similar_search_neighbors": 0
        },
        "usefulness": {
            "confidence": "high",
            "rationale": "useful",
            "verdict": "key_progress"
        }
    });
    let artifact = protocol_call_review_artifact_json(input.clone(), output.clone());
    write_protocol_artifact_envelope_with_payload(
        registration,
        "tool_call_review",
        created_at_ms,
        ProtocolEnvelopeOverrides::default(),
        input,
        output,
        artifact,
    )
}

fn protocol_anchor_artifact_json(
    input: serde_json::Value,
    output: serde_json::Value,
) -> serde_json::Value {
    let context = serde_json::json!({
        "sequence": input.clone(),
        "signals": output["signals"].clone(),
    });
    let judgment = serde_json::json!({
        "segments": output["segments"].clone(),
        "overall_rationale": output["overall_rationale"].clone(),
    });
    serde_json::json!({
        "procedure_name": "tool_call_intent_segmentation",
        "artifact": {
            "first": protocol_mechanized_step_json(input, context.clone()),
            "second": {
                "branches": {
                    "input": {
                        "state": context.clone(),
                        "disposition": "record_and_forward"
                    },
                    "left_branch": "mechanized",
                    "right_branch": "llm",
                    "left": protocol_mechanized_step_json(context.clone(), context.clone()),
                    "right": protocol_llm_step_json(context.clone(), judgment.clone())
                },
                "merge": protocol_mechanized_step_json(
                    serde_json::json!({
                        "source": context,
                        "left": output_context_from_anchor(&output),
                        "right": judgment
                    }),
                    output
                )
            }
        }
    })
}

fn output_context_from_anchor(output: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "sequence": output["sequence"].clone(),
        "signals": output["signals"].clone(),
    })
}

fn protocol_call_review_artifact_json(
    input: serde_json::Value,
    output: serde_json::Value,
) -> serde_json::Value {
    let context = serde_json::json!({
        "packet": output["packet"].clone(),
        "signals": output["signals"].clone(),
    });
    let usefulness = output["usefulness"].clone();
    let redundancy = output["redundancy"].clone();
    let recoverability = output["recoverability"].clone();
    serde_json::json!({
        "procedure_name": "tool_call_review",
        "artifact": {
            "first": protocol_mechanized_step_json(input, context.clone()),
            "second": {
                "branches": {
                    "input": {
                        "state": context.clone(),
                        "disposition": "record_and_forward"
                    },
                    "left_branch": "usefulness_redundancy",
                    "right_branch": "recoverability",
                    "left": {
                        "input": {
                            "state": context.clone(),
                            "disposition": "record_and_forward"
                        },
                        "left_branch": "usefulness",
                        "right_branch": "redundancy",
                        "left": protocol_llm_step_json(context.clone(), usefulness.clone()),
                        "right": protocol_llm_step_json(context.clone(), redundancy.clone())
                    },
                    "right": protocol_llm_step_json(context.clone(), recoverability.clone())
                },
                "merge": protocol_mechanized_step_json(
                    serde_json::json!({
                        "source": context,
                        "left": {
                            "source": output_context_from_review(&output),
                            "left": usefulness,
                            "right": redundancy
                        },
                        "right": recoverability
                    }),
                    output
                )
            }
        }
    })
}

fn output_context_from_review(output: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "packet": output["packet"].clone(),
        "signals": output["signals"].clone(),
    })
}

fn protocol_mechanized_step_json(
    input: serde_json::Value,
    output: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "step_id": "test_mechanized",
        "step_name": "test_mechanized",
        "executor_kind": "mechanized",
        "executor_label": "test",
        "evidence_policy": {},
        "input": input,
        "input_disposition": "record_and_forward",
        "output": output,
        "output_disposition": "record_and_forward",
        "provenance": {
            "strategy": "test"
        }
    })
}

fn protocol_llm_step_json(
    input: serde_json::Value,
    output: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "step_id": "test_llm",
        "step_name": "test_llm",
        "executor_kind": "llm_adjudicator",
        "executor_label": "test",
        "evidence_policy": {},
        "input": input,
        "input_disposition": "record_and_forward",
        "output": output,
        "output_disposition": "record_and_forward",
        "provenance": {
            "model_id": "model",
            "provider_slug": "provider",
            "raw_content": "{}",
            "response": {}
        }
    })
}

fn protocol_turn_json() -> serde_json::Value {
    serde_json::json!({
        "turn": 1,
        "tool_count": 2,
        "failed_tool_count": 0,
        "patch_proposed": false,
        "patch_applied": false
    })
}

fn protocol_call_json(
    index: usize,
    turn: u32,
    tool_name: &str,
    tool_kind: &str,
    summary: &str,
) -> serde_json::Value {
    serde_json::json!({
        "index": index,
        "turn": turn,
        "tool_name": tool_name,
        "tool_kind": tool_kind,
        "failed": false,
        "latency_ms": 10,
        "summary": summary,
        "args_preview": "",
        "result_preview": ""
    })
}

fn campaign_manifest(root: &Path) -> std::path::PathBuf {
    let manifest = root.join("campaign.json");
    fs::write(&manifest, "{}").expect("manifest");
    manifest
}

fn write_node(root: &Path, node_id: &str, branch_id: &str) {
    let node_dir = root.join("prototype1/nodes").join(node_id);
    fs::create_dir_all(&node_dir).expect("node dir");
    fs::write(
        node_dir.join("node.json"),
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
        .to_string(),
    )
    .expect("node");
}

fn write_evaluation(
    root: &Path,
    branch_id: &str,
    disposition: &str,
    parent_metrics: OperationalRunMetrics,
    child_metrics: OperationalRunMetrics,
) {
    write_evaluation_named(
        root,
        branch_id,
        branch_id,
        disposition,
        parent_metrics,
        child_metrics,
    );
}

fn write_evaluation_without_identity(
    root: &Path,
    branch_id: &str,
    disposition: &str,
    parent_metrics: OperationalRunMetrics,
    child_metrics: OperationalRunMetrics,
) {
    write_evaluation_named_inner(
        root,
        branch_id,
        branch_id,
        disposition,
        parent_metrics,
        child_metrics,
        false,
        false,
    );
}

fn write_evaluation_with_eval_set_identity(
    root: &Path,
    branch_id: &str,
    disposition: &str,
    parent_metrics: OperationalRunMetrics,
    child_metrics: OperationalRunMetrics,
    eval_set_identity: serde_json::Value,
) {
    write_evaluation_named_inner(
        root,
        branch_id,
        branch_id,
        disposition,
        parent_metrics,
        child_metrics,
        true,
        true,
    );
    let path = root
        .join("prototype1/evaluations")
        .join(format!("{branch_id}.json"));
    let mut report: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("evaluation"))
            .expect("evaluation json");
    report["eval_set_identity"] = eval_set_identity;
    fs::write(path, report.to_string()).expect("evaluation");
}

fn write_evaluation_named(
    root: &Path,
    file_stem: &str,
    branch_id: &str,
    disposition: &str,
    parent_metrics: OperationalRunMetrics,
    child_metrics: OperationalRunMetrics,
) {
    write_evaluation_named_inner(
        root,
        file_stem,
        branch_id,
        disposition,
        parent_metrics,
        child_metrics,
        true,
        true,
    );
}

fn write_evaluation_named_inner(
    root: &Path,
    file_stem: &str,
    branch_id: &str,
    disposition: &str,
    parent_metrics: OperationalRunMetrics,
    child_metrics: OperationalRunMetrics,
    include_identity: bool,
    include_registration: bool,
) {
    let eval_dir = root.join("prototype1/evaluations");
    fs::create_dir_all(&eval_dir).expect("eval dir");
    let (baseline_registration, treatment_registration) = if include_registration {
        (
            Some(write_run_registration(
                root,
                "instance-a",
                "run-baseline",
                RegisteredRunRole::Control,
            )),
            Some(write_run_registration(
                root,
                "instance-a",
                "run-treatment",
                RegisteredRunRole::Treatment,
            )),
        )
    } else {
        (None, None)
    };
    let baseline_record_path = baseline_registration
        .as_ref()
        .map(|registration| registration.artifacts.record_path.clone())
        .unwrap_or_else(|| Path::new("/tmp/baseline/record.json.gz").to_path_buf());
    let treatment_record_path = treatment_registration
        .as_ref()
        .map(|registration| registration.artifacts.record_path.clone())
        .unwrap_or_else(|| Path::new("/tmp/treatment/record.json.gz").to_path_buf());
    let mut report = serde_json::json!({
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
            "baseline_record_path": baseline_record_path,
            "treatment_record_path": treatment_record_path,
            "baseline_metrics": parent_metrics,
            "treatment_metrics": child_metrics,
            "evaluation": null,
            "status": "compared"
        }]
    });
    if let Some(registration) = baseline_registration.as_ref() {
        report["compared_instances"][0]["baseline_registration_path"] =
            serde_json::json!(registration.registry_path());
    }
    if let Some(registration) = treatment_registration.as_ref() {
        report["compared_instances"][0]["treatment_registration_path"] =
            serde_json::json!(registration.registry_path());
    }
    if include_identity {
        report["evaluation_procedure_id"] =
            serde_json::json!("prototype1.branch_evaluation.operational_metrics.v1");
        report["evaluator_identity"] = serde_json::json!({
            "id": "prototype1.branch_evaluation.mechanized",
            "version": "v1"
        });
        report["eval_set_identity"] = serde_json::json!({
            "id": "prototype1.eval_set.closure_instance_slice.v1:test",
            "kind": "closure_instance_slice",
            "authority": "typed_closure_context",
            "explicit": true,
            "benchmark_family": "multi_swe_bench_rust",
            "dataset_sources": [{
                "path": root.join("dataset.jsonl"),
                "label": "sample"
            }],
            "eval_policy": {},
            "instance_ids": ["instance-a"]
        });
    }
    fs::write(
        eval_dir.join(format!("{file_stem}.json")),
        report.to_string(),
    )
    .expect("evaluation");
}

fn write_evaluation_for_registrations(
    root: &Path,
    branch_id: &str,
    parent_metrics: OperationalRunMetrics,
    child_metrics: OperationalRunMetrics,
    baseline_registration: &RunRegistration,
    treatment_registration: &RunRegistration,
) {
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
            "overall_disposition": "keep",
            "reasons": ["test"],
            "evaluation_procedure_id": "prototype1.branch_evaluation.operational_metrics.v1",
            "evaluator_identity": {
                "id": "prototype1.branch_evaluation.mechanized",
                "version": "v1"
            },
            "eval_set_identity": {
                "id": "prototype1.eval_set.closure_instance_slice.v1:test",
                "kind": "closure_instance_slice",
                "authority": "typed_closure_context",
                "explicit": true,
                "benchmark_family": "multi_swe_bench_rust",
                "dataset_sources": [{
                    "path": root.join("dataset.jsonl"),
                    "label": "sample"
                }],
                "eval_policy": {},
                "instance_ids": ["instance-a"]
            },
            "compared_instances": [{
                "instance_id": "instance-a",
                "baseline_registration_path": baseline_registration.registry_path(),
                "treatment_registration_path": treatment_registration.registry_path(),
                "baseline_record_path": baseline_registration.artifacts.record_path,
                "treatment_record_path": treatment_registration.artifacts.record_path,
                "baseline_metrics": parent_metrics,
                "treatment_metrics": child_metrics,
                "evaluation": null,
                "status": "compared"
            }]
        })
        .to_string(),
    )
    .expect("evaluation");
}

fn write_run_registration(
    root: &Path,
    instance_id: &str,
    run_id: &str,
    run_role: RegisteredRunRole,
) -> RunRegistration {
    let intent = RunIntent {
        task_id: instance_id.to_string(),
        repo_root: root.join("repo"),
        storage_roots: RunStorageRoots::new(root.join("registries"), root.join("runs")),
        base_sha: Some("deadbeef".to_string()),
        budget: EvalBudget::default(),
        model_id: Some("model".to_string()),
        provider_slug: Some("provider".to_string()),
        campaign_id: Some("campaign-a".to_string()),
        batch_id: Some("batch-a".to_string()),
        run_arm_id: format!("{run_id}-arm"),
        run_role,
    };
    let registration = RunRegistration::register_with_run_id(intent, run_id).expect("registration");
    registration.persist().expect("persist registration");
    registration
}

fn write_protocol_artifact_envelope(
    registration: &RunRegistration,
    procedure_name: &str,
    created_at_ms: u64,
    model_id: Option<&str>,
    provider_slug: Option<&str>,
) -> std::path::PathBuf {
    write_protocol_artifact_envelope_with_payload(
        registration,
        procedure_name,
        created_at_ms,
        ProtocolEnvelopeOverrides {
            model_id,
            provider_slug,
            ..ProtocolEnvelopeOverrides::default()
        },
        serde_json::json!({"fixture": "ignored"}),
        serde_json::json!({"fixture": "ignored"}),
        serde_json::json!({"fixture": "ignored"}),
    )
}

#[derive(Default)]
struct ProtocolEnvelopeOverrides<'a> {
    schema_version: Option<&'a str>,
    run_id: Option<&'a str>,
    subject_id: Option<&'a str>,
    model_id: Option<&'a str>,
    provider_slug: Option<&'a str>,
}

fn write_protocol_artifact_envelope_with_payload(
    registration: &RunRegistration,
    procedure_name: &str,
    created_at_ms: u64,
    overrides: ProtocolEnvelopeOverrides<'_>,
    input: serde_json::Value,
    output: serde_json::Value,
    artifact: serde_json::Value,
) -> std::path::PathBuf {
    fs::create_dir_all(&registration.artifacts.protocol_artifacts_dir)
        .expect("protocol artifacts dir");
    let path = registration.artifacts.protocol_artifacts_dir.join(format!(
        "{created_at_ms}_{procedure_name}_{}.json",
        registration.frozen_spec.task_id
    ));
    let mut stored = serde_json::json!({
        "schema_version": overrides.schema_version.unwrap_or("protocol-artifact.v1"),
        "procedure_name": procedure_name,
        "subject_id": overrides.subject_id.unwrap_or(&registration.frozen_spec.task_id),
        "run_id": overrides.run_id.unwrap_or(&registration.run_id),
        "created_at_ms": created_at_ms,
        "input": input,
        "output": output,
        "artifact": artifact
    });
    if let Some(model_id) = overrides.model_id {
        stored["model_id"] = serde_json::json!(model_id);
    }
    if let Some(provider_slug) = overrides.provider_slug {
        stored["provider_slug"] = serde_json::json!(provider_slug);
    }
    fs::write(&path, stored.to_string()).expect("protocol artifact");
    path
}

fn write_evaluation_without_metrics(root: &Path, branch_id: &str) {
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
            "overall_disposition": "keep",
            "reasons": ["test"],
            "compared_instances": [{
                "instance_id": "instance-a",
                "baseline_record_path": "/tmp/baseline/record.json.gz",
                "treatment_record_path": "/tmp/treatment/record.json.gz",
                "baseline_metrics": null,
                "treatment_metrics": null,
                "evaluation": null,
                "status": "missing"
            }]
        })
        .to_string(),
    )
    .expect("evaluation");
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

fn empty_scores() -> ScoreSet {
    ScoreSet {
        schema_version: super::SCHEMA_VERSION.to_string(),
        procedure_id: super::PROCEDURE_ID.to_string(),
        profile: super::ScoreProfile::operational(),
        children: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn complete_score(node_id: &str, branch_id: &str, generation: u32, score: i64) -> ChildScore {
    ChildScore {
        node_id: node_id.to_string(),
        parent_node_id: Some("node-parent".to_string()),
        generation: Some(generation),
        branch_id: Some(branch_id.to_string()),
        state: ScoreState::Complete,
        score: Some(score),
        evaluations: Vec::new(),
        diagnostics: Vec::new(),
    }
}

fn selection_input(
    node_id: &str,
    branch_id: &str,
    generation: u32,
    disposition: BranchDisposition,
) -> SelectionInput {
    SelectionInput::new(
        CandidateRef {
            node_id: node_id.to_string(),
            branch_id: branch_id.to_string(),
            generation,
        },
        disposition,
        Path::new("evaluations/branch-a.json").to_path_buf(),
        vec![RunComparison {
            instance_id: "instance-a".to_string(),
            parent_metrics: Some(metrics(false, false, 3)),
            child_metrics: Some(metrics(true, true, 1)),
            status: "compared".to_string(),
        }],
    )
}

fn diagnostic_field(diagnostics: &[super::ScoreDiagnostic], field: &str) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.field == field)
}

fn diagnostic_message(diagnostics: &[super::ScoreDiagnostic], field: &str, message: &str) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.field == field && diagnostic.message.contains(message))
}

fn review_diagnostic_field(row: &super::ScoreSelectionRow, field: &str) -> bool {
    row.diagnostics
        .iter()
        .any(|diagnostic| diagnostic.field == field)
}
