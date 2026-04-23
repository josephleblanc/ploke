use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::layout::protocol_artifacts_dir_for_run;
use crate::run_registry::{
    ResolvedProtocolRunIdentity, resolve_protocol_run_identity, sync_protocol_registration_status,
};
use crate::spec::PrepareError;

pub const PROTOCOL_ARTIFACT_SCHEMA_VERSION: &str = "protocol-artifact.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProtocolArtifact {
    pub schema_version: String,
    pub procedure_name: String,
    pub subject_id: String,
    pub run_id: String,
    pub created_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    pub input: Value,
    pub output: Value,
    pub artifact: Value,
}

#[derive(Debug, Clone)]
pub struct StoredProtocolArtifactFile {
    pub path: PathBuf,
    pub stored: StoredProtocolArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProtocolArtifactIdentityMismatch {
    pub path: PathBuf,
    pub procedure_name: String,
    pub field: &'static str,
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for ProtocolArtifactIdentityMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "protocol artifact '{}' for procedure '{}' has {}='{}' but expected '{}'",
            self.path.display(),
            self.procedure_name,
            self.field,
            self.actual,
            self.expected
        )
    }
}

pub(crate) fn protocol_artifact_summary(entry: &StoredProtocolArtifactFile) -> String {
    match entry.stored.procedure_name.as_str() {
        "tool_call_review" => {
            let focal = entry
                .stored
                .output
                .get("neighborhood")
                .and_then(|v| v.get("focal"))
                .and_then(|v| v.get("index"))
                .and_then(|v| v.as_u64())
                .map(|idx| format!("focal={idx}"));
            let overall = entry
                .stored
                .output
                .get("overall")
                .and_then(|v| v.as_str())
                .map(|value| format!("overall={value}"));
            let confidence = entry
                .stored
                .output
                .get("overall_confidence")
                .and_then(|v| v.as_str())
                .map(|value| format!("confidence={value}"));
            let summary = [focal, overall, confidence]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if summary.is_empty() {
                "local review artifact".to_string()
            } else {
                summary
            }
        }
        "tool_call_intent_segmentation" => {
            let segments = entry
                .stored
                .output
                .get("segments")
                .and_then(|v| v.as_array())
                .map(|segments| format!("segments={}", segments.len()));
            let uncovered = entry
                .stored
                .output
                .get("uncovered_call_indices")
                .and_then(|v| v.as_array())
                .map(|indices| format!("uncovered={}", indices.len()));
            let summary = [segments, uncovered]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if summary.is_empty() {
                "intent segmentation artifact".to_string()
            } else {
                summary
            }
        }
        "tool_call_segment_review" => {
            let target = entry
                .stored
                .output
                .get("packet")
                .and_then(|v| v.get("target_id"))
                .and_then(|v| v.as_str())
                .map(|value| format!("target={value}"));
            let overall = entry
                .stored
                .output
                .get("overall")
                .and_then(|v| v.as_str())
                .map(|value| format!("overall={value}"));
            let confidence = entry
                .stored
                .output
                .get("overall_confidence")
                .and_then(|v| v.as_str())
                .map(|value| format!("confidence={value}"));
            let summary = [target, overall, confidence]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if summary.is_empty() {
                "segment review artifact".to_string()
            } else {
                summary
            }
        }
        "intervention_issue_detection" => {
            let case_count = entry
                .stored
                .output
                .get("cases")
                .and_then(|v| v.as_array())
                .map(|cases| format!("cases={}", cases.len()));
            let primary = entry
                .stored
                .output
                .get("cases")
                .and_then(|v| v.as_array())
                .and_then(|cases| cases.first())
                .and_then(|case| case.get("target_tool"))
                .and_then(|v| v.as_str())
                .map(|value| format!("primary={value}"));
            let summary = [case_count, primary]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if summary.is_empty() {
                "issue detection artifact".to_string()
            } else {
                summary
            }
        }
        "intervention_synthesis" => {
            let candidate_count = entry
                .stored
                .output
                .get("candidate_set")
                .and_then(|v| v.get("candidates"))
                .and_then(|v| v.as_array())
                .map(|candidates| format!("candidates={}", candidates.len()));
            let target = entry
                .stored
                .output
                .get("candidate_set")
                .and_then(|v| v.get("target_relpath"))
                .and_then(|v| v.as_str())
                .map(|value| format!("target={value}"));
            let summary = [candidate_count, target]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if summary.is_empty() {
                "intervention synthesis artifact".to_string()
            } else {
                summary
            }
        }
        "intervention_apply" => {
            let candidate = entry
                .stored
                .output
                .get("candidate_id")
                .and_then(|v| v.as_str())
                .map(|value| format!("candidate={value}"));
            let changed = entry
                .stored
                .output
                .get("changed")
                .and_then(|v| v.as_bool())
                .map(|value| format!("changed={value}"));
            let summary = [candidate, changed]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
            if summary.is_empty() {
                "intervention apply artifact".to_string()
            } else {
                summary
            }
        }
        _ => "persisted protocol artifact".to_string(),
    }
}

