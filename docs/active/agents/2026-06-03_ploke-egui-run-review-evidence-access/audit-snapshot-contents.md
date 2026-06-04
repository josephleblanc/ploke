# Audit: GraphSnapshot export vs run-review disk evidence

**Date:** 2026-06-03

## Export shape

`GraphSnapshot` = `{ format_version: 1, records: RunRecordSet }` from `FsRunStore::load_record_set()` only (`graph_snapshot.rs`, `export-graph` CLI / UI export).

**In `RunRecordSet`:** `forest_input` (scheduler, nodes, passive_evidence), `history_blocks`, `transition_journal`, `agent_turn_records` (only when files exist under export `run_root`).

**`passive_evidence` loads when present:** closure, protocol artifacts (full JSON bodies), run records (full `record.json.gz`), evaluations, branch registry, history summaries, agent-turn paths, etc.

**Not in snapshot:** `validation-audit.json`, `llm-full-responses.jsonl`, `execution-log.json` bodies, registry JSON, checkout, `run_trace_audit.py` output.

## `protocol-graph.json` (user export)

| Field | Value |
|-------|--------|
| `history_blocks` | **0** → sidebar `selections 0` (source data, not export drop) |
| Campaign | `p1-g31pro-direct-p35flash-20260604-011844` |
| `protocol_artifacts` | 57 indexed bodies |
| `run_records` | 1 full embedded record |
| Closure / paths | Absolute paths in snapshot; file bodies mostly not embedded except record + protocol |

## `trajectory-multi-gen.json`

| Field | Value |
|-------|--------|
| `history_blocks` | **6** `selection_decision` payloads |
| Traversal strategy | `score_child_prop` on all 6 |
| `formula` on payloads | **false** on all 6 |
| `score_child_prop_total` in UI | Empty — **data gap** (no formula/score rows in sealed history), not export stripping |

## vs aborted run review (113746)

Different campaign than `protocol-graph.json`. Snapshot from protocol-style root covers record + protocol reviews; still missing agent-turn sidecars (unless under export root), validation-audit, llm-full-responses, execution-log content, registry semantics, checkout verification.

## `score_child_prop` verdict

| Fixture | Why empty |
|---------|-----------|
| `protocol-graph.json` | No `history_blocks` |
| `trajectory-multi-gen.json` | Decisions exist; **no `formula`** → no metric witness totals |

## Code anchors

- `crates/ploke-tree/src/store/graph_snapshot.rs` — `from_run_root`
- `crates/ploke-tree/src/store/record_set.rs` — `RunRecordSet` fields
- `crates/ploke-tree/src/graph/types.rs` — `trajectory_generations()` + `score_child_prop_total()`

See [README.md](README.md), [audit-egui-surfaces.md](audit-egui-surfaces.md), [audit-run-review-citations.md](audit-run-review-citations.md).
