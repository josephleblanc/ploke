use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ploke_protocol::{Confidence, IntentLabel, OverallVerdict, SegmentStatus};

use super::*;

#[test]
fn malformed_known_payload_returns_typed_failure_record() {
    let source = r#"{
        "schema_version": "protocol-artifact.v1",
        "procedure_name": "tool_call_review",
        "subject_id": "subject-1",
        "run_id": "run-1",
        "created_at_ms": 42,
        "model_id": "model-1",
        "provider_slug": "provider-1",
        "input": {},
        "output": {},
        "artifact": {}
    }"#;

    let failure =
        decode_artifact_str(source, None).expect_err("malformed payload should return failure");

    assert_eq!(
        failure.expected_payload_kind,
        Some(ArtifactPayloadKind::ToolCallReview)
    );
    let coordinate = failure.coordinate.expect("coordinate");
    assert_eq!(coordinate.schema_version, SCHEMA_V1);
    assert_eq!(coordinate.procedure_name, TOOL_CALL_REVIEW);
    assert_eq!(coordinate.subject_id, "subject-1");
    assert_eq!(coordinate.run_id, "run-1");
    assert_eq!(coordinate.created_at_ms, 42);
    assert_eq!(coordinate.model_id.as_deref(), Some("model-1"));
    assert_eq!(coordinate.provider_slug.as_deref(), Some("provider-1"));
    assert!(
        failure.error.contains("missing field"),
        "unexpected error: {}",
        failure.error
    );
}

#[test]
fn intervention_issue_detection_artifact_serializes_named_shape() {
    let artifact = InterventionIssueDetectionArtifact {
        case_count: 1,
        primary_issue: Some("request_code_context"),
    };

    let value = serde_json::to_value(&artifact).expect("serialize issue detection artifact");

    assert_eq!(
        value,
        serde_json::json!({
            "case_count": 1,
            "primary_issue": "request_code_context",
        })
    );
}

#[test]
fn intervention_issue_detection_artifact_decodes_as_loaded_non_tool_variant() {
    let source = serde_json::json!({
        "schema_version": SCHEMA_V1,
        "procedure_name": INTERVENTION_ISSUE_DETECTION,
        "subject_id": "subject-1",
        "run_id": "run-1",
        "created_at_ms": 42,
        "input": {
            "run_id": "run-1",
            "subject_id": "subject-1",
            "total_calls_in_run": 3,
            "anchor_segment_count": 1,
            "protocol_reviewed_call_count": 1,
            "protocol_reviewed_segment_count": 0,
            "protocol_artifact_count": 2
        },
        "output": {
            "cases": []
        },
        "artifact": {
            "case_count": 0
        }
    });
    let text = serde_json::to_string(&source).expect("serialize source");

    let artifact = decode_artifact_str(&text, None).expect("decode issue artifact");

    assert_eq!(artifact.procedure_name, INTERVENTION_ISSUE_DETECTION);
    match artifact.body {
        ArtifactBody::InterventionIssueDetection(payload) => {
            assert_eq!(payload.input.protocol_artifact_count, 2);
            assert!(payload.output.cases.is_empty());
            assert_eq!(payload.artifact.case_count, 0);
        }
        other => panic!("expected intervention issue detection payload, got {other:?}"),
    }
}

#[test]
fn intervention_apply_artifact_preserves_output_shape() {
    let output = serde_json::json!({
        "candidate_id": "candidate-1",
        "changed": true,
    });
    let artifact = InterventionApplyArtifact(&output);

    let value = serde_json::to_value(&artifact).expect("serialize apply artifact");

    assert_eq!(value, output);
}

