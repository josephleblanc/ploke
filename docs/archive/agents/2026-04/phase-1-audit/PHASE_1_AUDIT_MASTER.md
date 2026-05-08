# Phase 1 Implementation Audit

**Audit started:** 2026-04-11  
**Auditor:** Orchestrator agent with parallel sub-agents  
**Scope:** Verify actual vs claimed completion of Phase 1 deliverables  
**Status:** IN PROGRESS

---

## Audit Charter

### Background
Phase 1 was marked COMPLETE in `phase-1-runrecord-tracking.md`, but critical gaps were discovered:

1. `turn.db_state().lookup()` was listed as a **minimum deliverable** in `eval-design.md` §VII but was NOT implemented
2. **SetupPhase data** (indexing results, parse failures) is NOT captured in RunRecord
3. Cannot programmatically verify which crates were indexed or check for parse errors

### Audit Objectives
1. **Verify all claimed Phase 1 deliverables** against actual code
2. **Identify gaps** between design docs and implementation
3. **Document what works** vs what doesn't
4. **Provide recommendations** for closing gaps

### Reference Documents (all sub-agents should consult)
- `docs/active/plans/evals/eval-design.md` §VII - The Introspection & Replay API specification
- `docs/active/plans/evals/phased-exec-plan.md` - Phase 1 deliverables and exit criteria
- `docs/active/agents/2026-04-09_run-record-type-inventory.md` - Expected RunRecord schema
- `phase-1-runrecord-tracking.md` - Claims of what's complete

### Methodology
Sub-agents investigate in parallel, appending findings to this document.

---

## Append-Only Audit Log

<!-- Each sub-agent appends their findings below this line -->
<!-- Format: ### [AREA] Findings from Agent [N] -->
<!-- Example: ### Introspection API Findings from Agent 1 -->


### RunRecord Schema Findings from Agent 2

**Investigated:** `crates/ploke-eval/src/record.rs`, `docs/active/agents/2026-04-09_run-record-type-inventory.md`

**Claims vs Reality:**

| Type/Field | Inventory Claims | Actual in Code | Status |
|------------|------------------|----------------|--------|
| **RunRecord** | schema_version, manifest_id, metadata, phases, db_time_travel_index, conversation | All fields present | ✅ OK |
| **RunPhases** | setup, agent_turns, patch, validation | All fields present | ✅ OK |
| **SetupPhase.indexed_crates** | Vec<IndexedCrate> or similar | **MISSING** - no field for indexed crate list | ❌ MISSING |
| **SetupPhase.parse_failures** | Vec<ParseFailure> or detailed artifact | Single `parse_failure: Option<ParseFailureRecord>` | ⚠️ PARTIAL |
| **SetupPhase.tool_schema_version** | Required field | **MISSING** - exists in AgentMetadata but not SetupPhase | ❌ MISSING |
| **SetupPhase.indexing_errors** | Vec<IndexingError> or similar | **MISSING** - only has `indexing_status: IndexingStatusArtifact` | ❌ MISSING |
| **SetupPhase.started_at/ended_at** | ISO 8601 timestamps | Present | ✅ OK |
| **SetupPhase.repo_state** | RepoStateArtifact | Present | ✅ OK |
| **SetupPhase.db_timestamp_micros** | Cozo timestamp | Present | ✅ OK |
| **TurnRecord.turn_number** | u32 | Present | ✅ OK |
| **TurnRecord.timestamp** | ISO 8601 (started_at/ended_at) | Present | ✅ OK |
| **TurnRecord.db_timestamp_micros** | Cozo timestamp | Present | ✅ OK |
| **TurnRecord.tool_calls** | Vec<ToolExecutionRecord> | Present | ✅ OK |
| **TurnRecord.llm_prompt** | issue_prompt field | Present (named `issue_prompt`) | ✅ OK |
| **TurnRecord.llm_response** | LlmResponseRecord | Present | ✅ OK |
| **TurnRecord.events** | AgentTurnArtifact captured | Present as `agent_turn_artifact` | ✅ OK |
| **TimeTravelMarker.turn_number** | u32 | Present as `turn` | ✅ OK |
| **TimeTravelMarker.timestamp** | pre_turn/post_turn | Single `timestamp_micros` with `event` label | ⚠️ PARTIAL |

**Key Gaps Identified:**

