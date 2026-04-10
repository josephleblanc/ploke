//! RunRecord types for comprehensive run persistence and replay.
//!
//! This module provides the unified `RunRecord` structure that aggregates all data
//! from an eval run into a single queryable artifact. It enables:
//! - A4 (Layer 1): Comprehensive result schema
//! - A5 (measurement): Replay and introspection without re-running
//!
//! # Record Types Overview
//!
//! The RunRecord is a hierarchical structure that captures the complete state of an
//! eval run. It is emitted as `record.json.gz` alongside existing artifacts.
//!
//! ## Top-Level Structure
//!
//! ```text
//! RunRecord (record.json.gz)
//! ├── schema_version          # For future migrations
//! ├── manifest_id             # Links to run.json
//! ├── metadata                # Benchmark, agent, runtime config
//! │   ├── benchmark           # Instance ID, repo, base SHA, issue
//! │   ├── agent               # Model ID, provider, prompt version
//! │   └── runtime             # Temperature, max_turns, etc.
//! ├── phases                  # Phase-by-phase execution data
//! │   ├── setup               # Checkout, indexing, repo state
//! │   ├── agent_turns         # Vec<TurnRecord> - one per LLM interaction
//! │   ├── patch               # Edit proposals, applied status
//! │   └── validation          # Build/test results
//! ├── db_time_travel_index    # Cozo @ timestamps for historical queries
//! └── conversation            # Complete message history
//! ```
//!
//! ## Key Record Types
//!
//! | Type | Purpose | Contains |
//! |------|---------|----------|
//! | [`RunRecord`] | Top-level container | All run data, links to manifest |
//! | [`TurnRecord`] | Single agent turn | LLM request/response, tool calls, timestamp |
//! | [`TimeTravelMarker`] | Cozo DB snapshot point | Turn number + validity timestamp |
//! | [`LlmResponseRecord`] | Structured LLM output | Content, model, token usage, finish reason |
//! | [`ToolExecutionRecord`] | Tool call + result | Request, result (completed/failed), latency |
//! | [`RunOutcomeSummary`] | Quick summary | Status, turn count, total tokens, wall time |
//!
//! ## Relationship to Other Artifacts
//!
//! The RunRecord consolidates data from multiple existing artifacts:
//! - `run.json` → [`RunMetadata`] (manifest reference)
//! - `agent-turn-trace.json` → [`TurnRecord`] + [`TurnOutcome`]
//! - `repo-state.json` → [`SetupPhase::repo_state`]
//! - `indexing-status.json` → [`SetupPhase::indexing_status`]
//! - `final-snapshot.db` → queried via [`TimeTravelMarker`] timestamps
//!
//! # Output Location
//!
//! The RunRecord is written to:
//! ```text
//! runs/{date}/{task_id}/record.json.gz
//! ```
//!
//! This compressed JSON file replaces the fragmented artifact approach with a
//! single comprehensive record while maintaining backward compatibility with
//! existing artifacts during the transition period.
//!
//! # TODO: Access Examples
//!
//! Once the introspection API is implemented (Phase 1F), add examples here showing:
//! - Loading a RunRecord from disk
//! - Querying DB state at a specific turn using `db_time_travel_index`
//! - Reconstructing conversation up to a turn
//! - Extracting tool calls and their results
//! - Replaying a specific turn's execution
//!
//! See [`RunRecord`] methods for the API surface.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Re-export types from ploke-llm that we need for structured capture
pub use ploke_llm::request::ChatCompReqCore;
pub use ploke_llm::response::{FinishReason, OpenAiResponse, TokenUsage};
pub use ploke_llm::types::meta::{LLMMetadata, PerformanceMetrics};
pub use ploke_llm::types::model_types::ModelId;

use crate::runner::{
    AgentTurnArtifact, IndexingStatusArtifact, PatchArtifact, RepoStateArtifact,
    ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord,
};
use crate::spec::{EvalBudget, IssueInput, PreparedSingleRun};

/// Schema version for RunRecord migrations.
pub const RUN_RECORD_SCHEMA_VERSION: &str = "run-record.v1";