#[test]
fn intervention_synthesis_artifact_mirror_accepts_private_value_shape() {
    let artifact_json = serde_json::json!({
        "procedure_name": "intervention_synthesis",
        "artifact": {
            "first": step(
                "contextualize_intervention_synthesis",
                synthesis_input(),
                synthesis_context(),
                mechanized_provenance("mechanized"),
            ),
            "second": {
                "branches": {
                    "input": {
                        "state": synthesis_context(),
                        "disposition": "record_and_forward",
                    },
                    "left_branch": "candidate_pair",
                    "right_branch": "stronger_rewrite",
                    "left": {
                        "input": {
                            "state": synthesis_context(),
                            "disposition": "record_and_forward",
                        },
                        "left_branch": "minimal_rewrite",
                        "right_branch": "decision_rule_rewrite",
                        "left": llm_draft_step("propose_minimal_tool_guidance_rewrite"),
                        "right": llm_draft_step("propose_decision_rule_tool_guidance_rewrite"),
                    },
                    "right": llm_draft_step("propose_stronger_tool_guidance_rewrite"),
                },
                "merge": step(
                    "assemble_intervention_candidates",
                    draft_triplet(),
                    synthesis_output(),
                    mechanized_provenance("mechanized"),
                ),
            },
        },
    });

    let artifact: InterventionSynthesisArtifactMirror =
        serde_json::from_value(artifact_json.clone()).expect("deserialize synthesis artifact");
    let serialized = serde_json::to_value(&artifact).expect("serialize synthesis artifact");

    assert_eq!(artifact.procedure_name, "intervention_synthesis");
    assert_eq!(
        serialized["artifact"]["first"]["output"]["target_relpath"],
        serde_json::json!("crates/ploke-core/tool_text/request_code_context.md")
    );
    assert_eq!(
        serialized["artifact"]["second"]["merge"]["output"]["candidate_set"]["candidates"][0]["candidate_id"],
        serde_json::json!("candidate-1")
    );
}

