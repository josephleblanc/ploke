# Phase 1: RunRecord Implementation Tracking

**Layer:** Layer 0 (Observability & Data Capture) → Enables Layer 1 (A4) and Measurement (A5)  
**Status:** Active - Phase 1  
**Linked Hypotheses:** 
- **A4** (Layer 1): Comprehensive result schema — RunRecord *is* the schema
- **A5** (measurement): Replay/introspection — RunRecord *enables* replay capability (hard gate for H0)
**Branch:** `refactor/tool-calls`  
**Created:** 2026-04-09  
**Last Updated:** 2026-04-09  

---

## Overview

This document tracks the implementation of the comprehensive `RunRecord` for the ploke-eval harness. We are building **Layer 0: Observability & Data Capture** infrastructure per the [eval-design dependency ordering](../plans/evals/eval-design.md#v-dependency-ordered-implementation-layers).

This work enables:
- **A4 (Layer 1)**: "Comprehensive result schema" — RunRecord provides the unified schema
- **A5 (measurement)**: "Replay and introspection" — RunRecord enables querying without re-running (hard gate for H0)

**Reference docs:**
- [RunRecord type inventory](../../agents/2026-04-09_run-record-type-inventory.md)
- [RunRecord design handoff](../../workflow/handoffs/2026-04-09_run-record-design-handoff.md)
- [Phase execution plan](./phased-exec-plan.md)

---

## Deliverables

| Deliverable | Status | Enables | Notes |
|-------------|--------|---------|-------|
| Define `RunRecord` schema | ✅ Complete | **A4** (comprehensive schema) | Types in `ploke-eval/src/record.rs` |
| Capture conversation history | ⬜ Not Started | **A5** (replay) | Read from `state.chat` |
| Capture Cozo timestamps | ✅ Complete | **A5** (time-travel queries) | `current_validity_micros()` in `ploke-db` with unit test |
| Structured LLM event capture | ⬜ Not Started | **A4/A5** (full telemetry) | Use existing `ChatEvt::Response` |
| Emit `record.json.gz` | ⬜ Not Started | **A4** (artifact completeness) | Compress with flate2 |
| Introspection API | ⬜ Not Started | **A5** (query without re-run) | `query_at_turn()`, `conversation_up_to_turn()` |

---

## Key Design Decision: Minimize Production Code Changes

**Finding:** We can capture ~80% of needed LLM data **without modifying ploke-tui/ploke-llm**.

### What We Can Capture (No Production Changes)

The existing `ChatEvt::Response` event already contains:
```rust
ChatEvt::Response {
    request_id: Uuid,
    parent_id: Uuid,
    content: String,        // Assistant's response
    model: String,          // Model ID
    metadata: LLMMetadata,  // Usage, finish_reason, etc.
    usage: UsageMetrics,    // Token counts
}
```

**Current gap:** The runner captures this as `format!("{event:?}")` - a debug string.

**Fix in ploke-eval only:** Add structured handling in `handle_benchmark_event()`:
```rust
AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Response { ... })) => {
    // Create structured LlmResponseRecord instead of debug string
}
```

### What Requires Minimal Production Changes (If Needed)

For full raw `OpenAiResponse` and `ChatCompReqCore`:
- Add `ChatEvt::StepCompleted` variant in `ploke-tui/src/llm/manager/events.rs`
- Emit in `run_chat_session()` (~10 lines total)
- **Scope:** Additive only, no behavior changes

**Decision:** Implement Phase 1 using existing `ChatEvt::Response`. Add `StepCompleted` only if introspection requires raw response data.

---

## Implementation Phases

### Phase 1A: Foundation Types (Est. 4 hrs)

**Tasks:**
- [ ] Create `crates/ploke-eval/src/record.rs` with:
  - `RunRecord` - top-level container
  - `TurnRecord` - per-turn data with timestamps
  - `TimeTravelMarker` - for Cozo `@` queries
  - `LlmResponseRecord` - structured LLM response capture
  - `ToolExecutionRecord` - request + result + latency

- [ ] Add `record_path: Option<PathBuf>` to `RunArtifactPaths`

**Acceptance criteria:**
- Types compile with Serialize/Deserialize derives
- Schema version field present for future migrations

---

### Phase 1B: Cozo Time Travel (Est. 2 hrs)

**Tasks:**
- [ ] Add `current_validity_micros()` to `ploke-db/src/observability.rs`:
```rust
impl Database {
    pub fn current_validity_micros(&self) -> Result<i64, DbError> {
        // Query: "?[now] := now = to_int('NOW') :limit 1"
    }
}
```

- [ ] Capture timestamps at turn boundaries:
  - Pre-turn: `turn_start` marker
  - Post-turn: `turn_complete` marker
  - Build `db_time_travel_index: Vec<TimeTravelMarker>`

**Acceptance criteria:**
- Can query DB state at any turn using `@ timestamp_micros`

---

### Phase 1C: Conversation History (Est. 2 hrs)

**Tasks:**
- [ ] At turn end, read `state.chat.0.read().messages`
- [ ] Convert to `Vec<RequestMessage>`
- [ ] Store in `TurnRecord.conversation`

**Reference:** `snapshot_message()` pattern in runner.rs lines 439-453

**Acceptance criteria:**
- Full conversation reconstructible at any turn

---

### Phase 1D: Structured LLM Capture (Est. 3 hrs)

**Tasks:**
- [ ] Modify `handle_benchmark_event()` to capture `ChatEvt::Response`:
```rust
// Instead of: artifact.events.push(ObservedTurnEvent::LlmEvent(rendered))
// Do: capture structured fields into LlmResponseRecord
```

- [ ] Add `LlmResponseRecord` to `ObservedTurnEvent` enum:
```rust
pub enum ObservedTurnEvent {
    // ... existing variants
    LlmResponse(LlmResponseRecord),
}
```

- [ ] Populate `TurnRecord.llm_response` from captured data

**Acceptance criteria:**
- Token usage, model, content, finish_reason captured structurally
- No more debug-string LLM events in RunRecord

---

### Phase 1E: Emission and Compression (Est. 2 hrs)

**Tasks:**
- [ ] Wire RunRecord collection in runner:
  - Pre-turn: Initialize with metadata
  - Post-turn: Finalize TurnRecord
  - End of run: Write compressed record

- [ ] Add compression with `flate2`:
```rust
// Write record.json.gz
let encoder = GzEncoder::new(file, Compression::default());
serde_json::to_writer(encoder, &run_record)?;
```

- [ ] Update `RunArtifactPaths` with `record_path`

**Acceptance criteria:**
- `record.json.gz` emitted alongside existing artifacts
- Compression ratio > 5x for typical runs

---

### Phase 1F: Introspection API (Est. 3 hrs)

**Tasks:**
- [ ] Implement on `RunRecord`:
```rust
impl RunRecord {
    /// Query DB state at specific turn using Cozo @ timestamp
    pub fn query_at_turn(&self, turn: usize, query: &str) -> Result<QueryResult, Error>;
    
    /// Get conversation up to specific turn
    pub fn conversation_up_to_turn(&self, turn: usize) -> Vec<RequestMessage>;
    
    /// Get tool calls in specific turn
    pub fn tool_calls_in_turn(&self, turn: usize) -> Vec<&ToolExecutionRecord>;
    
    /// Reconstruct state for replay
    pub fn replay_state_at_turn(&self, turn: usize) -> ReplayState;
}
```

**Acceptance criteria:**
- Can answer common run questions without re-running
- A5 marked as satisfied in hypothesis registry

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Event flow changes affect live TUI | Only modify ploke-eval; minimal ploke-tui changes if needed |
| Record file size | Use flate2 compression; conversation history compresses well |
| Cozo timestamp performance | Query `NOW` only at phase boundaries |
| Backward compatibility | Keep existing artifacts; RunRecord is additive |

---

## Interruption Recovery

**If interrupted:** Resume from this document. Check off completed tasks.

**Current state:** Planning complete, ready to begin Phase 1A.

**Next action:** Create `crates/ploke-eval/src/record.rs` with foundation types.

---

## Progress Log

| Date | Action | Status |
|------|--------|--------|
| 2026-04-09 | Plan created and workflow-validated | ✅ Complete |
| 2026-04-09 | Production-code modification review | ✅ Complete - minimal changes needed |
| 2026-04-09 | Moved to phase tracking location | ✅ Complete |
| 2026-04-09 | Phase 1B: Cozo timestamp helper | ✅ Complete |
| 2026-04-09 | Phase 1B tests: unit test `current_validity_micros_returns_monotonic_timestamp` | ✅ Complete |
| 2026-04-09 | Phase 1B tests: fixture test `current_validity_micros_works_with_fixture_database` | ✅ Complete |

---

## Linked Artifacts

- **Hypothesis:** A5 (docs/active/workflow/hypothesis-registry.md)
- **Type Inventory:** docs/active/agents/2026-04-09_run-record-type-inventory.md
- **Design Handoff:** docs/active/workflow/handoffs/2026-04-09_run-record-design-handoff.md
- **Original Plan:** docs/active/agents/2026-04-09_runrecord-implementation-plan.md (deprecated)
