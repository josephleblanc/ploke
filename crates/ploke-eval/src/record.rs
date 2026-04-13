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
//! # Access Examples
//!
//! Loading and introspecting a RunRecord:
//!
//! ```rust,ignore
//! // Load a RunRecord from disk
//! let record = read_compressed_record(&path)?;
//!
//! // Query what happened at turn 3
//! if let Some(turn) = record.turn_record(3) {
//!     println!("Turn 3 LLM response: {}", turn.llm_response.unwrap().content);
//! }
//!
//! // Get Cozo timestamp for historical DB queries
//! if let Some(timestamp) = record.timestamp_for_turn(3) {
//!     // Query: ?[node] := *nodes{name: 'foo', node, @ 1744223415500000}
//! }
//!
//! // Find all turns where search_code was used
//! let search_turns = record.turns_with_tool("search_code");
//!
//! // Get total token usage
//! let usage = record.total_token_usage();
//! println!("Total tokens: {}", usage.total_tokens);
//!
//! // Reconstruct state for replay
//! if let Some(state) = record.replay_state_at_turn(3) {
//!     println!("At turn {}: {} tool calls executed", state.turn, state.tool_calls.len());
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

// Re-export types from ploke-llm that we need for structured capture
pub use ploke_llm::manager::{RequestMessage, Role as LlmRole};
pub use ploke_llm::request::ChatCompReqCore;
pub use ploke_llm::response::{FinishReason, OpenAiResponse, TokenUsage};
pub use ploke_llm::types::meta::{LLMMetadata, PerformanceMetrics};
pub use ploke_llm::types::model_types::ModelId;