#[test]
fn all_artifacts_roundtrip() {
    let dir = protocol_artifacts_dir();
    let mut paths = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("read protocol-artifacts dir {}: {err}", dir.display()))
        .map(|entry| entry.expect("read protocol entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();

    let mut counts = BTreeMap::<String, usize>::new();

    for path in &paths {
        let original_text = read_json_text(path);
        let original: serde_json::Value = serde_json::from_str(&original_text)
            .unwrap_or_else(|err| panic!("parse protocol artifact {}: {err}", path.display()));
        let artifact =
            decode_artifact_str(&original_text, Some(path.clone())).unwrap_or_else(|err| {
                panic!("deserialize protocol artifact {}: {err:?}", path.display())
            });
        let serialized = serde_json::to_value(&artifact)
            .unwrap_or_else(|err| panic!("serialize protocol artifact {}: {err}", path.display()));
        assert_eq!(
            serialized,
            original,
            "round-trip mismatch for {}",
            path.display()
        );

        *counts.entry(artifact.procedure_name.clone()).or_default() += 1;
    }

    assert_eq!(paths.len(), 3, "unexpected protocol artifact count");
    assert_eq!(
        counts
            .get(TOOL_CALL_INTENT_SEGMENTATION)
            .copied()
            .unwrap_or(0),
        1
    );
    assert_eq!(counts.get(TOOL_CALL_REVIEW).copied().unwrap_or(0), 1);
    assert_eq!(
        counts.get(TOOL_CALL_SEGMENT_REVIEW).copied().unwrap_or(0),
        1
    );
}

#[test]
fn typed_payloads() {
    let paths = protocol_artifact_paths();
    let segmentation = paths
        .iter()
        .find(|path| {
            path.to_string_lossy()
                .contains(TOOL_CALL_INTENT_SEGMENTATION)
        })
        .expect("segmentation artifact path");
    let review = paths
        .iter()
        .find(|path| path.to_string_lossy().contains(TOOL_CALL_REVIEW))
        .expect("review artifact path");

    let segmentation_text = read_json_text(segmentation);
    let segmentation_artifact = decode_artifact_str(&segmentation_text, Some(segmentation.clone()))
        .unwrap_or_else(|err| {
            panic!(
                "deserialize segmentation {}: {err:?}",
                segmentation.display()
            )
        });

    assert_eq!(segmentation_artifact.schema_version, SCHEMA_V1);
    assert_eq!(
        segmentation_artifact.procedure_name,
        TOOL_CALL_INTENT_SEGMENTATION
    );
    let segmentation_payload = match segmentation_artifact.body() {
        Some(ArtifactBody::ToolCallIntentSegmentation(payload)) => payload,
        other => panic!("expected segmentation payload, got {other:?}"),
    };
    assert_eq!(segmentation_payload.input.total_calls_in_run, 2);
    assert_eq!(segmentation_payload.output.coverage.total_calls, 2);
    assert_eq!(segmentation_payload.output.segments.len(), 1);
    assert_eq!(
        segmentation_payload.artifact.procedure_name,
        TOOL_CALL_INTENT_SEGMENTATION
    );

    let review_text = read_json_text(review);
    let review_artifact = decode_artifact_str(&review_text, Some(review.clone()))
        .unwrap_or_else(|err| panic!("deserialize review {}: {err:?}", review.display()));
    assert_eq!(review_artifact.procedure_name, TOOL_CALL_REVIEW);
    let review_payload = match review_artifact.typed_payload() {
        Some(ArtifactBody::ToolCallReview(payload)) => payload,
        other => panic!("expected review payload, got {other:?}"),
    };
    assert_eq!(review_payload.output.overall, OverallVerdict::Mixed);
    assert_eq!(review_payload.output.overall_confidence, Confidence::High);
    assert_eq!(review_payload.output.packet.total_calls_in_run, 2);
    assert_eq!(review_payload.output.packet.total_calls_in_scope, 2);
    assert_eq!(review_payload.output.packet.calls.len(), 2);
    assert_eq!(review_payload.artifact.procedure_name, TOOL_CALL_REVIEW);
    let branch_artifacts = &review_payload.artifact.artifact.second.branches;
    assert_eq!(
        branch_artifacts.left.left.provenance.model_id,
        "fixture-model"
    );
    assert_eq!(
        branch_artifacts.left.left.provenance.response.usage,
        Some(TokenUsageMirror {
            prompt_tokens: 9,
            completion_tokens: 6,
            total_tokens: 15,
        })
    );

    let segment_review = paths
        .iter()
        .find(|path| path.to_string_lossy().contains(TOOL_CALL_SEGMENT_REVIEW))
        .expect("segment review artifact path");
    let segment_review_text = read_json_text(segment_review);
    let segment_review_artifact =
        decode_artifact_str(&segment_review_text, Some(segment_review.clone())).unwrap_or_else(
            |err| {
                panic!(
                    "deserialize segment review {}: {err:?}",
                    segment_review.display()
                )
            },
        );
    let segment_review_payload = match segment_review_artifact.body() {
        Some(ArtifactBody::ToolCallSegmentReview(payload)) => payload,
        other => panic!("expected segment-review payload, got {other:?}"),
    };
    assert_eq!(segment_review_payload.input.segment.start_index, 0);
    assert_eq!(segment_review_payload.input.segment.end_index, 1);
    assert_eq!(segment_review_payload.output.packet.segment_index, Some(0));
    assert_eq!(
        segment_review_payload.output.packet.segment_status,
        Some(SegmentStatus::Labeled)
    );
    assert_eq!(
        segment_review_payload.output.packet.segment_label,
        Some(IntentLabel::InspectCandidate)
    );
    assert_eq!(
        segment_review_payload.artifact.procedure_name,
        TOOL_CALL_SEGMENT_REVIEW
    );
}

fn step(
    step_id: &str,
    input: serde_json::Value,
    output: serde_json::Value,
    provenance: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "step_id": step_id,
        "step_name": step_id,
        "executor_kind": "mechanized",
        "executor_label": "mechanized",
        "evidence_policy": {
            "allowed": [],
            "forbidden": [],
            "hindsight_allowed": false,
            "external_context_allowed": false,
        },
        "input": input,
        "input_disposition": "record_and_forward",
        "output": output,
        "output_disposition": "record_and_forward",
        "provenance": provenance,
    })
}

fn llm_draft_step(step_id: &str) -> serde_json::Value {
    serde_json::json!({
        "step_id": step_id,
        "step_name": step_id,
        "executor_kind": "llm_adjudicator",
        "executor_label": "json_adjudicator",
        "evidence_policy": {
            "allowed": [],
            "forbidden": [],
            "hindsight_allowed": false,
            "external_context_allowed": false,
        },
        "input": synthesis_context(),
        "input_disposition": "record_and_forward",
        "output": draft(),
        "output_disposition": "record_and_forward",
        "provenance": llm_provenance(),
    })
}

