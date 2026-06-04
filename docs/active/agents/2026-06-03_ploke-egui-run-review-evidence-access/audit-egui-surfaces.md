# Audit: ploke-egui evidence surfaces (code-verified)

**Date:** 2026-06-03  
**Scope:** What the README claims vs what egui/ploke-tree actually load and render.

## Five corrections to first-pass README

1. **Sidebar `selections`** — `graph.selections.selections.len()` (sealed history decisions), not “no eval data.” Protocol export with empty `history_blocks` → 0 is expected.
2. **`default_selections()`** — artifact-tree nodes for picking (`inspector.rs`), not Selection Story or sidebar count.
3. **Agent-turn sidecars** — loaded into `graph.agent_turn_records` (`fs.rs`); **no egui panel**. LLM trace uses **embedded `RunRecord` turns** (`llm_trace.rs`).
4. **Dashboard “registry”** — `closure.state.registry.status`, not `registries/runs/<run-id>.json`.
5. **`validation-audit.json`, `execution-log.json`, `llm-full-responses.jsonl`** — **not imported** by ploke-tree today (not merely “no tab”).

## Claim table

| Claim | Verified? | Code pointer | Notes |
|-------|-----------|--------------|-------|
| Native `--run-root` → graph | Yes | `import/mod.rs`, `FsRunStore::load_record_set` | |
| `export-graph` (dev) | Yes | `cli/mod.rs`, `GraphSnapshot::from_run_root` | |
| WASM frozen snapshot | Yes | `bootstrap.rs`, `web.rs` | No native run picker on wasm32 |
| Native run picker | Yes | `run_picker/mod.rs` | `~/.ploke-eval/campaigns/.../prototype1` |
| Eval & Protocol dashboard | Yes | `eval_protocol.rs`, `shell/eval_protocol.rs` | When passive evidence present |
| Analyst Snapshot / Call Review | Yes | `shell/eval_protocol.rs`, `call_review.rs`, `protocol_detail.rs` | |
| Closure header “registry” | Yes (mislabeled) | `render_eval_protocol_dashboard_header` | Campaign closure registry phase |
| `record.json.gz` in UI | Yes | `load_run_record_evidence`, `run_records.rs`, `llm_trace.rs` | Turns, packaging, patch diff when on record |
| Agent-turn sidecar UI | No | `agent_turn_records` on graph | Loaded, not rendered |
| `llm-full-responses.jsonl` | No | — | Not in store |
| `validation-audit.json` | No | — | Not loaded |
| `execution-log.json` | No | — | Not loaded |
| Per-run registry JSON | No | — | Not loaded |
| Trajectory + Selection Story | Conditional | `trajectory.rs`, `inspector.rs` | Needs sealed selections |
| Sidebar `artifacts` | Yes | `identity.rs` | `graph.artifacts.artifacts.len()` |
| Sidebar `selections` | Yes | `identity.rs` | `graph.selections.selections.len()` |
| Evidence roots strip | No | — | Per-record paths only |
| Deep links `?run=` / `?call=` | No | — | `?graph=` only (WASM) |

## Module map

| Module | Role |
|--------|------|
| `ui/eval_protocol.rs` | Dashboard aggregates |
| `shell/eval_protocol.rs` | Main pane, Analyst Snapshot, closure sections |
| `shell/run_records.rs` | Inspector run-record drilldown |
| `shell/llm_trace.rs` | Run-record turn trace |
| `shell/identity.rs` | Sidebar counts |
| `shell/trajectory.rs` | Generation table, selection drilldown |
| `run_picker/mod.rs` | Native campaign discovery |

See [README.md](README.md) and [audit-run-review-citations.md](audit-run-review-citations.md).
