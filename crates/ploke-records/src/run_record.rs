//! Passive `record.json.gz` schema shared by eval, tree, and UI readers.
//!
//! These records mirror the compressed Prototype 1 run record emitted by
//! `ploke-eval`. Deserializing this module's types does not validate a run,
//! replay state, execute tools, or grant runtime authority.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

use crate::agent_turn::{
    AgentTurnArtifactRecord, LlmMetadataRecord, LlmResponseRecord, RequestMessageRecord,
    ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord,
};
use crate::evaluation::PatchProjectionCheckState;
use crate::record::{Record, RecordFamily, RecordFormat};

pub const RUN_RECORD_SCHEMA_VERSION: &str = "run-record.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRecord {
    pub schema_version: String,
    pub manifest_id: String,
    pub metadata: RunMetadata,
    pub phases: RunPhases,
    pub db_time_travel_index: Vec<TimeTravelMarker>,
    #[serde(default)]
    pub conversation: Vec<ConversationMessageRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<RunTimingSummary>,
}

impl Record for RunRecord {
    const FAMILY: RecordFamily = RecordFamily::RunRecord;
    const SCHEMA: &'static str = RUN_RECORD_SCHEMA_VERSION;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

impl RunRecord {
    pub fn turn_count(&self) -> usize {
        self.phases.agent_turns.len()
    }

    pub fn tool_call_count(&self) -> usize {
        self.phases
            .agent_turns
            .iter()
            .map(|turn| turn.tool_calls.len())
            .sum()
    }