fn mechanized_provenance(strategy: &str) -> serde_json::Value {
    serde_json::json!({
        "strategy": strategy,
    })
}

fn llm_provenance() -> serde_json::Value {
    serde_json::json!({
        "model_id": "model-1",
        "provider_slug": "provider-1",
        "raw_content": "{}",
        "reasoning": null,
        "response": {
            "id": "response-1",
            "choices": [],
            "created": 0,
            "model": "model-1",
            "object": "chat.completion",
            "provider": null,
            "system_fingerprint": null,
            "usage": null,
        },
    })
}

fn synthesis_input() -> serde_json::Value {
    serde_json::json!({
        "issue": issue_case(),
        "source_state_id": "source-1",
        "source_content": "old",
        "operation_target": {
            "kind": "artifact",
            "artifact_id": "artifact:A1",
        },
    })
}

fn synthesis_context() -> serde_json::Value {
    serde_json::json!({
        "issue": issue_case(),
        "source_state_id": "source-1",
        "source_content": "old",
        "target_relpath": "crates/ploke-core/tool_text/request_code_context.md",
        "operation_target": {
            "kind": "artifact",
            "artifact_id": "artifact:A1",
        },
    })
}

fn synthesis_output() -> serde_json::Value {
    serde_json::json!({
        "candidate_set": {
            "source_state_id": "source-1",
            "target_relpath": "crates/ploke-core/tool_text/request_code_context.md",
            "source_content": "old",
            "candidates": [
                {
                    "candidate_id": "candidate-1",
                    "branch_label": "minimal_rewrite",
                    "proposed_content": "new",
                    "spec": {
                        "kind": "tool_guidance_mutation",
                        "spec_id": "reviewed-tool:request_code_context:minimal_rewrite",
                        "evidence_basis": "basis",
                        "intended_effect": "effect",
                        "tool": "request_code_context",
                        "edit": {
                            "kind": "replace_whole_text",
                            "new_text": "new",
                        },
                        "validation_policy": validation_policy(),
                    },
                    "patch_id": "patch:P1",
                }
            ],
            "operation_target": {
                "kind": "artifact",
                "artifact_id": "artifact:A1",
            },
        },
    })
}

fn draft_triplet() -> serde_json::Value {
    serde_json::json!({
        "source": synthesis_context(),
        "left": {
            "source": synthesis_context(),
            "left": draft(),
            "right": draft(),
        },
        "right": draft(),
    })
}

fn draft() -> serde_json::Value {
    serde_json::json!({
        "proposed_content": "new",
        "intended_effect": "effect",
        "rationale": "rationale",
    })
}

fn issue_case() -> serde_json::Value {
    serde_json::json!({
        "selection_basis": "protocol_reviewed_issue_calls",
        "target_tool": "request_code_context",
        "evidence": {
            "reviewed_call_count": 2,
            "reviewed_issue_call_count": 1,
            "protocol": {
                "reviewed_call_indices": [0, 1],
                "reviewed_segment_indices": [0],
                "candidate_concerns": ["redundant"],
                "nearby_segment_labels": ["inspect_candidate"],
            },
        },
    })
}

fn validation_policy() -> serde_json::Value {
    serde_json::json!({
        "allowed_relpaths": ["crates/ploke-core/tool_text/request_code_context.md"],
        "require_target_exists": true,
        "require_nonempty_result": true,
        "require_utf8": true,
        "require_content_change": true,
        "require_markers_after_apply": [],
        "require_cargo_check": false,
    })
}

fn protocol_artifact_paths() -> Vec<PathBuf> {
    let dir = protocol_artifacts_dir();
    let mut paths = fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("read protocol-artifacts dir {}: {err}", dir.display()))
        .map(|entry| entry.expect("read protocol entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn protocol_artifacts_dir() -> PathBuf {
    crate::test_fixtures::protocol_artifacts_dir()
}

fn read_json_text(path: &Path) -> String {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("read protocol artifact {}: {err}", path.display()));
    text
}
