use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ploke_records::protocol::{
    Artifact, ArtifactBody, ArtifactDecodeFailureRecord, ArtifactPayloadKind, ArtifactWriteRecord,
    INTERVENTION_APPLY, INTERVENTION_ISSUE_DETECTION, INTERVENTION_SYNTHESIS,
    TOOL_CALL_INTENT_SEGMENTATION, TOOL_CALL_REVIEW, TOOL_CALL_SEGMENT_REVIEW, decode_artifact_str,
};
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

#[derive(Debug, Clone)]
pub struct DecodedProtocolArtifactFile {
    pub path: PathBuf,
    pub artifact: Artifact,
}

#[derive(Debug, Clone)]
#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
pub enum ProtocolArtifactLoadResult {
    Loaded(DecodedProtocolArtifactFile),
    Unloaded(ProtocolArtifactLoadFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
pub enum ProtocolArtifactLoadFailure {
    Decode(ArtifactDecodeFailureRecord),
    Identity(ProtocolArtifactIdentityMismatch),
}

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
impl ProtocolArtifactLoadResult {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Loaded(entry) => Some(entry.path.as_path()),
            Self::Unloaded(failure) => failure.path(),
        }
    }
}

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
impl ProtocolArtifactLoadFailure {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Decode(failure) => failure.path.as_deref(),
            Self::Identity(mismatch) => Some(mismatch.path.as_path()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolArtifactIdentityMismatch {
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
    let stored = ArtifactWriteRecord {
        schema_version: PROTOCOL_ARTIFACT_SCHEMA_VERSION,
        procedure_name,
        subject_id: &resolved_identity.subject_id,
        run_id: &resolved_identity.run_id,
        created_at_ms,
        model_id,
        provider_slug,
        input,
        output,
        artifact,
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

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
pub fn list_protocol_artifact_load_results(
    record_path: &Path,
) -> Result<Vec<ProtocolArtifactLoadResult>, PrepareError> {
    let resolved_identity = resolve_protocol_run_identity(record_path)?;
    list_protocol_artifact_load_results_for_identity(&resolved_identity)
}

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
fn list_protocol_artifact_load_results_for_identity(
    resolved_identity: &ResolvedProtocolRunIdentity,
) -> Result<Vec<ProtocolArtifactLoadResult>, PrepareError> {
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
        entries.push(load_protocol_artifact_result(&path, resolved_identity));
    }

    entries.sort_by(|left, right| right.path().cmp(&left.path()));
    Ok(entries)
}

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
fn load_protocol_artifact_result(
    path: &Path,
    resolved_identity: &ResolvedProtocolRunIdentity,
) -> ProtocolArtifactLoadResult {
    match load_protocol_artifact_tolerant(path, resolved_identity) {
        Ok(entry) => ProtocolArtifactLoadResult::Loaded(entry),
        Err(failure) => ProtocolArtifactLoadResult::Unloaded(failure),
    }
}

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
fn load_protocol_artifact_tolerant(
    path: &Path,
    resolved_identity: &ResolvedProtocolRunIdentity,
) -> Result<DecodedProtocolArtifactFile, ProtocolArtifactLoadFailure> {
    let text = fs::read_to_string(path)
        .map_err(|source| ProtocolArtifactLoadFailure::Decode(read_decode_failure(path, source)))?;
    let decoded = decode_artifact_str(&text, Some(path.to_path_buf()))
        .map_err(ProtocolArtifactLoadFailure::Decode)?;
    let entry = DecodedProtocolArtifactFile {
        path: path.to_path_buf(),
        artifact: decoded,
    };
    validate_decoded_protocol_artifact_identity(&entry, resolved_identity)
        .map_err(ProtocolArtifactLoadFailure::Identity)?;
    Ok(entry)
}

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
fn read_decode_failure(path: &Path, source: std::io::Error) -> ArtifactDecodeFailureRecord {
    ArtifactDecodeFailureRecord {
        path: Some(path.to_path_buf()),
        coordinate: None,
        expected_payload_kind: None,
        error: format!("failed to read protocol artifact: {source}"),
    }
}

#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
#[allow(
    dead_code,
    reason = "task-stack:typed-persistence-spine consumed by the next aggregate slice"
)]
fn payload_kind_for_procedure(procedure_name: &str) -> Option<ArtifactPayloadKind> {
    match procedure_name {
        TOOL_CALL_INTENT_SEGMENTATION => Some(ArtifactPayloadKind::ToolCallIntentSegmentation),
        TOOL_CALL_REVIEW => Some(ArtifactPayloadKind::ToolCallReview),
        TOOL_CALL_SEGMENT_REVIEW => Some(ArtifactPayloadKind::ToolCallSegmentReview),
        INTERVENTION_ISSUE_DETECTION => Some(ArtifactPayloadKind::InterventionIssueDetection),
        INTERVENTION_SYNTHESIS => Some(ArtifactPayloadKind::InterventionSynthesis),
        INTERVENTION_APPLY => Some(ArtifactPayloadKind::InterventionApply),
        _ => None,
    }
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

fn validate_decoded_protocol_artifact_identity(
    entry: &DecodedProtocolArtifactFile,
    resolved_identity: &ResolvedProtocolRunIdentity,
) -> Result<(), ProtocolArtifactIdentityMismatch> {
    validate_decoded_protocol_artifact_field(
        entry,
        "stored.run_id",
        &entry.artifact.run_id,
        &resolved_identity.run_id,
    )?;
    validate_decoded_protocol_artifact_field(
        entry,
        "stored.subject_id",
        &entry.artifact.subject_id,
        &resolved_identity.subject_id,
    )?;

    if let Some(output_subject_id) = decoded_protocol_output_subject_id(entry) {
        validate_decoded_protocol_artifact_field(
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

fn validate_decoded_protocol_artifact_field(
    entry: &DecodedProtocolArtifactFile,
    field: &'static str,
    actual: &str,
    expected: &str,
) -> Result<(), ProtocolArtifactIdentityMismatch> {
    if actual == expected {
        return Ok(());
    }

    Err(ProtocolArtifactIdentityMismatch {
        path: entry.path.clone(),
        procedure_name: entry.artifact.procedure_name.clone(),
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

fn decoded_protocol_output_subject_id(entry: &DecodedProtocolArtifactFile) -> Option<&str> {
    match &entry.artifact.body {
        ArtifactBody::ToolCallIntentSegmentation(payload) => {
            Some(payload.output.sequence.subject_id.as_str())
        }
        ArtifactBody::ToolCallReview(payload) => Some(payload.output.packet.subject_id.as_str()),
        ArtifactBody::ToolCallSegmentReview(payload) => {
            Some(payload.output.packet.subject_id.as_str())
        }
        ArtifactBody::InterventionIssueDetection(_)
        | ArtifactBody::InterventionSynthesis(_)
        | ArtifactBody::InterventionApply(_) => None,
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

    fn test_identity(
        tmp: &tempfile::TempDir,
        run_id: &str,
        subject_id: &str,
    ) -> ResolvedProtocolRunIdentity {
        let run_dir = tmp.path().join(run_id);
        fs::create_dir_all(protocol_artifacts_dir_for_run(&run_dir)).expect("artifact dir");
        ResolvedProtocolRunIdentity {
            record_path: run_dir.join("record.json.zst"),
            run_dir,
            run_id: run_id.to_string(),
            subject_id: subject_id.to_string(),
        }
    }

    fn write_artifact_file(identity: &ResolvedProtocolRunIdentity, name: &str, body: Value) {
        let path = protocol_artifacts_dir_for_run(&identity.run_dir).join(name);
        fs::write(
            &path,
            serde_json::to_string_pretty(&body).expect("serialize artifact"),
        )
        .expect("write artifact");
    }

    fn write_artifact_text(identity: &ResolvedProtocolRunIdentity, name: &str, body: &str) {
        let path = protocol_artifacts_dir_for_run(&identity.run_dir).join(name);
        fs::write(path, body).expect("write artifact text");
    }

    fn sequence_json(subject_id: &str) -> Value {
        serde_json::json!({
            "subject_id": subject_id,
            "total_turns": 0,
            "total_calls_in_run": 0,
            "turns": [],
            "calls": []
        })
    }

    fn signals_json() -> Value {
        serde_json::json!({
            "total_turns": 0,
            "total_calls": 0,
            "search_calls": 0,
            "read_calls": 0,
            "browse_calls": 0,
            "edit_calls": 0,
            "execute_calls": 0,
            "failed_calls": 0,
            "repeated_search_runs": 0,
            "directory_pivots": 0
        })
    }

    fn review_context_json(subject_id: &str) -> Value {
        serde_json::json!({
            "sequence": sequence_json(subject_id),
            "signals": signals_json()
        })
    }

    fn segmentation_output_json(subject_id: &str) -> Value {
        serde_json::json!({
            "sequence": sequence_json(subject_id),
            "signals": signals_json(),
            "segments": [],
            "coverage": {
                "total_calls": 0,
                "labeled_segments": 0,
                "ambiguous_segments": 0,
                "labeled_calls": 0,
                "ambiguous_calls": 0,
                "uncovered_calls": 0
            },
            "overall_rationale": "empty test sequence"
        })
    }

    fn judgment_json() -> Value {
        serde_json::json!({
            "segments": [],
            "overall_rationale": "empty test sequence"
        })
    }

    fn mechanized_step_json(input: Value, output: Value) -> Value {
        serde_json::json!({
            "step_id": "step-mechanized",
            "step_name": "mechanized",
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

    fn llm_step_json(input: Value, output: Value) -> Value {
        serde_json::json!({
            "step_id": "step-llm",
            "step_name": "llm",
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

    fn valid_segmentation_json(
        run_id: &str,
        subject_id: &str,
        created_at_ms: u64,
        stored_subject_id: &str,
    ) -> Value {
        let sequence = sequence_json(subject_id);
        let context = review_context_json(subject_id);
        let judgment = judgment_json();
        let output = segmentation_output_json(subject_id);
        serde_json::json!({
            "schema_version": PROTOCOL_ARTIFACT_SCHEMA_VERSION,
            "procedure_name": TOOL_CALL_INTENT_SEGMENTATION,
            "subject_id": stored_subject_id,
            "run_id": run_id,
            "created_at_ms": created_at_ms,
            "model_id": "model",
            "provider_slug": "provider",
            "input": sequence,
            "output": output,
            "artifact": {
                "procedure_name": TOOL_CALL_INTENT_SEGMENTATION,
                "artifact": {
                    "first": mechanized_step_json(sequence_json(subject_id), context.clone()),
                    "second": {
                        "branches": {
                            "input": {
                                "state": context.clone(),
                                "disposition": "record_and_forward"
                            },
                            "left_branch": "mechanized",
                            "right_branch": "llm",
                            "left": mechanized_step_json(context.clone(), context.clone()),
                            "right": llm_step_json(context.clone(), judgment.clone())
                        },
                        "merge": mechanized_step_json(
                            serde_json::json!({
                                "source": context,
                                "left": review_context_json(subject_id),
                                "right": judgment
                            }),
                            segmentation_output_json(subject_id)
                        )
                    }
                }
            }
        })
    }

    fn malformed_known_payload_json(run_id: &str, subject_id: &str) -> Value {
        serde_json::json!({
            "schema_version": PROTOCOL_ARTIFACT_SCHEMA_VERSION,
            "procedure_name": TOOL_CALL_REVIEW,
            "subject_id": subject_id,
            "run_id": run_id,
            "created_at_ms": 42,
            "model_id": "model",
            "provider_slug": "provider",
            "input": {},
            "output": {},
            "artifact": {}
        })
    }

    fn future_payload_json(run_id: &str, subject_id: &str) -> Value {
        serde_json::json!({
            "schema_version": PROTOCOL_ARTIFACT_SCHEMA_VERSION,
            "procedure_name": "future_protocol_procedure",
            "subject_id": subject_id,
            "run_id": run_id,
            "created_at_ms": 43,
            "model_id": "future-model",
            "provider_slug": "future-provider",
            "input": {},
            "output": {},
            "artifact": {}
        })
    }

    fn load_rows(identity: &ResolvedProtocolRunIdentity) -> Vec<ProtocolArtifactLoadResult> {
        list_protocol_artifact_load_results_for_identity(identity).expect("list artifacts")
    }

    fn loaded_rows(rows: &[ProtocolArtifactLoadResult]) -> Vec<&DecodedProtocolArtifactFile> {
        rows.iter()
            .filter_map(|row| match row {
                ProtocolArtifactLoadResult::Loaded(entry) => Some(entry),
                ProtocolArtifactLoadResult::Unloaded(_) => None,
            })
            .collect()
    }

    fn decode_failures(rows: &[ProtocolArtifactLoadResult]) -> Vec<&ArtifactDecodeFailureRecord> {
        rows.iter()
            .filter_map(|row| match row {
                ProtocolArtifactLoadResult::Unloaded(ProtocolArtifactLoadFailure::Decode(
                    failure,
                )) => Some(failure),
                _ => None,
            })
            .collect()
    }

    fn identity_failures(
        rows: &[ProtocolArtifactLoadResult],
    ) -> Vec<&ProtocolArtifactIdentityMismatch> {
        rows.iter()
            .filter_map(|row| match row {
                ProtocolArtifactLoadResult::Unloaded(ProtocolArtifactLoadFailure::Identity(
                    failure,
                )) => Some(failure),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn tolerant_listing_returns_loaded_and_malformed_rows() {
        let tmp = tempfile::tempdir().expect("tmp");
        let identity = test_identity(&tmp, "run-1", "subject-1");
        write_artifact_file(
            &identity,
            "1000_valid.json",
            valid_segmentation_json(
                &identity.run_id,
                &identity.subject_id,
                1000,
                &identity.subject_id,
            ),
        );
        write_artifact_text(&identity, "1001_malformed.json", "{");

        let rows = load_rows(&identity);

        assert_eq!(rows.len(), 2);
        assert_eq!(loaded_rows(&rows).len(), 1);
        assert_eq!(decode_failures(&rows).len(), 1);
    }

    #[test]
    fn tolerant_listing_reports_future_procedure_decode_failure_with_coordinate() {
        let tmp = tempfile::tempdir().expect("tmp");
        let identity = test_identity(&tmp, "run-1", "subject-1");
        write_artifact_file(
            &identity,
            "1000_future.json",
            future_payload_json(&identity.run_id, &identity.subject_id),
        );

        let rows = load_rows(&identity);
        let failures = decode_failures(&rows);

        assert_eq!(failures.len(), 1);
        let coordinate = failures[0].coordinate.as_ref().expect("coordinate");
        assert_eq!(coordinate.procedure_name, "future_protocol_procedure");
        assert_eq!(coordinate.subject_id, identity.subject_id);
        assert_eq!(coordinate.run_id, identity.run_id);
        assert_eq!(failures[0].expected_payload_kind, None);
    }

    #[test]
    fn tolerant_listing_reports_known_nested_payload_failure_kind() {
        let tmp = tempfile::tempdir().expect("tmp");
        let identity = test_identity(&tmp, "run-1", "subject-1");
        write_artifact_file(
            &identity,
            "1000_bad_review.json",
            malformed_known_payload_json(&identity.run_id, &identity.subject_id),
        );

        let rows = load_rows(&identity);
        let failures = decode_failures(&rows);

        assert_eq!(failures.len(), 1);
        assert_eq!(
            failures[0].expected_payload_kind,
            Some(ArtifactPayloadKind::ToolCallReview)
        );
        let coordinate = failures[0].coordinate.as_ref().expect("coordinate");
        assert_eq!(coordinate.procedure_name, TOOL_CALL_REVIEW);
    }

    #[test]
    fn tolerant_listing_reports_identity_mismatch_row() {
        let tmp = tempfile::tempdir().expect("tmp");
        let identity = test_identity(&tmp, "run-1", "subject-1");
        write_artifact_file(
            &identity,
            "1000_identity_mismatch.json",
            valid_segmentation_json(
                &identity.run_id,
                &identity.subject_id,
                1000,
                "other-subject",
            ),
        );

        let rows = load_rows(&identity);
        let failures = identity_failures(&rows);

        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].field, "stored.subject_id");
        assert_eq!(failures[0].expected, identity.subject_id);
        assert_eq!(failures[0].actual, "other-subject");
    }

    #[test]
    fn tolerant_listing_mixes_all_json_rows_and_ignores_non_json() {
        let tmp = tempfile::tempdir().expect("tmp");
        let identity = test_identity(&tmp, "run-1", "subject-1");
        write_artifact_file(
            &identity,
            "1000_valid.json",
            valid_segmentation_json(
                &identity.run_id,
                &identity.subject_id,
                1000,
                &identity.subject_id,
            ),
        );
        write_artifact_text(&identity, "1001_malformed.json", "{");
        write_artifact_file(
            &identity,
            "1002_future.json",
            future_payload_json(&identity.run_id, &identity.subject_id),
        );
        write_artifact_file(
            &identity,
            "1003_identity_mismatch.json",
            valid_segmentation_json(
                &identity.run_id,
                &identity.subject_id,
                1003,
                "other-subject",
            ),
        );
        fs::write(
            protocol_artifacts_dir_for_run(&identity.run_dir).join("ignored.txt"),
            "not json",
        )
        .expect("write ignored");

        let rows = load_rows(&identity);

        assert_eq!(rows.len(), 4);
        assert_eq!(loaded_rows(&rows).len(), 1);
        assert_eq!(decode_failures(&rows).len(), 2);
        assert_eq!(identity_failures(&rows).len(), 1);
    }

    #[test]
    fn tolerant_listing_uses_json_body_not_filename_for_identity() {
        let tmp = tempfile::tempdir().expect("tmp");
        let identity = test_identity(&tmp, "run-1", "subject-1");
        write_artifact_file(
            &identity,
            "1000_tool_call_review_wrong-subject.json",
            valid_segmentation_json(
                &identity.run_id,
                &identity.subject_id,
                1000,
                &identity.subject_id,
            ),
        );

        let rows = load_rows(&identity);
        let loaded = loaded_rows(&rows);

        assert_eq!(rows.len(), 1);
        assert_eq!(loaded.len(), 1);
        assert_eq!(
            loaded[0].artifact.procedure_name,
            TOOL_CALL_INTENT_SEGMENTATION
        );
        assert_eq!(loaded[0].artifact.subject_id, identity.subject_id);
    }

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
