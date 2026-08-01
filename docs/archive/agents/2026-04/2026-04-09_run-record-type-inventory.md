# 2026-04-09 Run Record Type Inventory

**Date:** 2026-04-09  
**Task:** Catalog all serializable types from ploke-llm and ploke-eval for run record implementation  
**Related:** [2026-04-09_run-manifest-design-note.md](./2026-04-09_run-manifest-design-note.md), [eval-design.md](../plans/evals/eval-design.md)

---

## Overview

This document inventories all serializable types relevant to the run record structure. Use this as a reference when implementing the canonical run record shape in `ploke-eval`.

---

## Part 1: ploke-llm Types (LLM API Layer)

### Model Identification Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `ModelKey` | `crates/ploke-llm/src/types/model_types.rs` | 14-91 | `{author}/{slug}` format | ✅ Custom Serialize/Deserialize |
| `ModelId` | `crates/ploke-llm/src/types/model_types.rs` | 93-270 | `{author}/{slug}:{variant}` format | ✅ Custom Serialize/Deserialize |
| `ModelVariant` | `crates/ploke-llm/src/types/model_types.rs` | 145-170 | Enum: Free, Beta, Extended, Thinking, Online, Nitro, Floor, Other | ✅ Derive |
| `Architecture` | `crates/ploke-llm/src/types/model_types.rs` | 272-282 | Model architecture metadata | ✅ Derive |

### LLM Request Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `ChatCompReqCore` | `crates/ploke-llm/src/request/completion.rs` | 14-50 | Core completion request | ✅ Derive |
| `RequestMessage` | `crates/ploke-llm/src/manager/mod.rs` | 42-50 | Message in conversation | ✅ Derive |
| `Role` | `crates/ploke-llm/src/manager/mod.rs` | 75-82 | Enum: User, Assistant, System, Tool | ✅ Derive |
| `ModelPricing` | `crates/ploke-llm/src/request/mod.rs` | 29-99 | Pricing information per token type | ✅ Derive |
| `JsonObjMarker` | `crates/ploke-llm/src/request/marker.rs` | (via re-export) | JSON object marker for response_format | ✅ |

### LLM Response Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `OpenAiResponse` | `crates/ploke-llm/src/response/mod.rs` | 10-28 | Full API response | ✅ Derive |
| `TokenUsage` | `crates/ploke-llm/src/response/mod.rs` | 38-44 | prompt/completion/total tokens | ✅ Derive |
| `FinishReason` | `crates/ploke-llm/src/response/mod.rs` | 88-98 | Enum: Stop, Length, ContentFilter, ToolCalls, Timeout, Error | ✅ Derive |
| `Choices` | `crates/ploke-llm/src/response/mod.rs` | 66-86 | Choice with message/delta/error | ✅ Derive |
| `ResponseMessage` | `crates/ploke-llm/src/response/mod.rs` | 134-149 | Assistant response message | ✅ Derive |
| `StreamingDelta` | `crates/ploke-llm/src/response/mod.rs` | 100-110 | Streaming response delta | ✅ Derive |
| `ErrorResponse` | `crates/ploke-llm/src/response/mod.rs` | 112-120 | Error details | ✅ Derive |
| `ToolCall` | `crates/ploke-llm/src/response/tool_call.rs` | (re-exported) | Tool call from model | ✅ Derive |
| `FunctionCall` | `crates/ploke-llm/src/response/tool_call.rs` | (re-exported) | Function call details | ✅ Derive |

### LLM Metadata Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `LLMMetadata` | `crates/ploke-llm/src/types/meta.rs` | 7-27 | Aggregated LLM execution metadata | ✅ Derive |
| `PerformanceMetrics` | `crates/ploke-llm/src/types/meta.rs` | 30-35 | tokens_per_second, time_to_first_token, queue_time | ✅ Derive |

### Chat Session Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `ChatStepOutcome` | `crates/ploke-llm/src/manager/session.rs` | 22-34 | Content vs ToolCalls outcome | ✅ (outcome enum) |
| `ChatStepData` | `crates/ploke-llm/src/manager/session.rs` | 203-207 | Full step result with response | ✅ |
| `ChatHttpConfig` | `crates/ploke-llm/src/manager/session.rs` | 36-54 | HTTP timeout, referer, title | ✅ |

---

## Part 2: ploke-eval Types (Eval Harness Layer)

### Run Specification Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `PrepareSingleRunRequest` | `crates/ploke-eval/src/spec.rs` | 38-46 | Input for preparing a run | ✅ Derive |
| `PreparedSingleRun` | `crates/ploke-eval/src/spec.rs` | 67-77 | Prepared run manifest (currently `run.json`) | ✅ Derive |
| `IssueInput` | `crates/ploke-eval/src/spec.rs` | 31-36 | Issue title/body/body_path | ✅ Derive |
| `EvalBudget` | `crates/ploke-eval/src/spec.rs` | 14-29 | max_turns, max_tool_calls, wall_clock_secs | ✅ Derive |
| `RunSource` | `crates/ploke-eval/src/spec.rs` | 49-52 | Enum: MultiSweBench source | ✅ Derive (tagged) |
| `MultiSweBenchSource` | `crates/ploke-eval/src/spec.rs` | 54-66 | MSB-specific metadata | ✅ Derive |
| `PreparedMsbBatch` | `crates/ploke-eval/src/spec.rs` | 79-89 | Batch preparation manifest | ✅ Derive |