    pub fn failed_tool_call_count(&self) -> usize {
        self.phases
            .agent_turns
            .iter()
            .flat_map(|turn| &turn.tool_calls)
            .filter(|call| matches!(call.result, ToolResult::Failed(_)))
            .count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompressedRunRecordProfile {
    pub path: PathBuf,
    pub compressed_bytes: u64,
    pub decompressed_bytes: u64,
    pub open_read_ns: u64,
    pub decompress_ns: u64,
    pub deserialize_ns: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProfiledRunRecord {
    pub record: RunRecord,
    pub profile: CompressedRunRecordProfile,
}

pub fn read_compressed_record(path: &Path) -> Result<RunRecord, io::Error> {
    let file = File::open(path)?;
    let decoder = GzDecoder::new(file);
    let record = serde_json::from_reader(decoder)?;
    Ok(record)
}

pub fn read_compressed_record_profiled(path: &Path) -> Result<ProfiledRunRecord, io::Error> {
    let start = Instant::now();
    let mut file = File::open(path)?;
    let mut compressed = Vec::new();
    file.read_to_end(&mut compressed)?;
    let open_read_ns = elapsed_ns(start);

    let start = Instant::now();
    let mut decoder = GzDecoder::new(compressed.as_slice());
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    let decompress_ns = elapsed_ns(start);

    let start = Instant::now();
    let record = serde_json::from_slice::<RunRecord>(&decompressed).map_err(io::Error::other)?;
    let deserialize_ns = elapsed_ns(start);

    Ok(ProfiledRunRecord {
        record,
        profile: CompressedRunRecordProfile {
            path: path.to_path_buf(),
            compressed_bytes: compressed.len().try_into().unwrap_or(u64::MAX),
            decompressed_bytes: decompressed.len().try_into().unwrap_or(u64::MAX),
            open_read_ns,
            decompress_ns,
            deserialize_ns,
        },
    })
}

fn elapsed_ns(start: Instant) -> u64 {
    start.elapsed().as_nanos().try_into().unwrap_or(u64::MAX)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunMetadata {
    #[serde(default = "default_run_arm")]
    pub run_arm: RunArm,
    pub benchmark: BenchmarkMetadata,
    pub agent: AgentMetadata,
    pub runtime: RuntimeMetadata,
    pub budget: EvalBudget,
}

fn default_run_arm() -> RunArm {
    RunArm {
        id: "shell-only".to_owned(),
        role: RunArmRole::Control,
        command: "run single setup".to_owned(),
        execution: "setup-only".to_owned(),
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunArmRole {
    Control,
    Treatment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunArm {
    pub id: String,
    pub role: RunArmRole,
    pub command: String,
    pub execution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchmarkMetadata {
    pub instance_id: String,
    pub repo_root: PathBuf,
    pub base_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentMetadata {
    #[serde(
        rename = "selected_model",
        alias = "model_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub model_id: Option<String>,
    #[serde(
        rename = "selected_provider",
        alias = "provider",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_endpoint: Option<SelectedEndpointProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_schema_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SelectedEndpointProvenance {
    pub provider_name: String,
    pub provider_slug: String,
    pub endpoint_name: String,
    pub endpoint_model_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RuntimeMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_clock_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalBudget {
    pub max_turns: u32,
    pub max_tool_calls: u32,
    pub wall_clock_secs: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RunPhases {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupPhase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub agent_turns: Vec<TurnRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<PatchPhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationPhase>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packaging: Option<PackagingPhase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SetupPhase {
    pub started_at: String,
    pub ended_at: String,
    pub repo_state: RepoStateRecord,
    pub indexing_status: IndexingStatusRecord,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub indexed_crates: Vec<IndexedCrateSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parse_failures: Vec<ParseFailureRecord>,
    pub db_timestamp_micros: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_schema_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoStateRecord {
    pub repo_root: PathBuf,
    pub requested_base_sha: Option<String>,
    pub checked_out_head_sha: Option<String>,
    pub git_status_porcelain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexingStatusRecord {
    pub status: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_progress: Option<IndexingProgressRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexingProgressRecord {
    pub raw_status: String,
    pub recent_processed: usize,
    pub num_not_proc: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_file: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
    pub observed_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexedCrateSummary {
    pub name: String,
    pub version: String,
    pub namespace: String,
    pub root_path: PathBuf,
    pub file_count: usize,
    pub node_count: usize,
    pub embedded_count: usize,
    pub status: CrateIndexStatus,
    pub parse_error: Option<ParseErrorSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CrateIndexStatus {
    Success,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParseErrorSummary {
    pub message: String,
    pub target_dir: PathBuf,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParseFailureRecord {
    pub target_dir: PathBuf,
    pub message: String,
    pub occurred_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnRecord {
    pub turn_number: u32,
    pub started_at: String,
    pub ended_at: String,
    pub db_timestamp_micros: i64,
    pub issue_prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_request: Option<ChatRequestRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_response: Option<LlmResponseRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolExecutionRecord>,
    pub outcome: TurnOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_turn_artifact: Option<AgentTurnArtifactRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatRequestRecord {
    pub model: String,
    pub messages: Vec<RequestMessageRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionRecord {
    pub request: ToolRequestRecord,
    pub result: ToolResult,
    #[serde(default)]
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum ToolResult {
    Completed(ToolCompletedRecord),
    Failed(ToolFailedRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum TurnOutcome {
    ToolCalls { count: usize },
    Content,
    Error { message: String },
    Timeout { elapsed_secs: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchPhase {
    pub started_at: String,
    pub ended_at: String,
    pub patch_artifact: crate::agent_turn::PatchArtifactRecord,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationPhase {
    pub started_at: String,
    pub ended_at: String,
    pub build_result: BuildResult,
    pub test_result: TestResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub benchmark_verdict: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionArtifactState {
    NotRecorded,
    NotApplicable,
    Missing,
    Empty,
    Nonempty,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackagingPhase {
    pub started_at: String,
    pub ended_at: String,
    pub submission_artifact_state: SubmissionArtifactState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msb_submission_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch_projection_path: Option<PathBuf>,
    #[serde(default)]
    pub patch_projection_check_state: PatchProjectionCheckState,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum BuildResult {
    Success,
    Failed { exit_code: i32, stderr: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status")]
pub enum TestResult {
    Passed,
    Failed {
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
    Skipped {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeTravelMarker {
    pub turn: u32,
    pub timestamp_micros: i64,
    pub event: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationMessageRecord {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<LlmMetadataRecord>,
    pub content: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunTimingSummary {
    pub started_at: String,
    pub ended_at: String,
    pub total_wall_clock_secs: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_wall_clock_secs: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_wall_clock_secs: Option<f64>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::*;

    #[test]
    fn read_compressed_record_profiled_splits_reader_phases() {
        let root = unique_temp_dir("ploke-records-profiled-run-record");
        fs::create_dir_all(&root).expect("temp dir");
        let path = root.join("record.json.gz");
        let json = r#"{
  "schema_version": "run-record.v1",
  "manifest_id": "manifest-fixture",
  "metadata": {
    "benchmark": {
      "instance_id": "instance-fixture",
      "repo_root": "/tmp/repo",
      "base_sha": null
    },
    "agent": {},
    "runtime": {},
    "budget": {
      "max_turns": 1,
      "max_tool_calls": 1,
      "wall_clock_secs": 1
    }
  },
  "phases": {},
  "db_time_travel_index": []
}"#;
        let file = fs::File::create(&path).expect("create fixture");
        let mut encoder = GzEncoder::new(file, Compression::default());
        encoder.write_all(json.as_bytes()).expect("write gzip");
        encoder.finish().expect("finish gzip");

        let profiled = read_compressed_record_profiled(&path).expect("read profiled record");
        assert_eq!(profiled.record.schema_version, RUN_RECORD_SCHEMA_VERSION);
        assert_eq!(profiled.record.manifest_id, "manifest-fixture");
        assert!(profiled.profile.compressed_bytes > 0);
        assert!(profiled.profile.decompressed_bytes > profiled.profile.compressed_bytes);
        let _ = fs::remove_dir_all(root);
    }

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("{label}-{nanos}"))
    }
}