/// Top-level container for all run data — the singular `record.json.gz`.
///
/// This struct aggregates every phase of an eval run into a single serializable
/// record that enables full replay and introspection. It is the primary output
/// of the RunRecord system, written as a compressed JSON file alongside the
/// existing fragmented artifacts.
///
/// # Structure
///
/// The RunRecord is organized into:
/// - **Metadata**: What was being tested (benchmark, agent config, runtime params)
/// - **Phases**: What happened during the run (setup → turns → patch → validation)
/// - **Time-travel index**: When each phase occurred (for Cozo `@` queries)
/// - **Conversation**: Complete message history across all turns
///
/// # Usage
///
/// ```rust,ignore
/// // Create from a prepared run manifest
/// let mut record = RunRecord::new(&prepared_run);
///
/// // Capture timestamps at turn boundaries
/// record.mark_time_travel(1, db.now_micros(), "turn_start");
///
/// // Add turn data from agent execution
/// record.phases.agent_turns.push(turn_record);
///
/// // Finalize and write to disk
/// record.finalize(validation_phase, outcome_summary);
/// write_compressed_record(&paths.record_path.unwrap(), &record)?;
/// ```
///
/// See the module-level documentation for the full type hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    /// Schema version for future migrations.
    pub schema_version: String,

    /// Unique identifier matching the run manifest.
    pub manifest_id: String,

    /// High-level metadata about the run.
    pub metadata: RunMetadata,

    /// Phase-by-phase execution data.
    pub phases: RunPhases,

    /// Time-travel index for Cozo DB queries at historical states.
    pub db_time_travel_index: Vec<TimeTravelMarker>,

    /// Complete conversation history across all turns.
    pub conversation: Vec<ConversationMessage>,
}

impl RunRecord {
    /// Create a new RunRecord with the given manifest.
    pub fn new(manifest: &PreparedSingleRun) -> Self {
        Self {
            schema_version: RUN_RECORD_SCHEMA_VERSION.to_string(),
            manifest_id: manifest.task_id.clone(),
            metadata: RunMetadata::from_manifest(manifest),
            phases: RunPhases::default(),
            db_time_travel_index: Vec::new(),
            conversation: Vec::new(),
        }
    }

    /// Add a time-travel marker at the current turn.
    pub fn mark_time_travel(&mut self, turn: u32, timestamp_micros: i64, event: impl Into<String>) {
        self.db_time_travel_index.push(TimeTravelMarker {
            turn,
            timestamp_micros,
            event: event.into(),
        });
    }

    /// Get the timestamp for a specific turn's completion.
    pub fn turn_timestamp(&self, turn: u32) -> Option<i64> {
        self.db_time_travel_index
            .iter()
            .find(|m| m.turn == turn && m.event == "turn_complete")
            .map(|m| m.timestamp_micros)
    }
}

/// Metadata extracted from the run manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    /// The benchmark instance being evaluated.
    pub benchmark: BenchmarkMetadata,

    /// Agent configuration for this run.
    pub agent: AgentMetadata,

    /// Runtime parameters.
    pub runtime: RuntimeMetadata,

    /// Budget constraints applied.
    pub budget: EvalBudget,
}

impl RunMetadata {
    /// Extract metadata from a PreparedSingleRun.
    pub fn from_manifest(manifest: &PreparedSingleRun) -> Self {
        Self {
            benchmark: BenchmarkMetadata {
                instance_id: manifest.task_id.clone(),
                repo_root: manifest.repo_root.clone(),
                base_sha: manifest.base_sha.clone(),
                issue: Some(manifest.issue.clone()),
            },
            agent: AgentMetadata::default(), // Populated during run
            runtime: RuntimeMetadata::default(), // Populated during run
            budget: manifest.budget.clone(),
        }
    }
}

/// Benchmark identification metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkMetadata {
    /// Unique task/instance identifier.
    pub instance_id: String,

    /// Path to the repository root.
    pub repo_root: PathBuf,

    /// Git SHA checked out for this run.
    pub base_sha: Option<String>,

    /// Issue description/input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<IssueInput>,
}

/// Agent configuration metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentMetadata {
    /// Selected model ID (e.g., "anthropic/claude-sonnet-4").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<ModelId>,

    /// Provider used (e.g., "openrouter", "anthropic").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// System prompt identifier (hash or version).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt_version: Option<String>,

    /// Tool schema version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_schema_version: Option<String>,
}

/// Runtime parameter metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeMetadata {
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Maximum turns allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,

    /// Maximum tool calls allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_calls: Option<u32>,

    /// Wall clock timeout in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wall_clock_timeout_secs: Option<u64>,
}

/// Phase-by-phase execution data.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunPhases {
    /// Setup phase: checkout, indexing, initial state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupPhase>,

    /// Agent turn phase: all LLM interactions and tool calls.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub agent_turns: Vec<TurnRecord>,

    /// Patch phase: edit proposals and application.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<PatchPhase>,

    /// Validation phase: build and test results.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<ValidationPhase>,
}

/// Setup phase data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPhase {
    /// When setup started.
    pub started_at: String, // ISO 8601

    /// When setup completed.
    pub ended_at: String, // ISO 8601

    /// Repository state after checkout.
    pub repo_state: RepoStateArtifact,

    /// Indexing status result.
    pub indexing_status: IndexingStatusArtifact,

    /// Any parse failures during indexing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_failure: Option<ParseFailureRecord>,

    /// Cozo timestamp at setup completion.
    pub db_timestamp_micros: i64,
}

