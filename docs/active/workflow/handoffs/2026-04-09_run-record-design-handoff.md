# 2026-04-09 Handoff: Run Record Design

**From:** Agent  
**To:** Next session  
**Date:** 2026-04-09  
**Branch:** `refactor/tool-calls`  
**Related:** [2026-04-09_run-record-type-inventory.md](../../agents/2026-04-09_run-record-type-inventory.md), [2026-04-09_run-manifest-design-note.md](../../agents/2026-04-09_run-manifest-design-note.md)

---

## What Was Done

### 1. AGENTS.md Updated
Added "Eval Workflow and Research Operations" section linking to all workflow documentation.

### 2. Doc Review Follow-ups Resolved
- **A5 confirmed as hard gate** for H0 interpretation
- **Diagnostic hypothesis format** decided: `D-{DOMAIN}-{NNN}` (e.g., `D-TOOL-001`)
- Validity-guard thresholds deferred until next formal run

### 3. Hypothesis Registry Updated
- A5 marked `active` with hard gate note
- Added 3 example diagnostic hypotheses as templates

### 4. Run Record Design Clarified
**Key insight:** Cozo time travel changes the snapshot strategy.

- **OLD (incorrect):** Multiple DB snapshots per turn
- **NEW (correct):** Single final DB + timestamps for `@` queries

### 5. Type Inventory Created
Comprehensive catalog of all serializable types from:
- `ploke-llm` (LLM API layer)
- `ploke-eval` (Eval harness layer)
- External references from `ploke-tui`, `ploke-db`

---

## Key Design Decisions

### Run Manifest vs Run Record Split

| Aspect | Run Manifest (`run.json`) | Run Record (`record.json.gz`) |
|--------|---------------------------|-------------------------------|
| **Purpose** | Quick comparison, indexing | Full replay and introspection |
| **Size** | Small (~KB) | Large (~MB compressed) |
| **Content** | Differentiating config | Per-turn detailed state |
| **Human readable** | Yes | No (compressed) |
| **Cozo queries** | No | Yes (via timestamps) |

### Cozo Time Travel Integration

```rust
// Query DB state at specific turn
db.run(&format!("?[node] := *nodes{{name: 'foo', node, @ {}}}", timestamp_micros))
```

**Required:** Store `timestamp_micros` at each turn boundary in `db_time_travel_index`.

### Proposed Run Record Structure

```
RunRecord
├── metadata (from manifest)
├── phases
│   ├── setup (RepoStateArtifact, IndexingStatusArtifact)
│   ├── agent_turns (Vec<TurnRecord>)
│   ├── patch (PatchArtifact)
│   └── validation (build/test results)
├── db_time_travel_index (Vec<TimeTravelMarker>)
└── conversation (Vec<RequestMessage>)
```

---

## Artifacts Created

1. **[2026-04-09_run-record-type-inventory.md](../../agents/2026-04-09_run-record-type-inventory.md)**
   - Complete type catalog with file paths and line numbers
   - Proposed `RunRecord` structure
   - Implementation checklist (TODO items)

2. **[2026-04-09_run-manifest-design-note.md](../../agents/2026-04-09_run-manifest-design-note.md)**
   - Cozo time travel clarification
   - Revised artifact layout
   - Example manifest and record structures

3. **Updated hypothesis registry** with diagnostic hypotheses

4. **Updated AGENTS.md** with eval workflow references

---

## Next Steps (Priority Order)

### Immediate (Design → Implementation)

1. **Define Rust types for run record**
   - Create `crates/ploke-eval/src/record.rs`
   - Define `RunRecord`, `TurnRecord`, `TimeTravelMarker`
   - Derive Serialize/Deserialize

2. **Update runner to emit record**
   - Add `record_path` to `RunArtifactPaths`
   - Capture timestamps at turn boundaries
   - Build `db_time_travel_index` during execution
   - Compress with `flate2` or similar

3. **Implement introspection API**
   - `RunRecord::query_at_turn(turn, query)` using `@ timestamp`
   - `RunRecord::conversation_up_to_turn(turn)`
   - `RunRecord::tool_calls_in_turn(turn)`

### Short-term (Phase 1 Exit Criteria)

4. **Converge split artifacts**
   - Ensure `run.json` + `record.json.gz` covers all current split files
   - Deprecate redundant artifact files gradually

5. **Add replay capability**
   - Load run record
   - Reconstruct conversation state at any turn
   - Re-execute tool calls against current DB (counterfactual replay)

6. **Update manifest draft**
   - Update `docs/workflow/run-manifest.v0.draft.json` to match design

---

## Open Questions

1. **Compression format:** `gzip` vs `zstd` vs `lz4`? (trade-off: speed vs size)
2. **Chunking:** Should large runs support chunked records? (for very long agent sessions)
3. **Backward compatibility:** How to handle schema evolution in records?
4. **Embedding in record:** Should we include full embedding vectors or reference them in Cozo?

---

## Resume Prompt

Continue from [2026-04-09_run-record-type-inventory.md](../../agents/2026-04-09_run-record-type-inventory.md).

Start with the implementation checklist:
1. Define `RunRecord` struct
2. Update runner to capture timestamps and emit record
3. Implement basic introspection API

Reference the type inventory for all required imports and field types.
