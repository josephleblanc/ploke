//! Passive persisted protocol artifact DTOs.
//!
//! These records mirror persisted files under run-local
//! `protocol-artifacts/*.json` directories.
//! They do not evaluate procedures or enforce runtime authority.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::value::JsonRecordValue;

pub const SCHEMA_V1: &str = "protocol-artifact.v1";
pub const TOOL_CALL_INTENT_SEGMENTATION: &str = "tool_call_intent_segmentation";
pub const TOOL_CALL_REVIEW: &str = "tool_call_review";
pub const TOOL_CALL_SEGMENT_REVIEW: &str = "tool_call_segment_review";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Artifact {
    pub schema_version: String,
    pub procedure_name: String,
    pub subject_id: String,
    pub run_id: String,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    pub input: JsonRecordValue,
    pub output: JsonRecordValue,
    pub artifact: JsonRecordValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactFile {
    pub path: PathBuf,
    pub artifact: Artifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub total_calls: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Procedure {
    pub input_total_calls_in_run: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Review {
    pub overall: String,
    pub overall_confidence: String,
}

impl Artifact {
    pub fn segmentation_probe(&self) -> Option<(Procedure, Coverage, Segment)> {
        if self.procedure_name != TOOL_CALL_INTENT_SEGMENTATION {
            return None;
        }
        let input_total_calls_in_run =
            json_get_u64(&self.input, &["total_calls_in_run"]).unwrap_or_default();
        let total_calls = json_get_u64(&self.output, &["coverage", "total_calls"])?;
        let count = json_get_array_len(&self.output, &["segments"])?;
        Some((
            Procedure {
                input_total_calls_in_run,
            },
            Coverage { total_calls },
            Segment { count },
        ))
    }

    pub fn review_probe(&self) -> Option<Review> {
        if self.procedure_name != TOOL_CALL_REVIEW {
            return None;
        }
        Some(Review {
            overall: json_get_str(&self.output, &["overall"])?.to_string(),
            overall_confidence: json_get_str(&self.output, &["overall_confidence"])?.to_string(),
        })
    }
}

fn json_get<'a>(value: &'a JsonRecordValue, path: &[&str]) -> Option<&'a JsonRecordValue> {
    let mut current = value;
    for key in path {
        let JsonRecordValue::Object(object) = current else {
            return None;
        };
        current = object.get(*key)?;
    }
    Some(current)
}

fn json_get_u64(value: &JsonRecordValue, path: &[&str]) -> Option<u64> {
    match json_get(value, path)? {
        JsonRecordValue::U64(v) => Some(*v),
        JsonRecordValue::I64(v) if *v >= 0 => Some(*v as u64),
        _ => None,
    }
}

fn json_get_str<'a>(value: &'a JsonRecordValue, path: &[&str]) -> Option<&'a str> {
    match json_get(value, path)? {
        JsonRecordValue::String(v) => Some(v.as_str()),
        _ => None,
    }
}

fn json_get_array_len(value: &JsonRecordValue, path: &[&str]) -> Option<usize> {
    match json_get(value, path)? {
        JsonRecordValue::Array(v) => Some(v.len()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    #[ignore]
    fn real_run_all_protocol_artifacts_roundtrip_and_counts() {
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
            let artifact: Artifact =
                serde_json::from_value(original.clone()).unwrap_or_else(|err| {
                    panic!("deserialize protocol artifact {}: {err}", path.display())
                });
            let serialized = serde_json::to_value(&artifact).unwrap_or_else(|err| {
                panic!("serialize protocol artifact {}: {err}", path.display())
            });
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
    fn real_run_segmentation_and_review_probes() {
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
            .unwrap_or_else(|err| {
                panic!("deserialize segmentation {}: {err}", segmentation.display())
            });

        assert_eq!(segmentation_artifact.schema_version, SCHEMA_V1);
        assert_eq!(
            segmentation_artifact.procedure_name,
            TOOL_CALL_INTENT_SEGMENTATION
        );
        let (procedure, coverage, segment) = segmentation_artifact
            .segmentation_probe()
            .expect("segmentation probe");
        assert_eq!(procedure.input_total_calls_in_run, 10);
        assert_eq!(coverage.total_calls, 10);
        assert_eq!(segment.count, 5);

        let review_value = read_json_value(review);
        let review_artifact: Artifact = serde_json::from_value(review_value)
            .unwrap_or_else(|err| panic!("deserialize review {}: {err}", review.display()));
        assert_eq!(review_artifact.procedure_name, TOOL_CALL_REVIEW);
        let review_probe = review_artifact.review_probe().expect("review probe");
        assert_eq!(review_probe.overall, "mixed");
        assert_eq!(review_probe.overall_confidence, "high");
    }

    #[test]
    #[ignore]
    fn print_segmentation_roundtrip_probe() {
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
        let (procedure, coverage, segment) = artifact
            .segmentation_probe()
            .expect("segmentation probe from artifact");
        serde_json::json!({
            "schema_version": artifact.schema_version,
            "procedure_name": artifact.procedure_name,
            "input": {
                "total_calls_in_run": procedure.input_total_calls_in_run,
            },
            "output": {
                "coverage_total_calls": coverage.total_calls,
                "segments_len": segment.count,
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
}
