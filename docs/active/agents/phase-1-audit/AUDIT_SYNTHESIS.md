# Phase 1 Implementation Audit - Synthesis

**Audit completed:** 2026-04-11  
**Status:** CRITICAL GAPS IDENTIFIED  
**Recommendation:** Phase 1 should NOT be marked complete

---

## Executive Summary

The audit reveals **significant gaps** between claimed and actual Phase 1 implementation. While RunRecord types exist and basic introspection methods work, **core Phase 1 deliverables from eval-design.md are missing**:

1. **`turn.db_state().lookup()` - NOT IMPLEMENTED** (minimum deliverable per §VII)
2. **`replay_query(turn, query)` - NOT IMPLEMENTED** (minimum deliverable per §VII)
3. **SetupPhase is NEVER populated** - Cannot determine indexing status from RunRecord
4. **Historical DB queries - NOT POSSIBLE** - Timestamps captured but unusable

---

## Gap Priority Matrix

| Gap | Blocks A4 | Blocks A5 | Blocks H0 | Effort | Priority |
|-----|-----------|-----------|-----------|--------|----------|
| SetupPhase not populated | Medium | High | High | 4 hrs | **P0** |
| `turn.db_state().lookup()` missing | High | Critical | Critical | 1-2 days | **P0** |
| `replay_query()` missing | Medium | Critical | High | 1 day | **P0** |
| Historical queries not possible | High | Critical | Critical | 1-2 days | **P0** |
| Missing iterator methods (`conversations()`, `tool_calls()`) | Low | Medium | Low | 4 hrs | P1 |
| Missing `run.failures()`, `run.config()` | Medium | Medium | Low | 4 hrs | P1 |

---

## Detailed Findings by Area

### Area 1: Introspection API (Agent 1)

**IMPLEMENTED (12 methods):**
- `new()`, `mark_time_travel()`, `turn_timestamp()` / `timestamp_for_turn()`
- `tool_calls_in_turn()`, `turn_record()`, `llm_response_at_turn()`
- `total_token_usage()`, `turn_count()`, `outcome_summary()`
- `was_tool_used()`, `turns_with_tool()`, `replay_state_at_turn()` (data-only)

**MISSING (13 methods from eval-design.md §VII):**
- `run.conversations()` → iterator
- `run.tool_calls()` → all tool calls  
- `run.db_snapshots()` → DB state
- `run.metrics()` → unified metrics
- `run.failures()` → classified failures
- `run.config()` → run configuration
- `run.manifest()` → full manifest
- `turn.messages()` → turn messages
- `turn.tool_call()` → single tool call
- `turn.tool_result()` → tool result
- `turn.db_state()` → queryable DB snapshot
- `turn.db_state().lookup(name)` → **CRITICAL: minimum Phase 1 deliverable**
- `replay()`, `replay_tool_call()`, `replay_query()` → **CRITICAL: all replay functions**

**Key Issue:** `turn.db_state().lookup()` was explicitly listed as a **minimum Phase 1 deliverable** in `eval-design.md` §XIII: "Implement `turn.db_state().lookup(name)` — because you specifically mentioned wanting to answer 'does this node exist at the time the agent queried for it?'"

---

### Area 2: RunRecord Schema (Agent 2)

**COMPLETE types:**
- `RunRecord` - all required fields present
- `RunPhases` - all 4 phases defined  
- `TurnRecord` - all required fields present

**INCOMPLETE types:**

| Field | Status | Issue |
|-------|--------|-------|
| `SetupPhase.indexed_crates` | **MISSING** | No field to capture which crates were indexed |
| `SetupPhase.indexing_errors` | **MISSING** | No field for indexing error collection |
| `SetupPhase.tool_schema_version` | **MISSING** | Exists in AgentMetadata but not SetupPhase |
| `SetupPhase.parse_failure` | PARTIAL | Only single failure, missing diagnostics |
| `TimeTravelMarker` | PARTIAL | Uses `event` label instead of explicit pre/post fields |

**Phase 1 Deliverable Gap:**
The `phased-exec-plan.md` requires capturing "**failure classifications**" in the run data schema. The current SetupPhase cannot capture multiple parse failures or detailed diagnostics.

---

### Area 3: DB Query/Replay Capability (Agent 3)

**Can we query DB at historical timestamp?** **NO**

**What EXISTS:**
- `current_validity_micros()` returns Cozo validity timestamps
- `TimeTravelMarker` struct stores `(turn, timestamp_micros, event)`
- `db_time_travel_index: Vec<TimeTravelMarker>` captures timestamps

**What's MISSING:**
- No method accepts a timestamp parameter
- ALL queries hardcode `@ 'NOW'`
- No `replay_query(turn, query)` function
- No `turn.db_state().lookup(name)` API