1. **SetupPhase is missing critical indexing metadata:**
   - No `indexed_crates` field to capture which crates were indexed
   - No `indexing_errors` field for capturing multiple indexing errors
   - `parse_failure` is singular (`Option<ParseFailureRecord>`) but should support multiple failures
   - Missing `tool_schema_version` (exists in AgentMetadata but not in SetupPhase where it belongs for version tracking)

2. **ParseFailureRecord lacks diagnostic detail:**
   - Code has: `target_dir`, `message`, `occurred_at_ms`
   - Missing: `diagnostics: Vec<FlattenedParserDiagnostic>` (exists in `ParseFailureArtifact` in runner.rs but not in record.rs)

3. **TimeTravelMarker doesn't match exact spec:**
   - Inventory specified: `pre_turn_timestamp`, `post_turn_timestamp` 
   - Code has: single `timestamp_micros` with `event` string label ("turn_start", "turn_complete")
   - This is functionally equivalent but schema differs from design

**Evidence - Actual SetupPhase struct:**

```rust
// From crates/ploke-eval/src/record.rs lines 486-506
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
```

**Evidence - Expected per Phase 1 Deliverables (from phased-exec-plan.md):**

> "Define and implement the run data schema (conversations, tool calls, DB snapshots, metrics, **failure classifications**)"

The current SetupPhase only captures:
- ✅ Conversations (via RunRecord.conversation)
- ✅ Tool calls (via TurnRecord.tool_calls)
- ✅ DB snapshots (via TimeTravelMarker for time-travel queries)
- ⚠️ Metrics (partial - token usage captured, but not indexing metrics)
- ❌ **Failure classifications** (missing - parse failures, indexing errors not fully captured)

**Recommendations:**

1. **Add `indexed_crates: Vec<IndexedCrateSummary>` to SetupPhase** - capture what was indexed
2. **Change `parse_failure: Option<ParseFailureRecord>` to `parse_failures: Vec<ParseFailureRecord>`** - support multiple failures
3. **Add `diagnostics: Vec<FlattenedParserDiagnostic>` to ParseFailureRecord** - match ParseFailureArtifact detail
4. **Add `indexing_errors: Vec<IndexingErrorRecord>` to SetupPhase** - capture non-parse indexing failures
5. **Consider moving `tool_schema_version` from AgentMetadata to SetupPhase** OR ensure it's populated for version tracking
6. **Document the TimeTravelMarker.event field** as the canonical way to distinguish pre/post turn timestamps

---


### Introspection API Findings from Agent 1

**Investigated:** `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/src/tests/replay.rs`

**Claims vs Reality:**

| Claimed Method (from eval-design.md §VII) | Actual Implementation | Status |
|----------------|----------------------|--------|
| `run.conversations()` → iterator over agent turns | NOT IMPLEMENTED - No such method exists on `RunRecord` | MISSING |
| `run.tool_calls()` → all tool calls with inputs, outputs, timing | NOT IMPLEMENTED - Only `tool_calls_in_turn(turn: u32)` exists for per-turn access | MISSING |
| `run.db_snapshots()` → DB state at each turn boundary | NOT IMPLEMENTED - Only `db_time_travel_index` field with raw `Vec<TimeTravelMarker>` | MISSING |
| `run.metrics()` → computed metrics for this run | PARTIAL - `total_token_usage()` and `outcome_summary()` exist but no unified `metrics()` method | PARTIAL |
| `run.failures()` → classified failure records | NOT IMPLEMENTED - No failure classification retrieval method | MISSING |
| `run.config()` → frozen configuration for this run | NOT IMPLEMENTED - `metadata` field exists but no `config()` accessor | MISSING |
| `run.manifest()` → immutable run manifest | PARTIAL - `manifest_id` field exists but no `manifest()` method returning full manifest | PARTIAL |
| `turn.messages()` → full message history at this point | NOT IMPLEMENTED - `conversation` field exists on `RunRecord` but no `messages()` method on turn | MISSING |
| `turn.tool_call()` → the tool call made (if any) | NOT IMPLEMENTED - Only `tool_calls_in_turn()` on RunRecord, not on TurnRecord | MISSING |
| `turn.tool_result()` → what the tool returned | NOT IMPLEMENTED - No such method on `TurnRecord` | MISSING |
| `turn.db_state()` → queryable DB snapshot | NOT IMPLEMENTED - `db_timestamp_micros` field exists but no `db_state()` method | MISSING |
| `turn.db_state().lookup(name)` → "does this node exist?" | NOT IMPLEMENTED - This was a minimum Phase 1 deliverable per §XIII | MISSING |
| `replay(run, from_turn=N)` → re-execute from turn N | NOT IMPLEMENTED - No replay function exists | MISSING |
| `replay_tool_call(turn)` → re-execute just the tool call | NOT IMPLEMENTED - No such function exists | MISSING |
| `replay_query(turn, query)` → run arbitrary query against DB snapshot | NOT IMPLEMENTED - No such function exists | MISSING |