/// Parse failure record (simplified from FlattenedParserDiagnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseFailureRecord {
    pub target_dir: PathBuf,
    pub message: String,
    pub occurred_at_ms: i64,
}

/// Single agent turn record — one entry per LLM interaction.
///
/// A turn represents one complete cycle of: send prompt → receive response →
/// execute tool calls (if any). The `TurnRecord` captures everything needed to
/// replay or introspect that specific turn, including the Cozo DB timestamp
/// for querying the code graph state as it existed at that moment.
///
/// # Relationship to RunRecord
///
/// TurnRecords are stored in [`RunRecord::phases::agent_turns`], forming a
/// chronological sequence of the entire agent session. Each turn references
/// the same `db_timestamp_micros` that appears in the
/// [`db_time_travel_index`](RunRecord::db_time_travel_index).
///
/// # Example Structure
///
/// ```text
/// TurnRecord (turn 3 of 7)
/// ├── turn_number: 3
/// ├── started_at: "2026-04-09T18:30:15Z"
/// ├── ended_at: "2026-04-09T18:30:45Z"
/// ├── db_timestamp_micros: 1744223415500000  # For Cozo @ query
/// ├── issue_prompt: "Fix the bug in src/lib.rs..."
/// ├── llm_request: ChatCompReqCore { ... }    # What we sent
/// ├── llm_response: LlmResponseRecord { ... } # What we received
/// ├── tool_calls: [ToolExecutionRecord, ...]  # Tools executed
/// └── outcome: TurnOutcome::ToolCalls { count: 2 }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    /// Turn number (1-indexed).
    pub turn_number: u32,

    /// When this turn started.
    pub started_at: String, // ISO 8601

    /// When this turn completed.
    pub ended_at: String, // ISO 8601

    /// Cozo DB timestamp at turn start (for @ queries).
    pub db_timestamp_micros: i64,

    /// The issue prompt for this turn.
    pub issue_prompt: String,

    /// LLM request sent (if captured).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_request: Option<ChatCompReqCore>,

    /// LLM response received (if captured).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_response: Option<LlmResponseRecord>,

    /// Tool calls executed during this turn.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<ToolExecutionRecord>,

    /// Outcome of this turn.
    pub outcome: TurnOutcome,

    /// Events observed during this turn (from AgentTurnArtifact).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_turn_artifact: Option<AgentTurnArtifact>,
}

/// Structured LLM response capture.
///
/// Captures the essential data from an LLM response in a structured format
/// suitable for persistence and replay. This type is populated from
/// `ChatEvt::Response` events in the runner, avoiding the need to modify
/// production code in ploke-tui or ploke-llm.
///
/// # Relationship to Raw Response
///
/// While the raw `OpenAiResponse` contains full API details, `LlmResponseRecord`
/// extracts the fields most useful for analysis: content, model identification,
/// token usage, and finish reason. This provides a balance between completeness
/// and serialization efficiency.
///
/// # Usage
///
/// Stored in [`TurnRecord::llm_response`] for each turn, enabling:
/// - Cost analysis via [`TokenUsage`]
/// - Model performance comparison
/// - Content replay for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponseRecord {
    /// Response content (assistant's message).
    pub content: String,

    /// Model that generated the response.
    pub model: String,

    /// Token usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsage>,

    /// Why the generation stopped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,

    /// Response metadata (latency, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<LLMMetadata>,
}

/// A single tool execution (request + result).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRecord {
    /// The tool call request.
    pub request: ToolRequestRecord,

    /// The result of the tool execution.
    pub result: ToolResult,

    /// Latency in milliseconds.
    pub latency_ms: u64,
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum ToolResult {
    /// Tool completed successfully.
    Completed(ToolCompletedRecord),
    /// Tool execution failed.
    Failed(ToolFailedRecord),
}

/// Outcome of an agent turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TurnOutcome {
    /// Tool calls were executed.
    ToolCalls { count: usize },
    /// Content was returned (no tool calls).
    Content,
    /// An error occurred.
    Error { message: String },
    /// Turn timed out.
    Timeout { elapsed_secs: u64 },
}

/// Patch phase data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchPhase {
    /// When patch collection started.
    pub started_at: String,

    /// When patch collection completed.
    pub ended_at: String,

    /// The complete patch artifact.
    pub patch_artifact: PatchArtifact,

    /// Git diff of changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// Validation phase data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationPhase {
    /// When validation started.
    pub started_at: String,

    /// When validation completed.
    pub ended_at: String,

    /// Build result.
    pub build_result: BuildResult,

    /// Test result.
    pub test_result: TestResult,

    /// Benchmark verdict (e.g., "passed", "failed").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_verdict: Option<String>,
}