pub(crate) fn protocol_artifact_preview(value: &Value) -> String {
    preview_json_value(value, 2, 4, 220)
}

pub fn write_protocol_artifact<Input, Output, Artifact>(
    record_path: &Path,
    procedure_name: &str,
    subject_id: &str,
    model_id: Option<&str>,
    provider_slug: Option<&str>,
    input: &Input,
    output: &Output,
    artifact: &Artifact,
) -> Result<PathBuf, PrepareError>
where
    Input: Serialize,
    Output: Serialize,
    Artifact: Serialize,
{
    let resolved_identity = resolve_protocol_run_identity(record_path)?;
    if subject_id != resolved_identity.subject_id {
        return Err(PrepareError::DatabaseSetup {
            phase: "protocol_artifact_identity",
            detail: format!(
                "refusing to write protocol artifact for run '{}' with subject_id='{}'; resolved subject_id is '{}'",
                resolved_identity.run_id, subject_id, resolved_identity.subject_id
            ),
        });
    }
    let artifacts_dir = protocol_artifacts_dir_for_run(&resolved_identity.run_dir);
    fs::create_dir_all(&artifacts_dir).map_err(|source| {
        PrepareError::CreateProtocolArtifactDir {
            path: artifacts_dir.clone(),
            source,
        }
    })?;

    let created_at_ms = now_millis();
    let file_name = format!(
        "{}_{}_{}.json",
        created_at_ms,
        sanitize_component(procedure_name),
        sanitize_component(subject_id)
    );
    let path = artifacts_dir.join(file_name);
    let stored = StoredProtocolArtifact {
        schema_version: PROTOCOL_ARTIFACT_SCHEMA_VERSION.to_string(),
        procedure_name: procedure_name.to_string(),
        subject_id: resolved_identity.subject_id.clone(),
        run_id: resolved_identity.run_id,
        created_at_ms,
        model_id: model_id.map(str::to_string),
        provider_slug: provider_slug.map(str::to_string),
        input: serde_json::to_value(input).map_err(PrepareError::SerializeProtocolArtifact)?,
        output: serde_json::to_value(output).map_err(PrepareError::SerializeProtocolArtifact)?,
        artifact: serde_json::to_value(artifact)
            .map_err(PrepareError::SerializeProtocolArtifact)?,
    };
    let serialized =
        serde_json::to_string_pretty(&stored).map_err(PrepareError::SerializeProtocolArtifact)?;
    fs::write(&path, serialized).map_err(|source| PrepareError::WriteProtocolArtifact {
        path: path.clone(),
        source,
    })?;
    sync_protocol_registration_status(record_path)?;
    Ok(path)
}