**What Actually Exists in `impl RunRecord` (lines 166-373):**

```rust
// Public methods actually implemented:
pub fn new(manifest: &PreparedSingleRun) -> Self
pub fn mark_time_travel(&mut self, turn: u32, timestamp_micros: i64, event: impl Into<String>)
pub fn turn_timestamp(&self, turn: u32) -> Option<i64>
pub fn timestamp_for_turn(&self, turn: u32) -> Option<i64>  // alias for turn_timestamp
pub fn tool_calls_in_turn(&self, turn: u32) -> Vec<&ToolExecutionRecord>
pub fn turn_record(&self, turn: u32) -> Option<&TurnRecord>
pub fn llm_response_at_turn(&self, turn: u32) -> Option<&LlmResponseRecord>
pub fn total_token_usage(&self) -> TokenUsage
pub fn turn_count(&self) -> u32
pub fn outcome_summary(&self) -> RunOutcomeSummary
pub fn was_tool_used(&self, tool_name: &str) -> bool
pub fn turns_with_tool(&self, tool_name: &str) -> Vec<u32>
pub fn replay_state_at_turn(&self, turn: u32) -> Option<ReplayState>  // returns data, not actual replay
```

**Key Gaps Identified:**

1. **`turn.db_state().lookup(name)` is COMPLETELY MISSING** - This was explicitly listed as a minimum Phase 1 deliverable in `eval-design.md` §XIII: "Implement `turn.db_state().lookup(name)` — because you specifically mentioned wanting to answer 'does this node exist at the time the agent queried for it?'". There is no `DbState` type, no `db_state()` method on `TurnRecord`, and no `lookup()` method anywhere.

2. **No iterator methods** - The design calls for `run.conversations()` and `run.db_snapshots()` to return iterators, but only direct-index methods like `turn_record(turn: u32)` exist.

3. **No high-level aggregation methods** - `run.tool_calls()` (to get ALL calls across all turns), `run.failures()`, `run.config()`, and `run.manifest()` are not implemented.

4. **TurnRecord has NO methods** - The `TurnRecord` struct (lines 545-579) has only public fields, no methods at all. All turn-level inspection must go through `RunRecord` methods.

5. **Replay is data-only, not functional** - `replay_state_at_turn()` returns a `ReplayState` struct with data, but there's no actual replay capability. The tests in `replay.rs` are historical diagnostic replays of specific fixtures, not implementations of the replay API from §VII.

**Evidence:**

```rust
// From record.rs lines 200-214 - the closest thing to db_state():
/// Get the Cozo DB timestamp for querying historical state at a specific turn.
pub fn timestamp_for_turn(&self, turn: u32) -> Option<i64> {
    self.turn_timestamp(turn)
}
// But no db_state().lookup() - only returns a raw i64 timestamp!

// From record.rs lines 545-579 - TurnRecord has no impl block:
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub turn_number: u32,
    pub started_at: String,
    pub ended_at: String,
    pub db_timestamp_micros: i64,  // Raw timestamp, not a queryable state
    // ... fields only, no methods
}

// No impl TurnRecord block exists in the file!
```

**Recommendation:**

1. **Implement `DbState` type** with `lookup(name)` method that takes the timestamp from `TurnRecord::db_timestamp_micros` and queries the Cozo DB
2. **Add `db_state()` method to `TurnRecord`** that returns `DbState` wrapper
3. **Implement missing run-level methods**: `conversations()`, `tool_calls()`, `failures()`, `config()`, `manifest()`
4. **Implement missing replay functions**: `replay()`, `replay_tool_call()`, `replay_query()`
5. **Add methods to `TurnRecord`** instead of requiring all access through `RunRecord` methods


### SetupPhase Capture Findings from Agent 4

**Investigated:** `crates/ploke-eval/src/record.rs`, `crates/ploke-eval/src/runner.rs`, artifact files