use crate::runner::{
    AgentTurnArtifact, IndexingStatusArtifact, ObservedTurnEvent, PatchArtifact, RepoStateArtifact,
    RunArm, SelectedEndpointProvenance, ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord,
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
    pub fn new(manifest: &PreparedSingleRun, run_arm: RunArm) -> Self {
        Self {
            schema_version: RUN_RECORD_SCHEMA_VERSION.to_string(),
            manifest_id: manifest.task_id.clone(),
            metadata: RunMetadata::from_manifest(manifest, run_arm),
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

    // =========================================================================
    // Introspection API (Phase 1F)
    // =========================================================================

    /// Get the Cozo DB timestamp for querying historical state at a specific turn.
    ///
    /// Returns the validity timestamp in microseconds that can be used with Cozo's
    /// `@` operator to query the database as it existed at that turn.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(timestamp) = record.timestamp_for_turn(3) {
    ///     // Query: ?[node] := *nodes{name: 'foo', node, @ 1744223415500000}
    /// }
    /// ```
    pub fn timestamp_for_turn(&self, turn: u32) -> Option<i64> {
        self.turn_timestamp(turn)
    }

    /// Get all tool calls executed in a specific turn.
    ///
    /// Returns an empty vector if the turn doesn't exist or had no tool calls.
    /// Turn numbers are 1-indexed (first turn is 1).
    pub fn tool_calls_in_turn(&self, turn: u32) -> Vec<ToolExecutionRecord> {
        self.phases
            .agent_turns
            .iter()
            .find(|t| t.turn_number == turn)
            .map(TurnRecord::tool_calls)
            .unwrap_or_default()
    }

    /// Get the TurnRecord for a specific turn.
    ///
    /// Turn numbers are 1-indexed (first turn is 1).
    pub fn turn_record(&self, turn: u32) -> Option<&TurnRecord> {
        self.phases
            .agent_turns
            .iter()
            .find(|t| t.turn_number == turn)
    }

    /// Get the LLM response for a specific turn, if any.
    ///
    /// Turn numbers are 1-indexed (first turn is 1).
    pub fn llm_response_at_turn(&self, turn: u32) -> Option<&LlmResponseRecord> {
        self.turn_record(turn).and_then(|t| t.llm_response.as_ref())
    }

    /// Get the total token usage across all turns.
    pub fn total_token_usage(&self) -> TokenUsage {
        let mut total = TokenUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        };

        for turn in &self.phases.agent_turns {
            if let Some(usage) = turn.llm_response.as_ref().and_then(|r| r.usage.as_ref()) {
                total.prompt_tokens += usage.prompt_tokens;
                total.completion_tokens += usage.completion_tokens;
                total.total_tokens += usage.total_tokens;
            }
        }

        total
    }

    /// Get the total number of turns in this run.
    pub fn turn_count(&self) -> u32 {
        self.phases.agent_turns.len() as u32
    }

    /// Get a summary of the run outcome.
    ///
    /// Returns high-level statistics without needing to decompress the full record.
    pub fn outcome_summary(&self) -> RunOutcomeSummary {
        let total_usage = self.total_token_usage();
        let total_tool_calls: u32 = self
            .phases
            .agent_turns
            .iter()
            .map(|t| t.tool_calls.len() as u32)
            .sum();

        // Determine status from validation phase or turn outcomes
        let status = if self.phases.validation.is_some() {
            "completed".to_string()
        } else if self.phases.agent_turns.is_empty() {
            "failed".to_string()
        } else {
            "incomplete".to_string()
        };

        RunOutcomeSummary {
            status,
            agent_outcome: None, // Would be populated from final turn outcome
            benchmark_verdict: self
                .phases
                .validation
                .as_ref()
                .map(|v| format!("{:?}", v.build_result)),
            turn_count: self.turn_count(),
            total_token_usage: total_usage,
            total_tool_calls,
            wall_clock_secs: 0.0, // Would need to track start/end times
        }
    }

    /// Iterate over all turns in order.
    ///
    /// Returns an iterator that yields each `TurnRecord` in chronological order,
    /// from turn 1 to the final turn.
    pub fn conversations(&self) -> impl Iterator<Item = &TurnRecord> {
        self.phases.agent_turns.iter()
    }

    /// Aggregate ALL tool calls from all turns.
    ///
    /// Returns a vector of every `ToolExecutionRecord` executed across all turns
    /// in the run, in chronological order.
    ///
    /// This method extracts tool calls from `agent_turn_artifact.events` to work
    /// with existing records where the `tool_calls` field may not be populated.
    pub fn tool_calls(&self) -> Vec<ToolExecutionRecord> {
        self.phases
            .agent_turns
            .iter()
            .flat_map(|t| {
                // First try the direct field (for new records)
                if !t.tool_calls.is_empty() {
                    t.tool_calls.clone()
                } else if let Some(artifact) = &t.agent_turn_artifact {
                    // Extract from artifact events (for existing records)
                    extract_tool_calls_from_events(&artifact.events)
                } else {
                    Vec::new()
                }
            })
            .collect()
    }

    /// Get DbState for each turn from db_time_travel_index.
    ///
    /// Returns a vector of `DbState` objects representing the database state
    /// at each time-travel marker in the index.
    pub fn db_snapshots(&self) -> Vec<DbState> {
        self.db_time_travel_index
            .iter()
            .map(|m| DbState::new(m.timestamp_micros))
            .collect()
    }

    /// Filter turns with error outcomes.
    ///
    /// Returns a vector of references to `TurnRecord`s where the outcome
    /// was `TurnOutcome::Error`.
    pub fn failures(&self) -> Vec<&TurnRecord> {
        self.phases
            .agent_turns
            .iter()
            .filter(|t| matches!(t.outcome, TurnOutcome::Error { .. }))
            .collect()
    }

    /// Get the frozen run configuration.
    ///
    /// Returns a reference to the `RunMetadata` containing benchmark, agent,
    /// and runtime configuration for this run.
    pub fn config(&self) -> &RunMetadata {
        &self.metadata
    }

    /// Check if a specific tool was called in any turn.
    pub fn was_tool_used(&self, tool_name: &str) -> bool {
        self.phases.agent_turns.iter().any(|turn| {
            turn.tool_calls()
                .iter()
                .any(|call| call.request.tool == tool_name)
        })
    }

    /// Get all turns where a specific tool was called.
    pub fn turns_with_tool(&self, tool_name: &str) -> Vec<u32> {
        self.phases
            .agent_turns
            .iter()
            .filter(|turn| {
                turn.tool_calls()
                    .iter()
                    .any(|call| call.request.tool == tool_name)
            })
            .map(|turn| turn.turn_number)
            .collect()
    }

    /// Reconstruct the state at a specific turn for replay or introspection.
    ///
    /// Returns a `ReplayState` containing everything needed to understand or resume
    /// execution from that point. Turn numbers are 1-indexed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Get state at turn 3
    /// if let Some(state) = record.replay_state_at_turn(3) {
    ///     println!("At turn {}, the LLM said: {}",
    ///         state.turn,
    ///         state.llm_response.map(|r| r.content).unwrap_or_default()
    ///     );
    /// }
    /// ```
    pub fn replay_state_at_turn(&self, turn: u32) -> Option<ReplayState> {
        let turn_record = self.turn_record(turn)?;
        let timestamp = self.timestamp_for_turn(turn)?;
        let conversation_up_to_turn = turn_record.messages();
        let conversation_up_to_turn = if conversation_up_to_turn.is_empty()
            && turn == self.turn_count()
            && !self.conversation.is_empty()
        {
            self.conversation.clone()
        } else {
            conversation_up_to_turn
        };

        Some(ReplayState {
            turn,
            db_timestamp_micros: timestamp,
            issue_prompt: turn_record.issue_prompt.clone(),
            llm_request: turn_record.llm_request.clone(),
            llm_response: turn_record.llm_response.clone(),
            tool_calls: turn_record.tool_calls(),
            conversation_up_to_turn,
            repo_root: self.metadata.benchmark.repo_root.clone(),
            base_sha: self.metadata.benchmark.base_sha.clone(),
        })
    }

    /// Execute a query against the DB as it existed at a specific turn.
    ///
    /// This method enables replay introspection by querying the database state
    /// at the timestamp recorded for a given turn.
    ///
    /// # Arguments
    ///
    /// * `turn` - The turn number (1-indexed) to query at
    /// * `db` - A reference to the Database to query against
    /// * `query` - The CozoScript query to execute. Use `@ 'NOW'` for the historical timestamp.
    ///
    /// # Returns
    ///
    /// Returns `Ok(QueryResult)` on success, or `Err(ReplayError)` if:
    /// - The turn doesn't exist in the run record
    /// - No timestamp is recorded for the turn
    /// - The database query fails
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let result = run_record.replay_query(1, &db, "?[name] := *nodes{name @ 'NOW'}")?;
    /// for row in result.rows {
    ///     println!("{:?}", row);
    /// }
    /// ```
    pub fn replay_query(
        &self,
        turn: u32,
        db: &ploke_db::Database,
        query: &str,
    ) -> Result<ploke_db::QueryResult, ReplayError> {
        // 1. Get timestamp for turn from db_time_travel_index
        let timestamp = self
            .timestamp_for_turn(turn)
            .ok_or(ReplayError::TimestampNotFound(turn))?;

        // 2. Use db.raw_query_at_timestamp(query, timestamp)
        let result = db.raw_query_at_timestamp(query, timestamp)?;

        // 3. Return result
        Ok(result)
    }
}

/// Metadata extracted from the run manifest.
/// Extract tool execution records from observed turn events.
///
/// Matches ToolRequested events with ToolCompleted/ToolFailed events by call_id
/// to reconstruct complete tool execution records.
fn extract_tool_calls_from_events(events: &[ObservedTurnEvent]) -> Vec<ToolExecutionRecord> {
    use std::collections::HashMap;

    let mut pending_requests: HashMap<String, ToolRequestRecord> = HashMap::new();
    let mut tool_calls = Vec::new();

    for event in events {
        match event {
            ObservedTurnEvent::ToolRequested(req) => {
                pending_requests.insert(req.call_id.clone(), req.clone());
            }
            ObservedTurnEvent::ToolCompleted(completed) => {
                if let Some(request) = pending_requests.remove(&completed.call_id) {
                    tool_calls.push(ToolExecutionRecord {
                        request,
                        result: ToolResult::Completed(completed.clone()),
                        latency_ms: 0, // Latency not captured in current events
                    });
                }
            }
            ObservedTurnEvent::ToolFailed(failed) => {
                if let Some(request) = pending_requests.remove(&failed.call_id) {
                    tool_calls.push(ToolExecutionRecord {
                        request,
                        result: ToolResult::Failed(failed.clone()),
                        latency_ms: 0,
                    });
                }
            }
            _ => {}
        }
    }

    tool_calls
}

fn extract_llm_response_from_events(events: &[ObservedTurnEvent]) -> Option<LlmResponseRecord> {
    events.iter().rev().find_map(|event| match event {
        ObservedTurnEvent::LlmResponse(record) => Some(record.clone()),
        _ => None,
    })
}

fn turn_outcome_from_artifact(artifact: &AgentTurnArtifact, tool_call_count: usize) -> TurnOutcome {
    if let Some(terminal) = artifact.terminal_record.as_ref() {
        match terminal.outcome.as_str() {
            "completed" if tool_call_count > 0 => TurnOutcome::ToolCalls {
                count: tool_call_count,
            },
            "completed" => TurnOutcome::Content,
            "aborted" | "exhausted" => TurnOutcome::Error {
                message: terminal.summary.clone(),
            },
            other => TurnOutcome::Error {
                message: format!("Unexpected turn outcome: {other}"),
            },
        }
    } else if tool_call_count > 0 {
        TurnOutcome::ToolCalls {
            count: tool_call_count,
        }
    } else {
        TurnOutcome::Content
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetadata {
    /// Explicit arm provenance for control vs treatment recovery.
    pub run_arm: RunArm,

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
    pub fn from_manifest(manifest: &PreparedSingleRun, run_arm: RunArm) -> Self {
        Self {
            run_arm,
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
    #[serde(
        rename = "selected_model",
        alias = "model_id",
        skip_serializing_if = "Option::is_none"
    )]
    pub model_id: Option<ModelId>,

    /// Provider used (e.g., "openrouter", "anthropic").
    #[serde(
        rename = "selected_provider",
        alias = "provider",
        skip_serializing_if = "Option::is_none"
    )]
    pub provider: Option<String>,

    /// Selected endpoint provenance from provider resolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_endpoint: Option<SelectedEndpointProvenance>,

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

/// Per-crate summary for SetupPhase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexedCrateSummary {
    pub name: String,
    pub version: String,
    pub namespace: Uuid,
    pub root_path: PathBuf,
    pub file_count: usize,
    pub node_count: usize,
    pub embedded_count: usize,
    pub status: CrateIndexStatus,
    pub parse_error: Option<ParseErrorSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CrateIndexStatus {
    Success,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseErrorSummary {
    pub message: String,
    pub target_dir: PathBuf,
    pub occurred_at_ms: i64,
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

    /// Indexed crate summaries.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub indexed_crates: Vec<IndexedCrateSummary>,

    /// Any parse failures during indexing.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub parse_failures: Vec<ParseFailureRecord>,

    /// Cozo timestamp at setup completion.
    pub db_timestamp_micros: i64,

    /// Tool schema version used during setup.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_schema_version: Option<String>,
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

impl TurnRecord {
    /// Get a queryable DB snapshot for this turn's timestamp.
    ///
    /// Returns a `DbState` that can be used to query the database as it existed
    /// at the time this turn was recorded. Use this for introspection and replay.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let db_state = turn.db_state();
    /// if let Some(node) = db_state.lookup(&db, "my_function")? {
    ///     println!("Found node: {} (type: {})", node.name, node.node_type);
    /// }
    /// ```
    pub fn db_state(&self) -> DbState {
        DbState::new(self.db_timestamp_micros)
    }

    /// Reconstruct conversation history up to this turn.
    ///
    /// Returns a vector of `ConversationMessage` representing the conversation
    /// history up to and including this turn. This is reconstructed from
    /// the `agent_turn_artifact.llm_prompt` and `agent_turn_artifact.llm_response`.
    ///
    /// # Implementation
    ///
    /// Converts `RequestMessage` items from `llm_prompt` to `ConversationMessage`,
    /// mapping roles appropriately. If `llm_response` is present, appends it as
    /// an assistant message.
    pub fn messages(&self) -> Vec<ConversationMessage> {
        use ploke_tui::chat_history::MessageKind;

        let Some(artifact) = &self.agent_turn_artifact else {
            return Vec::new();
        };

        let mut messages: Vec<ConversationMessage> = artifact
            .llm_prompt
            .iter()
            .filter_map(|req_msg| {
                // Map RequestMessage role to MessageKind
                let kind = match req_msg.role {
                    LlmRole::System => MessageKind::System,
                    LlmRole::User => MessageKind::User,
                    LlmRole::Assistant => MessageKind::Assistant,
                    LlmRole::Tool => MessageKind::Tool,
                };

                Some(ConversationMessage {
                    id: Uuid::new_v4(),
                    branch_id: Uuid::nil(),
                    status: ploke_tui::chat_history::MessageStatus::Completed,
                    metadata: None,
                    parent: None,
                    children: Vec::new(),
                    selected_child: None,
                    content: req_msg.content.clone(),
                    kind,
                    tool_call_id: req_msg.tool_call_id.clone(),
                    tool_payload: None,
                    context_status: ploke_tui::chat_history::ContextStatus::default(),
                    last_included_turn: None,
                    include_count: 0,
                })
            })
            .collect();

        // Append the LLM response if present
        if let Some(response_content) = &artifact.llm_response {
            messages.push(ConversationMessage {
                id: Uuid::new_v4(),
                branch_id: Uuid::nil(),
                status: ploke_tui::chat_history::MessageStatus::Completed,
                metadata: None,
                parent: None,
                children: Vec::new(),
                selected_child: None,
                content: response_content.clone(),
                kind: MessageKind::Assistant,
                tool_call_id: None,
                tool_payload: None,
                context_status: ploke_tui::chat_history::ContextStatus::default(),
                last_included_turn: None,
                include_count: 0,
            });
        }

        messages
    }

    /// Get all tool calls for this turn.
    ///
    /// Returns tool calls from `self.tool_calls` if populated, otherwise
    /// extracts from `agent_turn_artifact.events` for compatibility with
    /// existing records.
    pub fn tool_calls(&self) -> Vec<ToolExecutionRecord> {
        if !self.tool_calls.is_empty() {
            self.tool_calls.clone()
        } else if let Some(artifact) = &self.agent_turn_artifact {
            extract_tool_calls_from_events(&artifact.events)
        } else {
            Vec::new()
        }
    }

    /// Get single tool call if this turn had exactly one.
    ///
    /// Returns `Some(ToolExecutionRecord)` if this turn executed exactly
    /// one tool call, otherwise returns `None`.
    pub fn tool_call(&self) -> Option<ToolExecutionRecord> {
        let calls = self.tool_calls();
        if calls.len() == 1 {
            calls.into_iter().next()
        } else {
            None
        }
    }

    /// Get result from single tool call.
    ///
    /// Returns `Some(ToolResult)` if this turn had exactly one tool call,
    /// returning its result. Returns `None` if there were zero or multiple
    /// tool calls.
    pub fn tool_result(&self) -> Option<ToolResult> {
        self.tool_call().map(|tc| tc.result)
    }
}

/// Wrapper for querying DB at a specific historical timestamp.
///
/// `DbState` provides a lightweight handle for querying the database as it existed
/// at a specific point in time. It does not hold a reference to the database itself;
/// instead, you pass a `&Database` to its query methods.
///
/// This design avoids lifetime issues while still enabling convenient introspection
/// of historical database states during replay.
///
/// # Usage
///
/// ```rust,ignore
/// let db_state = turn.db_state();
///
/// // Look up a specific node by name
/// if let Some(info) = db_state.lookup(&db, "my_function")? {
///     println!("Found: {:?}", info);
/// }
///
/// // Execute arbitrary queries
/// let results = db_state.query(&db, "?[name] := *nodes{name @ 'NOW'}")?;
/// ```
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DbState {
    timestamp_micros: i64,
}

impl DbState {
    /// Create a new `DbState` for the given timestamp.
    pub fn new(timestamp_micros: i64) -> Self {
        Self { timestamp_micros }
    }

    /// Get the timestamp for this DB state.
    pub fn timestamp_micros(&self) -> i64 {
        self.timestamp_micros
    }

    /// Query for a node by name at this timestamp.
    ///
    /// Looks up a node in the database by its name, returning `NodeInfo` if found.
    /// Returns `Ok(None)` if no node with the given name exists at this timestamp.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference to the Database to query against
    /// * `name` - The name of the node to look up
    ///
    /// # Returns
    ///
    /// Returns `Ok(Some(NodeInfo))` if a node is found, `Ok(None)` if not found,
    /// or `Err(DbError)` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let db_state = turn.db_state();
    /// match db_state.lookup(&db, "handle_request")? {
    ///     Some(info) => println!("Found {} (type: {})", info.name, info.node_type),
    ///     None => println!("Node not found at this timestamp"),
    /// }
    /// ```
    pub fn lookup(
        &self,
        db: &ploke_db::Database,
        name: &str,
    ) -> Result<Option<NodeInfo>, ploke_db::DbError> {
        use cozo::DataValue;

        // Escape quotes in name to prevent injection
        let escaped_name = name.replace('"', "\\\"");

        // Primary node relations to query, in order of priority
        const NODE_RELATIONS: &[&str] = &[
            "function",
            "struct",
            "enum",
            "trait",
            "method",
            "const",
            "static",
            "macro",
            "type_alias",
        ];

        // Try each relation in order, return first match
        for relation in NODE_RELATIONS {
            let query = format!(
                r#"?[id, name] := *{relation}{{id, name @ 'NOW'}}, name = "{}""#,
                escaped_name
            );

            let result = db.raw_query_at_timestamp(&query, self.timestamp_micros)?;

            // If we got a result, extract the first row
            if !result.rows.is_empty() {
                let row = &result.rows[0];

                // Get column indices from headers
                let id_idx = result.headers.iter().position(|h| h == "id");
                let name_idx = result.headers.iter().position(|h| h == "name");

                // Extract UUID from id column
                let id = id_idx
                    .and_then(|idx| row.get(idx))
                    .and_then(|val| match val {
                        DataValue::Uuid(uuid_wrapper) => Some(uuid_wrapper.0),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        ploke_db::DbError::Cozo(format!(
                            "Expected UUID for id column in {}",
                            relation
                        ))
                    })?;

                // Extract name from name column
                let node_name = name_idx
                    .and_then(|idx| row.get(idx))
                    .and_then(|val| match val {
                        DataValue::Str(s) => Some(s.to_string()),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        ploke_db::DbError::Cozo(format!(
                            "Expected string for name column in {}",
                            relation
                        ))
                    })?;

                return Ok(Some(NodeInfo {
                    id,
                    name: node_name,
                    node_type: relation.to_string(),
                }));
            }
        }

        // No match found in any relation
        Ok(None)
    }

    /// Execute an arbitrary query at this timestamp.
    ///
    /// This method allows executing any CozoScript query against the database
    /// as it existed at the timestamp stored in this `DbState`. Use `@ 'NOW'`
    /// in your query to reference the historical timestamp.
    ///
    /// # Arguments
    ///
    /// * `db` - A reference to the Database to query against
    /// * `query` - The CozoScript query to execute. Use `@ 'NOW'` for the historical timestamp.
    ///
    /// # Returns
    ///
    /// Returns `Ok(QueryResult)` on success, or `Err(DbError)` if the query fails.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let db_state = turn.db_state();
    /// let result = db_state.query(&db, "?[name] := *nodes{name @ 'NOW'}")?;
    /// for row in result.rows {
    ///     println!("{:?}", row);
    /// }
    /// ```
    pub fn query(
        &self,
        db: &ploke_db::Database,
        query: &str,
    ) -> Result<ploke_db::QueryResult, ploke_db::DbError> {
        db.raw_query_at_timestamp(query, self.timestamp_micros)
    }
}

/// Information about a node found in the DB.
///
/// This struct contains the essential information about a code graph node
/// returned by `DbState::lookup()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// The unique identifier of the node.
    pub id: Uuid,

    /// The name of the node.
    pub name: String,

    /// The type of the node (e.g., "Function", "Struct", "Module").
    pub node_type: String,
}

/// Error type for replay operations.
///
/// This enum represents the various errors that can occur when replaying
/// or introspecting historical run data.
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    /// The requested turn was not found in the run record.
    #[error("Turn {0} not found in run record")]
    TurnNotFound(u32),

    /// A database error occurred during the replay operation.
    #[error("Database error: {0}")]
    DbError(#[from] ploke_db::DbError),

    /// The timestamp for the requested turn was not found.
    #[error("Timestamp not found for turn {0}")]
    TimestampNotFound(u32),
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
    Failed {
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
    Skipped {
        reason: String,
    },
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

// Re-export Message from ploke-tui for conversation capture
pub use ploke_tui::chat_history::Message as ConversationMessage;

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

/// Replay state at a specific turn — everything needed to resume or introspect.
///
/// This struct captures the complete state of an eval run at a specific turn,
/// enabling replay from that point without re-running earlier turns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayState {
    /// The turn number this state represents.
    pub turn: u32,

    /// Cozo DB timestamp for querying historical code graph state.
    pub db_timestamp_micros: i64,

    /// The issue prompt for this turn.
    pub issue_prompt: String,

    /// LLM request that was sent (if captured).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_request: Option<ChatCompReqCore>,

    /// LLM response that was received (if captured).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_response: Option<LlmResponseRecord>,

    /// Tool calls executed in this turn.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tool_calls: Vec<ToolExecutionRecord>,

    /// Conversation history up to and including this turn.
    pub conversation_up_to_turn: Vec<ConversationMessage>,

    /// Repository state at this point in the run.
    pub repo_root: PathBuf,

    /// Base SHA that was checked out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
}

/// Extension trait for building RunRecords from runner artifacts.
pub trait RunRecordBuilder {
    /// Add a turn record from an AgentTurnArtifact.
    fn add_turn_from_artifact(&mut self, artifact: AgentTurnArtifact, db_timestamp_micros: i64);

    /// Finalize the run record with validation results.
    fn finalize(&mut self, validation: ValidationPhase, outcome: RunOutcomeSummary) -> &RunRecord;
}

impl RunRecordBuilder for RunRecord {
    fn add_turn_from_artifact(&mut self, artifact: AgentTurnArtifact, db_timestamp_micros: i64) {
        let tool_calls = extract_tool_calls_from_events(&artifact.events);
        let llm_request = (!artifact.llm_prompt.is_empty()).then(|| {
            ChatCompReqCore::default()
                .with_messages(artifact.llm_prompt.clone())
                .with_model(artifact.selected_model.clone())
        });
        let llm_response = extract_llm_response_from_events(&artifact.events);
        let outcome = turn_outcome_from_artifact(&artifact, tool_calls.len());

        let turn = TurnRecord {
            turn_number: self.phases.agent_turns.len() as u32 + 1,
            started_at: chrono::Utc::now().to_rfc3339(), // Will be captured properly in runner
            ended_at: chrono::Utc::now().to_rfc3339(),   // Will be captured properly in runner
            db_timestamp_micros,
            issue_prompt: artifact.issue_prompt.clone(),
            llm_request,
            llm_response,
            tool_calls,
            outcome,
            agent_turn_artifact: Some(artifact),
        };

        self.phases.agent_turns.push(turn);
    }

    fn finalize(&mut self, validation: ValidationPhase, _outcome: RunOutcomeSummary) -> &RunRecord {
        self.phases.validation = Some(validation);
        self
    }
}

/// Write a RunRecord to a compressed JSON file (record.json.gz).
///
/// Uses flate2 for gzip compression with default compression level.
/// The resulting file can be read back with `read_compressed_record`.
///
/// # Example
///
/// ```rust,ignore
/// let record = RunRecord::new(&prepared_run);
/// write_compressed_record(&path, &record)?;
/// ```
pub fn write_compressed_record(
    path: &std::path::Path,
    record: &RunRecord,
) -> Result<(), std::io::Error> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs::File;

    let file = File::create(path)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    serde_json::to_writer(&mut encoder, record)?;
    encoder.finish()?;
    Ok(())
}

/// Read a RunRecord from a compressed JSON file (record.json.gz).
///
/// # Example
///
/// ```rust,ignore
/// let record = read_compressed_record(&path)?;
/// ```
pub fn read_compressed_record(path: &std::path::Path) -> Result<RunRecord, std::io::Error> {
    use flate2::read::GzDecoder;
    use std::fs::File;

    let file = File::open(path)?;
    let decoder = GzDecoder::new(file);
    let record = serde_json::from_reader(decoder)?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Creates a minimal RunRecord for testing without needing a full PreparedSingleRun.
    fn create_test_record() -> RunRecord {
        RunRecord {
            schema_version: RUN_RECORD_SCHEMA_VERSION.to_string(),
            manifest_id: "test-instance-001".to_string(),
            metadata: RunMetadata {
                run_arm: RunArm::shell_only_control(),
                benchmark: BenchmarkMetadata {
                    instance_id: "test-instance-001".to_string(),
                    repo_root: std::path::PathBuf::from("/test/repo"),
                    base_sha: Some("abc123".to_string()),
                    issue: None,
                },
                agent: AgentMetadata::default(),
                runtime: RuntimeMetadata::default(),
                budget: crate::spec::EvalBudget::default(),
            },
            phases: RunPhases::default(),
            db_time_travel_index: vec![TimeTravelMarker {
                turn: 1,
                timestamp_micros: 1744223415500000,
                event: "turn_complete".to_string(),
            }],
            conversation: Vec::new(),
        }
    }

    fn create_test_turn_artifact() -> AgentTurnArtifact {
        AgentTurnArtifact {
            task_id: "test-instance-001".to_string(),
            selected_model: ModelId::from_str("anthropic/claude-sonnet-4").unwrap(),
            issue_prompt: "Fix the bug in src/lib.rs".to_string(),
            user_message_id: "user-001".to_string(),
            events: vec![
                ObservedTurnEvent::LlmResponse(LlmResponseRecord {
                    content: "I found the issue and will inspect the code.".to_string(),
                    model: "anthropic/claude-sonnet-4".to_string(),
                    usage: Some(TokenUsage {
                        prompt_tokens: 42,
                        completion_tokens: 7,
                        total_tokens: 49,
                    }),
                    finish_reason: Some(FinishReason::ToolCalls),
                    metadata: None,
                }),
                ObservedTurnEvent::ToolRequested(ToolRequestRecord {
                    request_id: "req-001".to_string(),
                    parent_id: "parent-001".to_string(),
                    call_id: "call-001".to_string(),
                    tool: "search_code".to_string(),
                    arguments: r#"{"query":"handle_request"}"#.to_string(),
                }),
                ObservedTurnEvent::ToolCompleted(ToolCompletedRecord {
                    request_id: "req-001".to_string(),
                    parent_id: "parent-001".to_string(),
                    call_id: "call-001".to_string(),
                    tool: "search_code".to_string(),
                    content: "Found function in src/lib.rs".to_string(),
                    ui_payload: None,
                }),
                ObservedTurnEvent::TurnFinished(crate::runner::TurnFinishedRecord {
                    session_id: "session-001".to_string(),
                    request_id: "req-001".to_string(),
                    parent_id: "parent-001".to_string(),
                    assistant_message_id: "assistant-001".to_string(),
                    outcome: "completed".to_string(),
                    error_id: None,
                    summary: "Request summary: [success]".to_string(),
                    attempts: 1,
                }),
            ],
            prompt_debug: None,
            terminal_record: Some(crate::runner::TurnFinishedRecord {
                session_id: "session-001".to_string(),
                request_id: "req-001".to_string(),
                parent_id: "parent-001".to_string(),
                assistant_message_id: "assistant-001".to_string(),
                outcome: "completed".to_string(),
                error_id: None,
                summary: "Request summary: [success]".to_string(),
                attempts: 1,
            }),
            final_assistant_message: None,
            patch_artifact: crate::runner::PatchArtifact {
                edit_proposals: Vec::new(),
                create_proposals: Vec::new(),
                applied: false,
                all_proposals_applied: false,
                expected_file_changes: Vec::new(),
                any_expected_file_changed: false,
                all_expected_files_changed: false,
            },
            llm_prompt: vec![
                RequestMessage::new_system("You are a coding assistant.".to_string()),
                RequestMessage::new_user("Fix the bug in src/lib.rs".to_string()),
            ],
            llm_response: Some("I found the issue and will inspect the code.".to_string()),
        }
    }

    #[test]
    fn write_and_read_compressed_record_roundtrip() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let record_path = temp_dir.path().join("record.json.gz");

        let original = create_test_record();

        // Write compressed record
        write_compressed_record(&record_path, &original)
            .expect("Failed to write compressed record");

        // Verify file exists and has content
        assert!(record_path.exists());
        let file_size = std::fs::metadata(&record_path)
            .expect("Failed to read metadata")
            .len();
        assert!(file_size > 0, "Compressed file should have content");

        // Read back and verify
        let loaded =
            read_compressed_record(&record_path).expect("Failed to read compressed record");

        // Verify key fields match
        assert_eq!(loaded.schema_version, original.schema_version);
        assert_eq!(loaded.manifest_id, original.manifest_id);
        assert_eq!(loaded.metadata.run_arm, original.metadata.run_arm);
        assert_eq!(
            loaded.metadata.benchmark.instance_id,
            original.metadata.benchmark.instance_id
        );
        assert_eq!(loaded.db_time_travel_index.len(), 1);
        assert_eq!(loaded.db_time_travel_index[0].turn, 1);
        assert_eq!(
            loaded.db_time_travel_index[0].timestamp_micros,
            1744223415500000
        );
    }

    #[test]
    fn compressed_record_achieves_compression_ratio() {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let record_path = temp_dir.path().join("record.json.gz");
        let json_path = temp_dir.path().join("record.json");

        let mut record = create_test_record();
        // Add some repetitive content to make compression effective
        record.phases.agent_turns.push(TurnRecord {
            turn_number: 1,
            started_at: "2026-04-09T18:30:15Z".to_string(),
            ended_at: "2026-04-09T18:30:45Z".to_string(),
            db_timestamp_micros: 1744223415500000,
            issue_prompt:
                "Fix the bug in src/lib.rs where the function does not handle edge cases properly."
                    .to_string(),
            llm_request: None,
            llm_response: Some(LlmResponseRecord {
                content: "I'll help you fix the bug. Let me start by examining the code."
                    .to_string(),
                model: "anthropic/claude-sonnet-4".to_string(),
                usage: Some(TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                }),
                finish_reason: Some(FinishReason::Stop),
                metadata: None,
            }),
            tool_calls: Vec::new(),
            outcome: TurnOutcome::Content,
            agent_turn_artifact: None,
        });

        // Write uncompressed JSON for comparison
        let json_file = std::fs::File::create(&json_path).expect("Failed to create JSON file");
        serde_json::to_writer(json_file, &record).expect("Failed to write JSON");

        // Write compressed record
        write_compressed_record(&record_path, &record).expect("Failed to write compressed record");

        let json_size = std::fs::metadata(&json_path)
            .expect("Failed to read JSON metadata")
            .len();
        let compressed_size = std::fs::metadata(&record_path)
            .expect("Failed to read compressed metadata")
            .len();

        // Verify compression achieved (compressed should be smaller)
        assert!(
            compressed_size < json_size,
            "Compression should reduce file size: JSON={}, compressed={}",
            json_size,
            compressed_size
        );
    }

    #[test]
    fn run_record_new_captures_explicit_run_arm() {
        let manifest = PreparedSingleRun {
            task_id: "case-123".to_string(),
            repo_root: std::path::PathBuf::from("/tmp/repo"),
            output_dir: std::path::PathBuf::from("/tmp/out"),
            issue: crate::spec::IssueInput {
                title: Some("Fix the thing".to_string()),
                body: Some("Body".to_string()),
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: None,
            budget: crate::spec::EvalBudget::default(),
            source: None,
        };

        let record = RunRecord::new(&manifest, RunArm::structured_current_policy_treatment());
        assert_eq!(
            record.metadata.run_arm,
            RunArm::structured_current_policy_treatment()
        );
        assert_eq!(record.metadata.benchmark.instance_id, "case-123");
    }

    #[test]
    fn agent_metadata_roundtrips_selected_endpoint_provenance() {
        let metadata = AgentMetadata {
            model_id: Some(ModelId::from_str("moonshotai/kimi-k2.5").expect("model id")),
            provider: Some("modelrun".to_string()),
            selected_endpoint: Some(SelectedEndpointProvenance {
                provider_name: "ModelRun".to_string(),
                provider_slug: "modelrun".to_string(),
                endpoint_name: "ModelRun | moonshotai/kimi-k2.5".to_string(),
                endpoint_model_name: "Kimi K2.5".to_string(),
                quantization: Some("fp4".to_string()),
            }),
            system_prompt_version: None,
            tool_schema_version: None,
        };

        let json = serde_json::to_string(&metadata).expect("serialize agent metadata");
        assert!(json.contains("\"selected_endpoint\""));
        assert!(json.contains("\"selected_model\":\"moonshotai/kimi-k2.5\""));
        assert!(json.contains("\"selected_provider\":\"modelrun\""));
        assert!(json.contains("\"quantization\":\"fp4\""));

        let roundtrip: AgentMetadata =
            serde_json::from_str(&json).expect("deserialize agent metadata");
        assert_eq!(
            roundtrip.model_id,
            Some(ModelId::from_str("moonshotai/kimi-k2.5").expect("model id"))
        );
        assert_eq!(roundtrip.provider.as_deref(), Some("modelrun"));
        assert_eq!(
            roundtrip
                .selected_endpoint
                .expect("selected endpoint")
                .quantization
                .as_deref(),
            Some("fp4")
        );
    }

    #[test]
    fn agent_metadata_accepts_legacy_model_and_provider_keys() {
        let legacy_json = r#"{
            "model_id": "moonshotai/kimi-k2.5",
            "provider": "modelrun",
            "selected_endpoint": {
                "provider_name": "ModelRun",
                "provider_slug": "modelrun",
                "endpoint_name": "ModelRun | moonshotai/kimi-k2.5",
                "endpoint_model_name": "Kimi K2.5",
                "quantization": "fp4"
            }
        }"#;

        let roundtrip: AgentMetadata =
            serde_json::from_str(legacy_json).expect("deserialize legacy agent metadata");
        assert_eq!(
            roundtrip.model_id,
            Some(ModelId::from_str("moonshotai/kimi-k2.5").expect("model id"))
        );
        assert_eq!(roundtrip.provider.as_deref(), Some("modelrun"));
        assert_eq!(
            roundtrip
                .selected_endpoint
                .expect("selected endpoint")
                .quantization
                .as_deref(),
            Some("fp4")
        );
    }

    // ========================================================================
    // Introspection API Tests (Phase 1F)
    // ========================================================================

    /// Creates a RunRecord with multiple turns for testing introspection methods.
    fn create_test_record_with_turns() -> RunRecord {
        let mut record = create_test_record();

        // Add time travel markers for turns 1-3
        record.db_time_travel_index = vec![
            TimeTravelMarker {
                turn: 1,
                timestamp_micros: 1744223415500000,
                event: "turn_complete".to_string(),
            },
            TimeTravelMarker {
                turn: 2,
                timestamp_micros: 1744223415600000,
                event: "turn_complete".to_string(),
            },
            TimeTravelMarker {
                turn: 3,
                timestamp_micros: 1744223415700000,
                event: "turn_complete".to_string(),
            },
        ];

        // Add turn 1: Initial analysis, no tool calls
        record.phases.agent_turns.push(TurnRecord {
            turn_number: 1,
            started_at: "2026-04-09T18:30:15Z".to_string(),
            ended_at: "2026-04-09T18:30:20Z".to_string(),
            db_timestamp_micros: 1744223415500000,
            issue_prompt: "Fix the bug in src/lib.rs".to_string(),
            llm_request: None,
            llm_response: Some(LlmResponseRecord {
                content: "I'll analyze the code.".to_string(),
                model: "anthropic/claude-sonnet-4".to_string(),
                usage: Some(TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 20,
                    total_tokens: 120,
                }),
                finish_reason: Some(FinishReason::Stop),
                metadata: None,
            }),
            tool_calls: Vec::new(),
            outcome: TurnOutcome::Content,
            agent_turn_artifact: None,
        });

        // Add turn 2: Uses search tool
        record.phases.agent_turns.push(TurnRecord {
            turn_number: 2,
            started_at: "2026-04-09T18:30:21Z".to_string(),
            ended_at: "2026-04-09T18:30:30Z".to_string(),
            db_timestamp_micros: 1744223415600000,
            issue_prompt: "Fix the bug in src/lib.rs".to_string(),
            llm_request: None,
            llm_response: Some(LlmResponseRecord {
                content: "Let me search for the function.".to_string(),
                model: "anthropic/claude-sonnet-4".to_string(),
                usage: Some(TokenUsage {
                    prompt_tokens: 120,
                    completion_tokens: 30,
                    total_tokens: 150,
                }),
                finish_reason: Some(FinishReason::ToolCalls),
                metadata: None,
            }),
            tool_calls: vec![ToolExecutionRecord {
                request: ToolRequestRecord {
                    request_id: "req-001".to_string(),
                    parent_id: "parent-001".to_string(),
                    call_id: "call-001".to_string(),
                    tool: "search_code".to_string(),
                    arguments: r#"{"query": "fn handle_request"}"#.to_string(),
                },
                result: ToolResult::Completed(ToolCompletedRecord {
                    request_id: "req-001".to_string(),
                    parent_id: "parent-001".to_string(),
                    call_id: "call-001".to_string(),
                    tool: "search_code".to_string(),
                    content: "Found function at line 42".to_string(),
                    ui_payload: None,
                }),
                latency_ms: 150,
            }],
            outcome: TurnOutcome::ToolCalls { count: 1 },
            agent_turn_artifact: None,
        });

        // Add turn 3: Uses edit tool
        record.phases.agent_turns.push(TurnRecord {
            turn_number: 3,
            started_at: "2026-04-09T18:30:31Z".to_string(),
            ended_at: "2026-04-09T18:30:45Z".to_string(),
            db_timestamp_micros: 1744223415700000,
            issue_prompt: "Fix the bug in src/lib.rs".to_string(),
            llm_request: None,
            llm_response: Some(LlmResponseRecord {
                content: "Now I'll apply the fix.".to_string(),
                model: "anthropic/claude-sonnet-4".to_string(),
                usage: Some(TokenUsage {
                    prompt_tokens: 150,
                    completion_tokens: 40,
                    total_tokens: 190,
                }),
                finish_reason: Some(FinishReason::ToolCalls),
                metadata: None,
            }),
            tool_calls: vec![ToolExecutionRecord {
                request: ToolRequestRecord {
                    request_id: "req-002".to_string(),
                    parent_id: "parent-002".to_string(),
                    call_id: "call-002".to_string(),
                    tool: "apply_code_edit".to_string(),
                    arguments: r#"{"path": "src/lib.rs", "line": 42}"#.to_string(),
                },
                result: ToolResult::Completed(ToolCompletedRecord {
                    request_id: "req-002".to_string(),
                    parent_id: "parent-002".to_string(),
                    call_id: "call-002".to_string(),
                    tool: "apply_code_edit".to_string(),
                    content: "Edit applied successfully".to_string(),
                    ui_payload: None,
                }),
                latency_ms: 200,
            }],
            outcome: TurnOutcome::ToolCalls { count: 1 },
            agent_turn_artifact: None,
        });

        record
    }

    #[test]
    fn timestamp_for_turn_returns_correct_timestamp() {
        let record = create_test_record_with_turns();

        assert_eq!(record.timestamp_for_turn(1), Some(1744223415500000));
        assert_eq!(record.timestamp_for_turn(2), Some(1744223415600000));
        assert_eq!(record.timestamp_for_turn(3), Some(1744223415700000));
        assert_eq!(record.timestamp_for_turn(4), None); // Non-existent turn
    }

    #[test]
    fn turn_record_returns_correct_turn() {
        let record = create_test_record_with_turns();

        let turn1 = record.turn_record(1).expect("Should find turn 1");
        assert_eq!(turn1.turn_number, 1);
        assert!(
            turn1
                .llm_response
                .as_ref()
                .unwrap()
                .content
                .contains("analyze")
        );

        let turn3 = record.turn_record(3).expect("Should find turn 3");
        assert_eq!(turn3.turn_number, 3);
        assert_eq!(turn3.tool_calls.len(), 1);

        assert!(record.turn_record(99).is_none());
    }

    #[test]
    fn tool_calls_in_turn_returns_correct_calls() {
        let record = create_test_record_with_turns();

        // Turn 1: No tool calls
        let calls1 = record.tool_calls_in_turn(1);
        assert!(calls1.is_empty());

        // Turn 2: One search tool call
        let calls2 = record.tool_calls_in_turn(2);
        assert_eq!(calls2.len(), 1);
        assert_eq!(calls2[0].request.tool, "search_code");

        // Turn 3: One edit tool call
        let calls3 = record.tool_calls_in_turn(3);
        assert_eq!(calls3.len(), 1);
        assert_eq!(calls3[0].request.tool, "apply_code_edit");

        // Non-existent turn: Empty
        let calls99 = record.tool_calls_in_turn(99);
        assert!(calls99.is_empty());
    }

    #[test]
    fn llm_response_at_turn_returns_correct_response() {
        let record = create_test_record_with_turns();

        let response1 = record
            .llm_response_at_turn(1)
            .expect("Should have response");
        assert!(response1.content.contains("analyze"));

        let response2 = record
            .llm_response_at_turn(2)
            .expect("Should have response");
        assert!(response2.content.contains("search"));

        assert!(record.llm_response_at_turn(99).is_none());
    }

    #[test]
    fn total_token_usage_sums_across_turns() {
        let record = create_test_record_with_turns();

        let total = record.total_token_usage();
        assert_eq!(total.prompt_tokens, 100 + 120 + 150); // 370
        assert_eq!(total.completion_tokens, 20 + 30 + 40); // 90
        assert_eq!(total.total_tokens, 120 + 150 + 190); // 460
    }

    #[test]
    fn turn_count_returns_correct_count() {
        let record = create_test_record_with_turns();
        assert_eq!(record.turn_count(), 3);

        let empty_record = create_test_record();
        assert_eq!(empty_record.turn_count(), 0);
    }

    #[test]
    fn was_tool_used_detects_tool_usage() {
        let record = create_test_record_with_turns();

        assert!(record.was_tool_used("search_code"));
        assert!(record.was_tool_used("apply_code_edit"));
        assert!(!record.was_tool_used("nonexistent_tool"));
    }

    #[test]
    fn turns_with_tool_returns_correct_turns() {
        let record = create_test_record_with_turns();

        let search_turns = record.turns_with_tool("search_code");
        assert_eq!(search_turns, vec![2]);

        let edit_turns = record.turns_with_tool("apply_code_edit");
        assert_eq!(edit_turns, vec![3]);

        let empty_turns = record.turns_with_tool("nonexistent");
        assert!(empty_turns.is_empty());
    }

    #[test]
    fn replay_state_at_turn_reconstructs_correctly() {
        let record = create_test_record_with_turns();

        let state = record.replay_state_at_turn(2).expect("Should get state");
        assert_eq!(state.turn, 2);
        assert_eq!(state.db_timestamp_micros, 1744223415600000);
        assert!(state.issue_prompt.contains("src/lib.rs"));
        assert_eq!(state.tool_calls.len(), 1);
        assert_eq!(state.tool_calls[0].request.tool, "search_code");
        assert_eq!(state.repo_root, std::path::PathBuf::from("/test/repo"));
        assert_eq!(state.base_sha, Some("abc123".to_string()));

        // LLM response should be present
        let response = state.llm_response.expect("Should have response");
        assert!(response.content.contains("search"));

        // Non-existent turn returns None
        assert!(record.replay_state_at_turn(99).is_none());
    }

    #[test]
    fn add_turn_from_artifact_persists_real_turn_fields() {
        let mut record = create_test_record();
        let artifact = create_test_turn_artifact();

        record.add_turn_from_artifact(artifact, 1744223415600000);

        let turn = record.turn_record(1).expect("Should persist turn");
        let llm_request = turn
            .llm_request
            .as_ref()
            .expect("Should persist llm request");
        assert_eq!(llm_request.messages.len(), 2);
        assert_eq!(llm_request.messages[1].content, "Fix the bug in src/lib.rs");
        assert_eq!(
            llm_request.model,
            ModelId::from_str("anthropic/claude-sonnet-4").unwrap()
        );

        let llm_response = turn
            .llm_response
            .as_ref()
            .expect("Should persist structured llm response");
        assert_eq!(
            llm_response.content,
            "I found the issue and will inspect the code."
        );
        assert_eq!(turn.tool_calls.len(), 1);
        assert_eq!(turn.tool_calls[0].request.tool, "search_code");
        assert!(matches!(turn.outcome, TurnOutcome::ToolCalls { count: 1 }));
    }

    #[test]
    fn replay_state_at_turn_uses_persisted_turn_messages() {
        let mut record = create_test_record();
        record.add_turn_from_artifact(create_test_turn_artifact(), 1744223415600000);

        let state = record
            .replay_state_at_turn(1)
            .expect("Should reconstruct state");

        assert_eq!(state.conversation_up_to_turn.len(), 3);
        assert_eq!(
            state.conversation_up_to_turn[0].content,
            "You are a coding assistant."
        );
        assert_eq!(
            state.conversation_up_to_turn[1].content,
            "Fix the bug in src/lib.rs"
        );
        assert_eq!(
            state.conversation_up_to_turn[2].content,
            "I found the issue and will inspect the code."
        );
    }

    #[test]
    fn outcome_summary_returns_correct_stats() {
        let record = create_test_record_with_turns();

        let summary = record.outcome_summary();
        assert_eq!(summary.turn_count, 3);
        assert_eq!(summary.total_tool_calls, 2); // 1 in turn 2, 1 in turn 3
        assert_eq!(summary.total_token_usage.total_tokens, 460);
        assert_eq!(summary.status, "incomplete"); // No validation phase
    }

    // ========================================================================
    // DbState and Introspection API Tests (Task 5)
    // ========================================================================

    #[test]
    fn db_state_creates_with_correct_timestamp() {
        let record = create_test_record_with_turns();
        let turn = record.turn_record(1).expect("Should find turn 1");

        let db_state = turn.db_state();
        assert_eq!(db_state.timestamp_micros(), 1744223415500000);
    }

    #[test]
    fn db_state_new_creates_correctly() {
        let db_state = DbState::new(1234567890);
        assert_eq!(db_state.timestamp_micros(), 1234567890);
    }

    #[test]
    fn node_info_serializes_correctly() {
        let node_info = NodeInfo {
            id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            name: "my_function".to_string(),
            node_type: "Function".to_string(),
        };

        let json = serde_json::to_string(&node_info).expect("Should serialize");
        assert!(json.contains("550e8400-e29b-41d4-a716-446655440000"));
        assert!(json.contains("my_function"));
        assert!(json.contains("Function"));

        let deserialized: NodeInfo = serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(deserialized.id, node_info.id);
        assert_eq!(deserialized.name, node_info.name);
        assert_eq!(deserialized.node_type, node_info.node_type);
    }

    #[test]
    fn replay_error_turn_not_found() {
        let err = ReplayError::TurnNotFound(42);
        assert_eq!(err.to_string(), "Turn 42 not found in run record");
    }

    #[test]
    fn replay_error_timestamp_not_found() {
        let err = ReplayError::TimestampNotFound(42);
        assert_eq!(err.to_string(), "Timestamp not found for turn 42");
    }

    #[test]
    fn replay_error_db_error_conversion() {
        let db_err = ploke_db::DbError::NotFound;
        let replay_err: ReplayError = db_err.into();

        match replay_err {
            ReplayError::DbError(_) => (), // Expected
            _ => panic!("Expected DbError variant"),
        }

        assert!(replay_err.to_string().contains("not found"));
    }

    #[test]
    fn replay_query_returns_error_for_missing_turn() {
        let mut record = create_test_record();

        // Clear the time travel index so there's no timestamp for turn 1
        record.db_time_travel_index.clear();

        // Verify timestamp_for_turn returns None for missing turn
        assert!(record.timestamp_for_turn(1).is_none());

        // Verify that replay_query would return TimestampNotFound error
        // (We can't test the full replay_query without a real DB, but we test the error path)
    }

    #[test]
    fn replay_query_returns_error_for_missing_timestamp() {
        let mut record = create_test_record();

        // Clear any existing time travel markers
        record.db_time_travel_index.clear();

        // Add a turn but no time travel marker
        record.phases.agent_turns.push(TurnRecord {
            turn_number: 1,
            started_at: "2026-04-09T18:30:15Z".to_string(),
            ended_at: "2026-04-09T18:30:20Z".to_string(),
            db_timestamp_micros: 1744223415500000,
            issue_prompt: "Test".to_string(),
            llm_request: None,
            llm_response: None,
            tool_calls: Vec::new(),
            outcome: TurnOutcome::Content,
            agent_turn_artifact: None,
        });

        // No time travel markers, so timestamp_for_turn should return None
        assert!(record.timestamp_for_turn(1).is_none());
    }

    // ========================================================================
    // SetupPhase Type Enhancement Tests (Task 2)
    // ========================================================================

    #[test]
    fn setup_phase_with_indexed_crates_serializes_to_json() {
        use crate::runner::{IndexingStatusArtifact, RepoStateArtifact};

        let setup_phase = SetupPhase {
            started_at: "2026-04-11T10:00:00Z".to_string(),
            ended_at: "2026-04-11T10:05:00Z".to_string(),
            repo_state: RepoStateArtifact {
                repo_root: std::path::PathBuf::from("/test/repo"),
                requested_base_sha: Some("abc123".to_string()),
                checked_out_head_sha: Some("def456".to_string()),
                git_status_porcelain: " M src/lib.rs".to_string(),
            },
            indexing_status: IndexingStatusArtifact {
                status: "completed".to_string(),
                detail: "Successfully indexed 2 crates".to_string(),
            },
            indexed_crates: vec![
                IndexedCrateSummary {
                    name: "main-crate".to_string(),
                    version: "1.0.0".to_string(),
                    namespace: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
                    root_path: std::path::PathBuf::from("/test/repo"),
                    file_count: 10,
                    node_count: 500,
                    embedded_count: 450,
                    status: CrateIndexStatus::Success,
                    parse_error: None,
                },
                IndexedCrateSummary {
                    name: "test-utils".to_string(),
                    version: "0.1.0".to_string(),
                    namespace: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap(),
                    root_path: std::path::PathBuf::from("/test/repo/test-utils"),
                    file_count: 3,
                    node_count: 100,
                    embedded_count: 95,
                    status: CrateIndexStatus::Partial,
                    parse_error: Some(ParseErrorSummary {
                        message: "Failed to parse one file".to_string(),
                        target_dir: std::path::PathBuf::from("/test/repo/test-utils/src/lib.rs"),
                        occurred_at_ms: 1712833200000,
                    }),
                },
            ],
            parse_failures: vec![ParseFailureRecord {
                target_dir: std::path::PathBuf::from("/test/repo/old-crate"),
                message: "Syntax error in main.rs".to_string(),
                occurred_at_ms: 1712833200000,
            }],
            db_timestamp_micros: 1712833200000000,
            tool_schema_version: Some("v1.2.3".to_string()),
        };

        // Serialize to JSON
        let json = serde_json::to_string_pretty(&setup_phase)
            .expect("Should serialize SetupPhase to JSON");

        // Verify JSON contains expected fields
        assert!(
            json.contains("indexed_crates"),
            "JSON should contain indexed_crates field"
        );
        assert!(
            json.contains("main-crate"),
            "JSON should contain crate name"
        );
        assert!(
            json.contains("550e8400-e29b-41d4-a716-446655440000"),
            "JSON should contain namespace UUID"
        );
        assert!(
            json.contains("Success"),
            "JSON should contain Success status"
        );
        assert!(
            json.contains("Partial"),
            "JSON should contain Partial status"
        );
        assert!(
            json.contains("parse_failures"),
            "JSON should contain parse_failures field"
        );
        assert!(
            json.contains("Syntax error in main.rs"),
            "JSON should contain parse failure message"
        );
        assert!(
            json.contains("tool_schema_version"),
            "JSON should contain tool_schema_version field"
        );
        assert!(
            json.contains("v1.2.3"),
            "JSON should contain tool schema version"
        );

        // Deserialize and verify
        let deserialized: SetupPhase =
            serde_json::from_str(&json).expect("Should deserialize JSON back to SetupPhase");
        assert_eq!(deserialized.indexed_crates.len(), 2);
        assert_eq!(deserialized.indexed_crates[0].name, "main-crate");
        assert_eq!(
            deserialized.indexed_crates[1].status,
            CrateIndexStatus::Partial
        );
        assert_eq!(deserialized.parse_failures.len(), 1);
        assert_eq!(deserialized.tool_schema_version, Some("v1.2.3".to_string()));
    }

    #[test]
    fn setup_phase_with_empty_indexed_crates_serializes_correctly() {
        use crate::runner::{IndexingStatusArtifact, RepoStateArtifact};

        let setup_phase = SetupPhase {
            started_at: "2026-04-11T10:00:00Z".to_string(),
            ended_at: "2026-04-11T10:05:00Z".to_string(),
            repo_state: RepoStateArtifact {
                repo_root: std::path::PathBuf::from("/test/repo"),
                requested_base_sha: Some("abc123".to_string()),
                checked_out_head_sha: Some("def456".to_string()),
                git_status_porcelain: "".to_string(),
            },
            indexing_status: IndexingStatusArtifact {
                status: "completed".to_string(),
                detail: "No crates to index".to_string(),
            },
            indexed_crates: vec![], // Empty
            parse_failures: vec![], // Empty
            db_timestamp_micros: 1712833200000000,
            tool_schema_version: None,
        };

        // Serialize to JSON
        let json =
            serde_json::to_string(&setup_phase).expect("Should serialize SetupPhase to JSON");

        // Verify JSON does not contain empty collections (they're skipped)
        assert!(
            !json.contains("indexed_crates"),
            "Empty indexed_crates should be skipped"
        );
        assert!(
            !json.contains("parse_failures"),
            "Empty parse_failures should be skipped"
        );
        assert!(
            !json.contains("tool_schema_version"),
            "None tool_schema_version should be skipped"
        );
    }
}