pub fn list_protocol_artifacts(
    record_path: &Path,
) -> Result<Vec<StoredProtocolArtifactFile>, PrepareError> {
    let resolved_identity = resolve_protocol_run_identity(record_path)?;
    let artifacts_dir = protocol_artifacts_dir_for_run(&resolved_identity.run_dir);
    if !artifacts_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    for entry in
        fs::read_dir(&artifacts_dir).map_err(|source| PrepareError::ReadProtocolArtifact {
            path: artifacts_dir.clone(),
            source,
        })?
    {
        let entry = entry.map_err(|source| PrepareError::ReadProtocolArtifact {
            path: artifacts_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let loaded = load_protocol_artifact(&path)?;
        validate_protocol_artifact_identity(&loaded, &resolved_identity).map_err(|mismatch| {
            PrepareError::DatabaseSetup {
                phase: "protocol_artifact_identity",
                detail: mismatch.to_string(),
            }
        })?;
        entries.push(loaded);
    }

    entries.sort_by(|left, right| right.path.cmp(&left.path));
    Ok(entries)
}

pub fn load_protocol_artifact(path: &Path) -> Result<StoredProtocolArtifactFile, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadProtocolArtifact {
        path: path.to_path_buf(),
        source,
    })?;
    let stored =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseProtocolArtifact {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(StoredProtocolArtifactFile {
        path: path.to_path_buf(),
        stored,
    })
}

pub(crate) fn validate_protocol_artifact_identity(
    entry: &StoredProtocolArtifactFile,
    resolved_identity: &ResolvedProtocolRunIdentity,
) -> Result<(), ProtocolArtifactIdentityMismatch> {
    validate_protocol_artifact_field(
        entry,
        "stored.run_id",
        &entry.stored.run_id,
        &resolved_identity.run_id,
    )?;
    validate_protocol_artifact_field(
        entry,
        "stored.subject_id",
        &entry.stored.subject_id,
        &resolved_identity.subject_id,
    )?;

    if let Some(output_subject_id) = protocol_output_subject_id(entry) {
        validate_protocol_artifact_field(
            entry,
            "output.subject_id",
            output_subject_id,
            &resolved_identity.subject_id,
        )?;
    }

    Ok(())
}

fn validate_protocol_artifact_field(
    entry: &StoredProtocolArtifactFile,
    field: &'static str,
    actual: &str,
    expected: &str,
) -> Result<(), ProtocolArtifactIdentityMismatch> {
    if actual == expected {
        return Ok(());
    }

    Err(ProtocolArtifactIdentityMismatch {
        path: entry.path.clone(),
        procedure_name: entry.stored.procedure_name.clone(),
        field,
        expected: expected.to_string(),
        actual: actual.to_string(),
    })
}

fn protocol_output_subject_id(entry: &StoredProtocolArtifactFile) -> Option<&str> {
    match entry.stored.procedure_name.as_str() {
        "tool_call_intent_segmentation" => entry
            .stored
            .output
            .get("sequence")
            .and_then(|value| value.get("subject_id"))
            .and_then(Value::as_str),
        "tool_call_review" | "tool_call_segment_review" => entry
            .stored
            .output
            .get("packet")
            .and_then(|value| value.get("subject_id"))
            .and_then(Value::as_str),
        _ => None,
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn sanitize_component(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
            output.push(ch);
        } else {
            output.push('_');
        }
    }

    let sanitized = output.trim_matches('_');
    if sanitized.is_empty() {
        "artifact".to_string()
    } else {
        sanitized.to_string()
    }
}

fn preview_json_value(value: &Value, depth: usize, max_items: usize, max_len: usize) -> String {
    let rendered = match value {
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => preview_json_string(text, max_len),
        Value::Array(items) => {
            if depth == 0 {
                format!(
                    "[{} item{}]",
                    items.len(),
                    if items.len() == 1 { "" } else { "s" }
                )
            } else {
                let mut parts = Vec::new();
                for item in items.iter().take(max_items) {
                    parts.push(preview_json_value(item, depth - 1, max_items, 64));
                }
                if items.len() > max_items {
                    parts.push("...".to_string());
                }
                format!("[{}]", parts.join(", "))
            }
        }
        Value::Object(map) => {
            if depth == 0 {
                format!(
                    "{{{} key{}}}",
                    map.len(),
                    if map.len() == 1 { "" } else { "s" }
                )
            } else {
                let mut parts = Vec::new();
                for (key, value) in map.iter().take(max_items) {
                    parts.push(format!(
                        "{}={}",
                        key,
                        preview_json_value(value, depth - 1, max_items, 64)
                    ));
                }
                if map.len() > max_items {
                    parts.push("...".to_string());
                }
                format!("{{{}}}", parts.join(", "))
            }
        }
    };

    if rendered.chars().count() > max_len {
        truncate_middle(&rendered, max_len)
    } else {
        rendered
    }
}

fn preview_json_string(text: &str, max_len: usize) -> String {
    let mut preview = text.replace('\n', "\\n");
    if preview.chars().count() > max_len {
        preview = truncate_middle(&preview, max_len);
    }
    format!("{preview:?}")
}

fn truncate_middle(text: &str, max_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let front = (max_len - 3) / 2;
    let back = max_len - 3 - front;
    format!(
        "{}...{}",
        chars[..front].iter().collect::<String>(),
        chars[chars.len() - back..].iter().collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_json_value_is_bounded() {
        let value = serde_json::json!({
            "procedure_name": "tool_call_review",
            "output": {
                "overall": "useful",
                "overall_confidence": "high",
                "neighborhood": {
                    "focal": {
                        "index": 42,
                        "tool_name": "search_code",
                    }
                }
            },
            "artifact": {
                "nested": {
                    "payload": "x".repeat(400),
                }
            }
        });

        let preview = protocol_artifact_preview(&value);
        assert!(preview.chars().count() <= 220);
        assert!(preview.contains("procedure_name"));
        assert!(preview.contains("overall"));
        assert!(!preview.contains(&"x".repeat(120)));
    }

    #[test]
    fn summary_prefers_known_procedure_signals() {
        let stored = StoredProtocolArtifact {
            schema_version: PROTOCOL_ARTIFACT_SCHEMA_VERSION.to_string(),
            procedure_name: "tool_call_intent_segmentation".to_string(),
            subject_id: "subject".to_string(),
            run_id: "run".to_string(),
            created_at_ms: 0,
            model_id: None,
            provider_slug: None,
            input: serde_json::json!({}),
            output: serde_json::json!({
                "segments": [1, 2, 3],
                "uncovered_call_indices": [9],
            }),
            artifact: serde_json::json!({}),
        };
        let entry = StoredProtocolArtifactFile {
            path: PathBuf::from("artifact.json"),
            stored,
        };

        assert_eq!(protocol_artifact_summary(&entry), "segments=3 uncovered=1");
    }
}