**SetupPhase Current State (in record.rs lines 486-506):**
```rust
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
```

**What Indexing Produces:**
- `indexing-status.json`: Contains `{"status": "completed", "detail": "..."}` or failure status
- `parse-failure.json`: Contains detailed parse failure with:
  - `target_dir`: Path to the failing crate
  - `message`: Human-readable error message
  - `occurred_at_ms`: Timestamp
  - `diagnostics: Vec<FlattenedParserDiagnostic>`: Detailed structured diagnostics

**Evidence from Actual Run (`~/.ploke-eval/runs/BurntSushi__ripgrep-1294/`):**
- `indexing-status.json` exists with: `{"status":"completed","detail":"Indexing completed..."}`
- `parse-failure.json` exists with detailed diagnostics showing:
  - Parse failed for `globset` crate
  - 6 files succeeded, 1 failed
  - Specific error: Syn parsing error in `ignore/src/walk.rs` line 484

**What's Captured in RunRecord:**

| Data Source | In RunRecord? | Where |
|-------------|---------------|-------|
| `indexing_status` | **NO** | Written to `indexing-status.json` only |
| `parse_failure` | **NO** | Written to `parse-failure.json` only |
| `repo_state` | **NO** | Written to `repo-state.json` only |
| `db_timestamp_micros` | **NO** | Not captured |
| `started_at`/`ended_at` | **NO** | Not captured |
| `indexed_crates` | **NO** | Not captured anywhere in RunRecord |

**Code Evidence - SetupPhase is NEVER populated:**

In `runner.rs` `RunMsbAgentSingleRequest::run()` (lines 1158-1467):
1. Line 1159: `let mut run_record = RunRecord::new(&prepared);` - Creates record with `phases: RunPhases::default()`
2. RunPhases::default() sets `setup: None` (Option<SetupPhase>)
3. **NO code ever assigns `run_record.phases.setup = Some(SetupPhase { ... })`**
4. The runner writes separate JSON files but never populates RunRecord.phases.setup

Verification from actual `record.json.gz`:
```bash
$ zcat record.json.gz | jq '.phases.setup'
null
```

**Key Gaps Identified:**

1. **SetupPhase is not populated with indexing results** - The struct exists but is never instantiated
2. **Indexing data only exists in separate JSON files** - `indexing-status.json`, `parse-failure.json`, `repo-state.json`
3. **RunRecord.phases.setup is always null** - Verified from actual compressed record
4. **Missing indexed_crates list** - No field exists to capture which crates were successfully indexed
5. **ParseFailureRecord lacks diagnostics** - Has `target_dir`, `message`, `occurred_at_ms` but missing `diagnostics: Vec<FlattenedParserDiagnostic>` from ParseFailureArtifact
6. **Missing eval-design.md §VIII requirement** - "Index/database snapshot ID" is NOT captured anywhere

**Can we validate parse results from RunRecord alone?** **NO**

The dual-syn validation requires knowing:
- Which crates were indexed? → **NOT in RunRecord** (only in `indexing-status.json`)
- Any parse failures? → **NOT in RunRecord** (only in `parse-failure.json`)
- What specifically failed? → **NOT in RunRecord** (diagnostics only in `parse-failure.json`)

**Recommendation:**

Add SetupPhase population to runner.rs in `RunMsbAgentSingleRequest::run()` after indexing completes (around line 1337):

```rust
// Populate SetupPhase after indexing completes
let setup_phase = SetupPhase {
    started_at: setup_start_time.to_rfc3339(),  // Need to capture this at start
    ended_at: chrono::Utc::now().to_rfc3339(),
    repo_state: repo_state.clone(),  // Already captured at line 1168
    indexing_status: indexing_status.clone(),  // Already captured at line 1328
    parse_failure: if parse_failure_path.exists() { 
        // Load and include parse-failure.json content
        Some(load_parse_failure_record(&parse_failure_path)?)
    } else { 
        None 
    },
    db_timestamp_micros: state.db.current_validity_micros()?,  // Capture Cozo timestamp
};
run_record.phases.setup = Some(setup_phase);
```

Additional fields to add to SetupPhase:
1. `indexed_crates: Vec<IndexedCrateSummary>` - List of successfully indexed crates
2. `indexing_snapshot_id: Option<String>` - DB snapshot ID per eval-design.md §VIII
3. Change `parse_failure: Option<ParseFailureRecord>` to `parse_failures: Vec<ParseFailureRecord>`
4. Add `diagnostics: Vec<FlattenedParserDiagnostic>` to ParseFailureRecord

