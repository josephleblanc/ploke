# 2026-04-09 Run Manifest and Record Design Note

**Date:** 2026-04-09  
**Subject:** Clarifying DB snapshot strategy using Cozo time travel  
**Related:** [eval-design.md](../plans/evals/eval-design.md), [run-manifest.v0.draft.json](../../../workflow/run-manifest.v0.draft.json)

---

## Key Clarification: Cozo Time Travel

Our database (Cozo) has a **time travel** feature that allows querying historical states without multiple snapshots.

### How Cozo Time Travel Works

From [Cozo time-travel docs](../../dependency_details/cozo/types/time-travel.md):

- Relations with `Validity` type as the last key part can be queried at any historical timestamp
- Query syntax: `?[name] := *rel{id: $id, name, @ 789}` where `@ 789` is the timestamp
- Timestamp can be microseconds since UNIX epoch, RFC 3339 strings, or special values `NOW`/`END`
- All data is preserved (immutable) - nothing is truly deleted

### Implications for Run Artifacts

**OLD thinking (incorrect):**
- Take DB snapshots at each turn boundary
- Store multiple `.sqlite` files per run
- Query by loading the appropriate snapshot

**CORRECTED thinking:**
- Single final DB per run with time travel enabled
- Store timestamps at key events (turn boundaries, failures)
- Query historical state using Cozo's `@ timestamp` syntax
- Only take explicit snapshots at failure points for debugging aborted runs

---

## Revised Artifact Structure

```
runs/
├── 2026-04-09/
│   ├── run-001/
│   │   ├── run.json           # Manifest (differentiating config)
│   │   ├── record.json.gz     # Full per-turn state with timestamps
│   │   ├── final.db           # Cozo DB with time travel (single file!)
│   │   └── failure-snapshots/ # Only if run aborted
│   │       └── db-turn-15.sqlite  # Explicit snapshot at failure
```

---

## Run Manifest (`run.json`) - Minimal

```json
{
  "schema_version": "run-manifest.v1",
  "manifest_id": "run-2026-04-09-001",
  
  "experiment": {
    "experiment_id": "exp-001",
    "hypothesis_ids": ["H0", "A1"],
    "arm_tag": "structured-all-tools"
  },
  
  "benchmark": {
    "dataset_kind": "multi_swe_bench",
    "instance_id": "org__repo-1234",
    "base_sha": "abc123"
  },
  
  "agent": {
    "model_id": "anthropic/claude-sonnet-4",
    "provider": "anthropic",
    "system_prompt_sha256": "sha256:...",
    "tool_schema_version": "v2"
  },
  
  "runtime": {
    "temperature": 0.0,
    "max_turns": 40,
    "max_tool_calls": 200
  },
  
  "timing": {
    "started_at": "2026-04-09T18:30:00Z",
    "ended_at": "2026-04-09T18:47:32Z",
    "wall_clock_secs": 1052.0
  },
  
  "outcome": {
    "run_status": "completed",
    "agent_outcome": "solved",
    "benchmark_verdict": "passed",
    "failure_classification": {
      "primary": null,
      "secondary": [],
      "confidence": "high"
    }
  },
  
  "metrics_summary": {
    "turn_count": 37,
    "token_cost_input": 15234,
    "token_cost_output": 4892,
    "tool_call_count": 45
  },
  
  "artifacts": {
    "record_path": "runs/2026-04-09/run-001/record.json.gz",
    "final_db_path": "runs/2026-04-09/run-001/final.db"
  }
}
```

---

## Run Record (`record.json.gz`) - With Timestamps

```json
{
  "schema_version": "run-record.v1",
  "manifest_id": "run-2026-04-09-001",
  
  "phases": [
    {
      "phase": "setup",
      "started_at": "2026-04-09T18:30:00.000Z",
      "ended_at": "2026-04-09T18:30:15.432Z",
      "steps": ["load_manifest", "checkout_base_sha", "write_repo_state"]
    },
    {
      "phase": "agent_turns",
      "turns": [
        {
          "turn_number": 1,
          "started_at": "2026-04-09T18:30:15.500Z",
          "ended_at": "2026-04-09T18:30:45.123Z",
          "db_timestamp_micros": 1744223415500000,
          
          "llm_request": { "...": "ChatCompReqCore" },
          "llm_response": { "...": "OpenAiResponse" },
          "tool_calls": [...],
          "outcome": "tool_calls_executed"
        }
      ]
    }
  ],
  
  "db_time_travel_index": [
    {"turn": 0, "timestamp_micros": 1744223415000000, "event": "setup_complete"},
    {"turn": 1, "timestamp_micros": 1744223415500000, "event": "turn_start"},
    {"turn": 1, "timestamp_micros": 1744223416512300, "event": "turn_complete"}
  ]
}
```

---

## Introspection API with Time Travel

```rust
// Example: Check if a node existed when agent queried for it
impl RunRecord {
    fn query_db_at_turn(&self, turn: usize, query: &str) -> QueryResult {
        let timestamp = self.db_time_travel_index
            .find(|e| e.turn == turn)
            .map(|e| e.timestamp_micros)
            .expect("valid turn");
        
        // Use Cozo's @ timestamp syntax
        let time_travel_query = format!("{} @ {}", query, timestamp);
        self.final_db.run(&time_travel_query)
    }
}
```

---

## Summary

| Aspect | Old (incorrect) | Corrected |
|--------|----------------|-----------|
| DB Snapshots | Multiple per turn | Single final DB |
| Historical query | Load snapshot file | Cozo `@ timestamp` syntax |
| Storage overhead | N × DB size | 1 × DB size + timestamps |
| Failure debugging | Last snapshot | Explicit failure snapshot only |

---

## Action Items

1. Update `run-manifest.v0.draft.json` to remove multiple snapshot paths
2. Add `db_time_travel_index` to run record schema
3. Ensure Cozo schema uses `Validity` type for time-travel-enabled relations
4. Document the `@ timestamp` query pattern in introspection API