### Artifact Path Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `RunArtifactPaths` | `crates/ploke-eval/src/runner.rs` | 112-123 | Paths to all artifacts | ✅ Derive |
| `AgentRunArtifactPaths` | `crates/ploke-eval/src/runner.rs` | 124-129 | Agent-specific paths | ✅ Derive |
| `BatchRunArtifactPaths` | `crates/ploke-eval/src/runner.rs` | 131-136 | Batch run paths | ✅ Derive |

### Artifact Content Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `RepoStateArtifact` | `crates/ploke-eval/src/runner.rs` | 138-144 | Git repo state at run start | ✅ Derive |
| `IndexingStatusArtifact` | `crates/ploke-eval/src/runner.rs` | 146-151 | Indexing success/failure status | ✅ Derive |
| `ParseFailureArtifact` | `crates/ploke-eval/src/runner.rs` | 152-158 | Parser error details | ✅ Derive |
| `SnapshotStatusArtifact` | `crates/ploke-eval/src/runner.rs` | 221-227 | DB snapshot status | ✅ Derive |
| `StartingDbCacheMetadata` | `crates/ploke-eval/src/runner.rs` | 229-239 | DB cache versioning info | ✅ Derive |
| `ExecutionLog` | `crates/ploke-eval/src/runner.rs` | 246-254 | High-level execution steps | ✅ Derive |

### Agent Turn Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `AgentTurnArtifact` | `crates/ploke-eval/src/runner.rs` | 386-397 | Per-turn complete record | ✅ Derive |
| `ObservedTurnEvent` | `crates/ploke-eval/src/runner.rs` | 345-354 | Enum of turn events | ✅ Derive |
| `ToolRequestRecord` | `crates/ploke-eval/src/runner.rs` | 294-301 | Tool call request | ✅ Derive |
| `ToolCompletedRecord` | `crates/ploke-eval/src/runner.rs` | 303-311 | Tool call success | ✅ Derive |
| `ToolFailedRecord` | `crates/ploke-eval/src/runner.rs` | 313-321 | Tool call failure | ✅ Derive |
| `MessageSnapshotRecord` | `crates/ploke-eval/src/runner.rs` | 323-331 | Message state snapshot | ✅ Derive |
| `TurnFinishedRecord` | `crates/ploke-eval/src/runner.rs` | 333-343 | Turn completion record | ✅ Derive |

### Patch/Edit Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `PatchArtifact` | `crates/ploke-eval/src/runner.rs` | 365-374 | Complete patch state | ✅ Derive |
| `ProposalSnapshotRecord` | `crates/ploke-eval/src/runner.rs` | 356-363 | Edit proposal snapshot | ✅ Derive |
| `ExpectedFileChangeRecord` | `crates/ploke-eval/src/runner.rs` | 376-384 | File change tracking | ✅ Derive |

### Batch Types

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `BatchInstanceResult` | `crates/ploke-eval/src/runner.rs` | 256-265 | Single instance result in batch | ✅ Derive |
| `BatchRunSummary` | `crates/ploke-eval/src/runner.rs` | 267-284 | Batch execution summary | ✅ Derive |
| `MultiSweBenchSubmissionRecord` | `crates/ploke-eval/src/runner.rs` | 399-406 | MSB submission format | ✅ Derive |
| `ReplayBatchArtifact` | `crates/ploke-eval/src/runner.rs` | 286-292 | Replayable batch data | ✅ Derive |

### Request Types (Runner Input)

| Type | File | Lines | Description | Serialize |
|------|------|-------|-------------|-----------|
| `RunMsbAgentSingleRequest` | `crates/ploke-eval/src/runner.rs` | 61-69 | Agent single run request | ✅ Derive |
| `RunMsbSingleRequest` | `crates/ploke-eval/src/runner.rs` | 71-79 | Single run request | ✅ Derive |
| `RunMsbBatchRequest` | `crates/ploke-eval/src/runner.rs` | 81-91 | Batch run request | ✅ Derive |
| `RunMsbAgentBatchRequest` | `crates/ploke-eval/src/runner.rs` | 93-103 | Agent batch request | ✅ Derive |
| `ReplayMsbBatchRequest` | `crates/ploke-eval/src/runner.rs` | 105-110 | Replay request | ✅ Derive |

---

## Part 3: External Types Referenced

### ploke-tui Types (via re-exports)