---

### DB Query/Replay Findings from Agent 3

**Investigated:** `crates/ploke-db/src/observability.rs`, query methods, replay tests

**Can we query DB at historical timestamp?** NO

**Evidence:**

```rust
// 1. Timestamp storage EXISTS in RunRecord
pub struct TimeTravelMarker {
    pub turn: u32,
    pub timestamp_micros: i64,  // Cozo validity timestamp in microseconds
    pub event: String,
}

// RunRecord stores these markers
pub struct RunRecord {
    pub db_time_travel_index: Vec<TimeTravelMarker>,
    // ...
}

// 2. Capturing timestamps WORKS
// In runner.rs line 1374-1381:
let db_timestamp = state
    .db
    .current_validity_micros()  // Gets Cozo validity timestamp
    .map_err(|e| ...)?;
run_record.mark_time_travel(1, db_timestamp, "turn_complete");

// 3. But query capability? NONE.
// The `current_validity_micros()` method docs claim:
/// This timestamp can be used with Cozo's `@` operator to query historical database state:
/// ```text
/// *relation{ ... } @ timestamp_micros
/// ```

// 4. ALL actual queries use @ 'NOW' - NEVER a historical timestamp
// From observability.rs line 345:
*conversation_turn{ id, at, parent_id, message_id, kind, content, thread_id @ 'NOW' },

// From observability.rs line 646:
*tool_call{ request_id, call_id, at, ... @ 'NOW' },

// From database.rs line 358:
?[{query_fields}] := *{relation} {{ {query_fields} @ 'NOW' }}, ({key_match})

// 5. QueryBuilder hardcodes @ 'NOW'
// From query/builder.rs line 220:
let right: &'static str = " @ 'NOW' }";
```

**Key Gaps Identified:**

1. **Timestamps are stored but NEVER used for historical queries**: The `db_time_travel_index` in RunRecord captures timestamps at each turn, but there is no code path that uses these timestamps to query past DB state.

2. **No method accepts a timestamp parameter**: 
   - `raw_query()` - takes only query string, no timestamp
   - `run_script()` - no timestamp parameter
   - `QueryBuilder` - no timestamp field, hardcoded `@ 'NOW'`
   - All query helpers - hardcoded `@ 'NOW'`

3. **No `replay_query(turn, query)` function exists**: As specified in eval-design.md §VII, this function should "run an arbitrary query against the DB snapshot from this turn" - completely missing.

4. **No `turn.db_state().lookup(name)` exists**: As specified in eval-design.md §VII, this should answer "does this node exist at the time the agent queried for it?" - completely missing.

**What Would Need to Be Built:**

```rust
// Option 1: Extend raw_query to accept optional timestamp
pub fn raw_query_at(&self, query: &str, timestamp_micros: i64) -> Result<QueryResult, DbError> {
    // Replace @ 'NOW' with @ timestamp_micros in query
    // OR require caller to use @ timestamp in query string
    self.run_script(query, params, ScriptMutability::Immutable)
}

// Option 2: Add replay method to RunRecord
impl RunRecord {
    pub fn replay_query(&self, turn: u32, query: &str) -> Result<QueryResult, DbError> {
        let timestamp = self.timestamp_for_turn(turn)
            .ok_or_else(|| "No timestamp for turn")?;
        // Open DB snapshot and query at timestamp
    }
}

// Option 3: Full TurnDbState API as designed in eval-design.md
impl TurnRecord {
    pub fn db_state(&self) -> DbState {
        DbState { timestamp: self.db_timestamp_micros, ... }
    }
}

impl DbState {
    pub fn lookup(&self, name: &str) -> Result<bool, DbError> {
        // Query using self.timestamp
    }
}
```

**Recommendation:**

**Priority: HIGH** - This is a core Phase 1 deliverable from eval-design.md §VII that is completely missing. The timestamps are being captured but cannot be used. Implementation requires:

1. Add `raw_query_at_timestamp()` method to Database
2. Implement `replay_query(turn, query)` on RunRecord 
3. Implement `turn.db_state().lookup(name)` API
4. Add tests verifying historical queries return different results than @ 'NOW'

**Estimated effort:** 1-2 days for basic implementation, 1 day for testing.
