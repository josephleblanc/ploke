use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ploke_protocol::{Confidence, IntentLabel, OverallVerdict, SegmentStatus};

use super::*;

#[test]
#[ignore]
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
        let original = read_json_value(path);
        let artifact: Artifact = serde_json::from_value(original.clone()).unwrap_or_else(|err| {
            panic!("deserialize protocol artifact {}: {err}", path.display())
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

    assert_eq!(paths.len(), 16, "unexpected protocol artifact count");
    assert_eq!(
        counts
            .get(TOOL_CALL_INTENT_SEGMENTATION)
            .copied()
            .unwrap_or(0),
        1
    );
    assert_eq!(counts.get(TOOL_CALL_REVIEW).copied().unwrap_or(0), 10);
    assert_eq!(
        counts.get(TOOL_CALL_SEGMENT_REVIEW).copied().unwrap_or(0),
        5
    );
}

#[test]
#[ignore]
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

    let segmentation_value = read_json_value(segmentation);
    let segmentation_artifact: Artifact = serde_json::from_value(segmentation_value)
        .unwrap_or_else(|err| panic!("deserialize segmentation {}: {err}", segmentation.display()));

    assert_eq!(segmentation_artifact.schema_version, SCHEMA_V1);
    assert_eq!(
        segmentation_artifact.procedure_name,
        TOOL_CALL_INTENT_SEGMENTATION
    );
    let segmentation_payload = match segmentation_artifact.body() {
        Some(ArtifactBody::ToolCallIntentSegmentation(payload)) => payload,
        other => panic!("expected segmentation payload, got {other:?}"),
    };
    assert_eq!(segmentation_payload.input.total_calls_in_run, 10);
    assert_eq!(segmentation_payload.output.coverage.total_calls, 10);
    assert_eq!(segmentation_payload.output.segments.len(), 5);
    assert_eq!(
        segmentation_payload.artifact.procedure_name,
        TOOL_CALL_INTENT_SEGMENTATION
    );

    let review_value = read_json_value(review);
    let review_artifact: Artifact = serde_json::from_value(review_value)
        .unwrap_or_else(|err| panic!("deserialize review {}: {err}", review.display()));
    assert_eq!(review_artifact.procedure_name, TOOL_CALL_REVIEW);
    let review_payload = match review_artifact.typed_payload() {
        Some(ArtifactBody::ToolCallReview(payload)) => payload,
        other => panic!("expected review payload, got {other:?}"),
    };
    assert_eq!(review_payload.output.overall, OverallVerdict::Mixed);
    assert_eq!(review_payload.output.overall_confidence, Confidence::High);
    assert_eq!(review_payload.output.packet.total_calls_in_run, 10);
    assert_eq!(review_payload.output.packet.total_calls_in_scope, 3);
    assert_eq!(review_payload.output.packet.calls.len(), 3);
    assert_eq!(review_payload.artifact.procedure_name, TOOL_CALL_REVIEW);
    let branch_artifacts = &review_payload.artifact.artifact.second.branches;
    assert_eq!(
        branch_artifacts.left.left.provenance.model_id,
        "x-ai/grok-4-fast"
    );
    assert_eq!(
        branch_artifacts.left.left.provenance.response.usage,
        Some(TokenUsageMirror {
            prompt_tokens: 944,
            completion_tokens: 665,
            total_tokens: 1609,
        })
    );

    let segment_review = paths
        .iter()
        .find(|path| path.to_string_lossy().contains(TOOL_CALL_SEGMENT_REVIEW))
        .expect("segment review artifact path");
    let segment_review_value = read_json_value(segment_review);
    let segment_review_artifact: Artifact = serde_json::from_value(segment_review_value)
        .unwrap_or_else(|err| {
            panic!(
                "deserialize segment review {}: {err}",
                segment_review.display()
            )
        });
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

#[test]
#[ignore]
fn print_segmentation_roundtrip() {
    let path = protocol_artifact_paths()
        .into_iter()
        .find(|path| {
            path.to_string_lossy()
                .contains(TOOL_CALL_INTENT_SEGMENTATION)
        })
        .expect("segmentation artifact path");
    let original = read_json_value(&path);

    println!(
        "before deserialize:\n{}",
        serde_json::to_string_pretty(&segmentation_probe_from_value(&original))
            .expect("format original probe")
    );

    let artifact: Artifact = serde_json::from_value(original.clone()).unwrap_or_else(|err| {
        panic!(
            "deserialize segmentation artifact {}: {err}",
            path.display()
        )
    });
    println!(
        "after deserialize:\n{}",
        serde_json::to_string_pretty(&segmentation_probe_from_artifact(&artifact))
            .expect("format artifact probe")
    );

    let serialized = serde_json::to_value(&artifact).expect("serialize segmentation artifact");
    println!(
        "after serialize again:\n{}",
        serde_json::to_string_pretty(&segmentation_probe_from_value(&serialized))
            .expect("format serialized probe")
    );

    assert_eq!(serialized, original);
}

fn segmentation_probe_from_value(value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": value.get("schema_version"),
        "procedure_name": value.get("procedure_name"),
        "input": {
            "total_calls_in_run": value.get("input").and_then(|input| input.get("total_calls_in_run")),
        },
        "output": {
            "coverage_total_calls": value.get("output")
                .and_then(|output| output.get("coverage"))
                .and_then(|coverage| coverage.get("total_calls")),
            "segments_len": value.get("output")
                .and_then(|output| output.get("segments"))
                .and_then(serde_json::Value::as_array)
                .map(Vec::len),
        }
    })
}

fn segmentation_probe_from_artifact(artifact: &Artifact) -> serde_json::Value {
    let payload = match artifact.body() {
        Some(ArtifactBody::ToolCallIntentSegmentation(payload)) => payload,
        other => panic!("expected segmentation payload, got {other:?}"),
    };
    serde_json::json!({
        "schema_version": artifact.schema_version,
        "procedure_name": artifact.procedure_name,
        "input": {
            "total_calls_in_run": payload.input.total_calls_in_run,
        },
        "output": {
            "coverage_total_calls": payload.output.coverage.total_calls,
            "segments_len": payload.output.segments.len(),
        }
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
    std::env::var_os("PLOKE_RECORDS_REAL_PROTOCOL_ARTIFACTS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                "/home/brasides/.ploke-eval/instances/prototype1/p1-edit-surface-history-long-20260508-1/treatments/branch-01187cd17226d1a4/instances/BurntSushi__ripgrep-2209/runs/run-1778270575083-structured-current-policy-7a8e5b98/protocol-artifacts",
            )
        })
}

fn read_json_value(path: &Path) -> serde_json::Value {
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!(
            "read protocol artifact {} (set PLOKE_RECORDS_REAL_PROTOCOL_ARTIFACTS_DIR to override): {err}",
            path.display()
        )
    });
    serde_json::from_str(&text).expect("parse protocol artifact JSON value")
}