/// Build validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum BuildResult {
    Success,
    Failed { exit_code: i32, stderr: String },
    Skipped { reason: String },
}

/// Test validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum TestResult {
    Passed,
    Failed { exit_code: i32, stdout: String, stderr: String },
    Skipped { reason: String },
}

/// Time-travel marker for Cozo DB `@` queries.
///
/// CozoDB's time-travel feature allows querying the database as it existed at any
/// historical point using the `@ timestamp` syntax. The `TimeTravelMarker` captures
/// the validity timestamp at key moments during a run, enabling replay and
/// introspection of code graph state at specific turns.
///
/// # Cozo Query Example
///
/// ```text
/// // Query what nodes existed at turn 3
/// ?[node] := *nodes{name: 'foo', node, @ 1744223415500000}
/// ```
///
/// The `timestamp_micros` field is the value used after the `@` operator.
///
/// # Usage in RunRecord
///
/// Markers are stored in [`RunRecord::db_time_travel_index`] and correspond to
/// entries in [`RunRecord::phases::agent_turns`] via the `turn` field.
///
/// # Events Captured
///
/// Typical events include:
/// - `"setup_complete"` — After initial checkout and indexing
/// - `"turn_start"` — Before sending prompt to LLM
/// - `"turn_complete"` — After receiving response and executing tools
/// - `"patch_applied"` — After edit proposals are applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeTravelMarker {
    /// Turn number this marker corresponds to.
    pub turn: u32,

    /// Cozo validity timestamp in microseconds since epoch.
    ///
    /// This value is used with Cozo's `@` operator to query historical state.
    pub timestamp_micros: i64,

    /// Event type marking what occurred at this timestamp.
    ///
    /// Common values: "turn_start", "turn_complete", "setup_complete", "patch_applied"
    pub event: String,
}

/// Simplified conversation message for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    /// Message ID.
    pub id: String,

    /// Role (user, assistant, system, tool).
    pub role: String,

    /// Message content.
    pub content: String,

    /// For tool calls: the tool call ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,

    /// For assistant messages: tool calls made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallRecord>>,
}

/// Tool call reference in conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    pub id: String,
    pub tool: String,
    pub arguments: serde_json::Value,
}

/// Run outcome summary (for quick reference without decompressing).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOutcomeSummary {
    /// Final status: "completed", "failed", "timeout", etc.
    pub status: String,

    /// Agent outcome: "solved", "partial", "failed", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_outcome: Option<String>,

    /// Benchmark verdict: "passed", "failed", etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark_verdict: Option<String>,

    /// Number of turns executed.
    pub turn_count: u32,

    /// Total token usage across all turns.
    pub total_token_usage: TokenUsage,

    /// Total tool calls made.
    pub total_tool_calls: u32,

    /// Wall clock time in seconds.
    pub wall_clock_secs: f64,
}

/// Extension trait for building RunRecords from runner artifacts.
pub trait RunRecordBuilder {
    /// Add a turn record from an AgentTurnArtifact.
    fn add_turn_from_artifact(&mut self, artifact: AgentTurnArtifact, db_timestamp_micros: i64);

    /// Finalize the run record with validation results.
    fn finalize(
        &mut self,
        validation: ValidationPhase,
        outcome: RunOutcomeSummary,
    ) -> &RunRecord;
}

impl RunRecordBuilder for RunRecord {
    fn add_turn_from_artifact(&mut self, artifact: AgentTurnArtifact, db_timestamp_micros: i64) {
        // Extract tool calls from artifact events
        let tool_calls = Vec::new();

        for event in &artifact.events {
            // This will be implemented when we wire up the runner
            // to populate structured tool execution records
            let _ = event; // Placeholder
        }

        let turn = TurnRecord {
            turn_number: self.phases.agent_turns.len() as u32 + 1,
            started_at: chrono::Utc::now().to_rfc3339(), // Will be captured properly in runner
            ended_at: chrono::Utc::now().to_rfc3339(),   // Will be captured properly in runner
            db_timestamp_micros,
            issue_prompt: artifact.issue_prompt.clone(),
            llm_request: None, // Populated from ChatEvt::Response handling
            llm_response: None, // Populated from ChatEvt::Response handling
            tool_calls,
            outcome: TurnOutcome::Content, // Determined from artifact
            agent_turn_artifact: Some(artifact),
        };

        self.phases.agent_turns.push(turn);
    }

    fn finalize(
        &mut self,
        validation: ValidationPhase,
        _outcome: RunOutcomeSummary,
    ) -> &RunRecord {
        self.phases.validation = Some(validation);
        self
    }
}