**Evidence:**
```rust
// Every single query uses @ 'NOW'
*conversation_turn{ ... @ 'NOW' },
*tool_call{ ... @ 'NOW' },

// QueryBuilder hardcodes it:
let right: &'static str = " @ 'NOW' }";
```

**The timestamps are being captured but NEVER used.** This is a core Phase 1 deliverable that is completely missing.

---

### Area 4: SetupPhase Capture (Agent 4)

**SetupPhase Current State:**
```rust
pub struct SetupPhase {
    pub started_at: String,
    pub ended_at: String,
    pub repo_state: RepoStateArtifact,
    pub indexing_status: IndexingStatusArtifact,
    pub parse_failure: Option<ParseFailureRecord>,  // Only single failure
    pub db_timestamp_micros: i64,
}
```

**CRITICAL FINDING:** SetupPhase is **NEVER populated**

In `runner.rs`:
1. `RunRecord::new()` creates `phases: RunPhases::default()` which sets `setup: None`
2. **NO code ever assigns `run_record.phases.setup = Some(SetupPhase { ... })`**
3. Runner writes separate JSON files but never populates RunRecord

**Verified from actual run artifact:**
```bash
$ zcat record.json.gz | jq '.phases.setup'
null
```

**Can we validate parse results from RunRecord alone?** **NO**

| Data | In RunRecord? | Location |
|------|---------------|----------|
| indexed_crates | NO | Only in `indexing-status.json` |
| parse_failures | NO | Only in `parse-failure.json` |
| diagnostics | NO | Only in `parse-failure.json` |
| repo_state | NO | Only in `repo-state.json` |

---

## Revised Phase 1 Exit Criteria

Based on the audit, the following must be completed for Phase 1 to be truly complete:

### P0 (Critical - Blocks A5/H0)

1. **Populate SetupPhase in runner.rs**
   - Capture setup_start_time at beginning
   - After indexing completes, create SetupPhase with all fields
   - Assign to `run_record.phases.setup`

2. **Add indexed_crates field to SetupPhase**
   - Create `IndexedCrateSummary` type
   - Populate with list of crates that were indexed

3. **Implement DB query at historical timestamp**
   - Add `raw_query_at_timestamp()` to Database
   - Support Cozo `@ timestamp_micros` syntax

4. **Implement `turn.db_state().lookup(name)`**
   - Create `DbState` wrapper type
   - Add `db_state()` method to `TurnRecord`
   - Implement `lookup()` using stored timestamp

5. **Implement `replay_query(turn, query)`**
   - Add to RunRecord impl
   - Use timestamp from db_time_travel_index

### P1 (Important - Completes API surface)

6. Add iterator methods: `run.conversations()`, `run.tool_calls()`, `run.db_snapshots()`
7. Add missing methods: `run.failures()`, `run.config()`, `run.manifest()`
8. Support multiple parse failures in SetupPhase
9. Add diagnostics to ParseFailureRecord

---

## Recommendations

### Immediate Actions

1. **DO NOT claim Phase 1 is complete** until P0 items are done
2. **Update `phase-1-runrecord-tracking.md`** to reflect actual status
3. **Update `recent-activity.md`** to document the gaps
4. **Create implementation tasks** for P0 items

### For Dual-Syn Validation (Current Blocker)

The immediate need is to validate dual-syn parsing on ripgrep. Short-term options:

**Option A: Quick fix** (4 hours)
- Add `indexed_crates` method to RunRecord that reads from `indexing-status.json`
- This unblocks validation without full SetupPhase implementation

**Option B: Proper fix** (1-2 days)
- Implement full SetupPhase population as described above
- This is required for Phase 1 completion anyway

**Recommendation:** Implement Option B since it's required for Phase 1 and blocks A5 (replay/introspection).

---

## Summary Table: Claimed vs Actual

| Deliverable (from phased-exec-plan.md) | Claimed Status | Actual Status |
|----------------------------------------|----------------|---------------|
| Define run data schema | ✅ Complete | ⚠️ Partial - schema defined but SetupPhase empty |
| Implement introspection API (minimum) | ✅ Complete | ❌ **INCOMPLETE** - `lookup()` and `replay_query()` missing |
| Implement automated triage | Not started | Not started |
| Validate eval harness (A4) | Not started | Blocked by missing introspection |
| Implement basic replay | ✅ Complete | ❌ **INCOMPLETE** - No functional replay |
| Implement version identifiers | ✅ Complete | ⚠️ Partial - versions not in SetupPhase |

**Phase 1 Actual Status: INCOMPLETE**  
**Estimated remaining effort: 3-4 days**