| Type | Source Crate | Usage in ploke-eval | Notes |
|------|--------------|---------------------|-------|
| `ToolUiPayload` | `ploke-tui::tools` | In `ToolCompletedRecord`, `ToolFailedRecord` | UI payload from tool execution |
| `FlattenedParserDiagnostic` | `ploke-tui::utils::parse_errors` | In `ParseFailureArtifact` | Parser error details |
| `EditProposalStatus` | `ploke-tui::app_state` | Referenced in `runner.rs` | Proposal state enum |
| `DiffPreview` | `ploke-tui::app_state` | Referenced in `ProposalSnapshotRecord` | Diff display format |

### ploke-db Types (via re-exports)

| Type | Source Crate | Usage in ploke-eval | Notes |
|------|--------------|---------------------|-------|
| `TypedEmbedData` | `ploke_db` | In `ReplayBatchArtifact` | Embedding data for replay |

---

## Part 4: Proposed Run Record Structure

Based on the type inventory, the canonical run record should contain:

### Section A: Metadata (from manifest)
- `manifest_id`: String
- `experiment`: ExperimentConfig (from hypothesis registry)
- `benchmark`: BenchmarkConfig
- `agent`: AgentConfig (model_id, provider, tool versions)
- `runtime`: RuntimeConfig (temperature, max_turns, etc.)

### Section B: Phases

#### Phase 1: Setup
- `started_at`, `ended_at`: ISO 8601 timestamps
- `steps`: Vec<String> (execution steps)
- `repo_state`: `RepoStateArtifact`
- `indexing_status`: `IndexingStatusArtifact`
- `parse_failure`: Option<`ParseFailureArtifact`>

#### Phase 2: Agent Turns
- `turns`: Vec<TurnRecord>

**TurnRecord structure:**
```rust
struct TurnRecord {
    turn_number: u32,
    started_at: String,  // ISO 8601
    ended_at: String,    // ISO 8601
    db_timestamp_micros: i64,  // For Cozo @ query
    
    // LLM interaction
    llm_request: ChatCompReqCore,
    llm_response: OpenAiResponse,
    llm_metadata: LLMMetadata,  // Derived from response
    
    // Tool execution
    tool_calls: Vec<ToolExecutionRecord>,
    
    // Outcome
    outcome: TurnOutcome,  // Enum: ToolCalls, Content, Error, Timeout
}

struct ToolExecutionRecord {
    request: ToolRequestRecord,
    result: ToolResult,  // Completed or Failed
    latency_ms: u64,
}
```

#### Phase 3: Patch
- `proposals`: Vec<`ProposalSnapshotRecord`>
- `applied`: bool
- `patch_content`: String (the actual git diff)

#### Phase 4: Validation
- `build_result`: BuildResult enum
- `test_result`: TestResult enum
- `benchmark_verdict`: String

### Section C: Time Travel Index
```rust
db_time_travel_index: Vec<TimeTravelMarker>

struct TimeTravelMarker {
    turn: u32,
    timestamp_micros: i64,  // Cozo validity timestamp
    event: String,  // "turn_start", "turn_complete", "tool_call", etc.
}
```

### Section D: Conversation History
```rust
conversation: Vec<RequestMessage>  // Complete message log
```

---

## Part 5: Implementation Checklist (TODO)

- [ ] Define `RunRecord` struct in `ploke-eval/src/record.rs` (new file)
- [ ] Define `TurnRecord`, `ToolExecutionRecord`, `TimeTravelMarker` structs
- [ ] Add serialization tests for run record types
- [ ] Update `runner.rs` to emit `record.json.gz` alongside existing artifacts
- [ ] Capture `db_timestamp_micros` at each turn boundary (Cozo NOW)
- [ ] Build `db_time_travel_index` during run execution
- [ ] Implement compression for record file
- [ ] Add `record_path` to `RunArtifactPaths`
- [ ] Create introspection API trait for querying run records
- [ ] Update manifest (`run.json`) to reference record path
- [ ] Document the `@ timestamp` query pattern for Cozo

---

## References

### Primary Source Files
1. `crates/ploke-llm/src/types/model_types.rs` - Model identification
2. `crates/ploke-llm/src/request/completion.rs` - Request types
3. `crates/ploke-llm/src/request/mod.rs` - Request module
4. `crates/ploke-llm/src/response/mod.rs` - Response types
5. `crates/ploke-llm/src/response/tool_call.rs` - Tool call types
6. `crates/ploke-llm/src/manager/mod.rs` - Message and Role types
7. `crates/ploke-llm/src/manager/session.rs` - Chat step types
8. `crates/ploke-llm/src/types/meta.rs` - Metadata types
9. `crates/ploke-eval/src/spec.rs` - Run specification
10. `crates/ploke-eval/src/runner.rs` - Artifact types

### Related Documentation
- [eval-design.md](../plans/evals/eval-design.md) §VII - Introspection API
- [2026-04-09_run-manifest-design-note.md](./2026-04-09_run-manifest-design-note.md) - Cozo time travel clarification
- [Cozo time-travel docs](../../dependency_details/cozo/types/time-travel.md) - `@ timestamp` syntax
