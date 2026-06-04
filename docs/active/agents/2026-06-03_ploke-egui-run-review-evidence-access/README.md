# ploke-egui: run-review evidence access

**Date:** 2026-06-03 (second pass, code-verified)  
**Status:** research / planning (no implementation)  
**Sibling audits:** [audit-run-review-citations.md](audit-run-review-citations.md), [audit-egui-surfaces.md](audit-egui-surfaces.md), [audit-snapshot-contents.md](audit-snapshot-contents.md)

## Problem

Durable run reviews require filesystem joins (`validation-audit.json`, `llm-full-responses.jsonl`, checkout `git diff`, `run_trace_audit.py`, etc.). `ploke-egui` already ingests much of `record.json.gz` and protocol JSON into `ploke_tree::Graph` when loading a campaign run root or a `GraphSnapshot` export. The gap is surfacing review-style **Evidence roots** and disk-only witnesses in one UI — not absence of typed run-record data in the graph.

## Verified: evidence vs graph vs UI

| Category | In `Graph` / export snapshot | In egui UI | Rating |
|----------|----------------------------|------------|--------|
| Closure + instance artifact paths | Yes (`ClosureEvidence`, `ClosureArtifactRefsRecord`) | No consolidated strip; Eval & Protocol shows status only | **Partial** |
| Per-run `registries/runs/*.json` | Path in closure refs | No; dashboard “registry” = campaign registry closure status | **Partial (data) / No (UI)** |
| `record.json.gz` | Full `RunRecord` in `RunRecordEvidence` | Inspector + packaging: turns, tools, LLM trace, patch diff when on record | **Yes** |
| Agent-turn sidecars | If present at import (`AgentTurnEvidence`, `AgentTurnRecordSet`) | Mostly via embedded `RunRecord`; sidecar fallback in parent-create | **Partial** |
| `llm-full-responses.jsonl` | Not loaded in `ploke-tree` | No | **No** |
| `validation-audit.json` | Not loaded; `ValidationPhase` on record unused in UI | No | **No** |
| Benchmark / MSB / projection files | Packaging states on record; paths in closure | Partial counts/labels; not audit tables or jsonl body | **Partial** |
| Patch body | `phases.patch.diff` on record when written | **Run Patch Generation** code block | **Partial** |
| Checkout `git diff` | `repo_root` on metadata only | No live git | **No** |
| Protocol `*_tool_call_review_*` | `ProtocolArtifactsEvidence` | Analyst Snapshot, Call Review Scan, detail | **Yes** |
| Sealed selections / trajectory | Requires `history_blocks` | Selection Story + trajectory when non-empty | **Partial** |
| Transition journal / nodes | In `RunRecordSet` | Entity counts; not full EvalOps board | **Partial** |
| `run_trace_audit.py` | N/A | CLI | **No** |
| WASM `protocol-graph.json` | Embeds records + absolute paths; `history_blocks` often `[]` | Same UI; paths not shown as review “Evidence roots” | **Yes** load / **Partial** provenance UX |

**Code anchors:** `FsRunStore::load_passive_evidence` / `load_run_record_evidence` (`crates/ploke-tree/src/store/fs.rs`); `Graph::eval_protocol_evidence`, `trajectory_generations` (`crates/ploke-tree/src/graph/types.rs`); `EvalProtocolDashboard` (`crates/ploke-egui/src/ui/eval_protocol.rs`); Analyst Snapshot / closure detail (`crates/ploke-egui/src/ui/app/shell/eval_protocol.rs`); run records / LLM trace / patch generation (`run_records.rs`, `llm_trace.rs`, `shell.rs`); sidebar counts (`identity.rs` — `artifacts` = `graph.artifacts.artifacts.len()`, `selections` = `graph.selections.selections.len()`); `default_selections` = artifact-tree nodes, not sealed selections (`inspector.rs`); WASM default (`bootstrap.rs`); `GraphSnapshot` (`graph_snapshot.rs`).

## Corrections (first pass)

1. **Run records are first-class in the graph** — not “thin rows only”; contradicts older `protocol-and-evaluation-data-locations.md` where it says record does not reach Graph.
2. **WASM exports embed absolute evidence paths** in closure + record keys; UI does not expose them as a review “Evidence roots” block.
3. **Patch diff can appear in UI** when stored on `RunRecord.phases.patch`; checkout diff and projection JSON files still require disk.
4. **Dashboard “registry”** is closure campaign registry status, not per-run registration JSON content.
5. **`selections 0`** on protocol export reflects empty sealed history (`history_blocks: []`), not an empty run or missing eval/protocol data.

## `protocol-graph.json` (targeted facts)

- `format_version: 1`; passive evidence includes `closure`, `protocol_artifacts`, `run_records` (full record JSON keyed by absolute `record_path`).
- `history_blocks: []` → sidebar `selections 0`; trajectory / `score_child_prop` N/A on this export (expected for protocol-only / non-sealed runs).
- Closure `instances[].artifacts` retains `registration_path`, `run_root`, `record_path`, `execution_log`, `msb_submission`, `protocol_artifacts_dir`, etc.
- Turn data under `run_records` phases, not necessarily top-level `agent_turn_records` in export.

## Run reviews still need disk for

Evidence roots block; **`execution-log.json` step names** (gate-required, was missing from first inventory); **`registries/runs/<run-id>.json` semantics**; `validation-audit.json`; **`agent-turn-trace.json` + `llm-full-responses.jsonl` + `run_trace_audit.py`** (primary trace reconstruction); `benchmark-patch-projection.json` / MSB jsonl witnesses; **checkout `git diff` / `rg`**; campaign-wide `nodes/*` for harness reviews.

**Snapshot + UI already cover:** Parsed run record, protocol artifact bodies, closure status, protocol counts, turn outcomes and tool lifecycle from record, patch diff when on record.

## Failed / aborted run (grounded)

| Question | Protocol export / egui | Full run dir + review |
|----------|------------------------|------------------------|
| Protocol reviews? | Good when `protocol_artifacts` embedded | Same |
| Agent turn aborted? | `TurnOutcome` on embedded record turns | + `agent-turn-summary.json` |
| Validation red? | Not in UI today | `validation-audit.json` |
| Nonempty patch vs abort? | Submission/projection **states** + patch diff if on record | Audit + checkout + projection file |
| Selection / score_child_prop? | `selections 0` if `history_blocks: []` | Multi-gen fixture or live import |

## Phased proposals (verified gaps only)

**Phase 1:** Provenance strip from `ClosureArtifactRefsRecord` + primary `record_path` (copyable); label WASM snapshot vs native `--run-root`; clarify sidebar `selections` vs `artifacts`.

**Phase 2:** Ingest/render `validation-audit.json` and `llm-full-responses.jsonl`; optional deep-link query params.

**Phase 3:** Review-packet export; campaign harness index; checkout witness (explicitly out of graph today).

## Related

- [Eval & Protocol analyst surface plan](../../../crates/ploke-egui/docs/plan/eval-protocol-analyst-surface/main-plan.md)
- [Inspector UX discipline](../../../crates/ploke-egui/docs/style/inspector-ux.md)
- [protocol-and-evaluation-data-locations.md](../../../crates/ploke-egui/docs/model/protocol-and-evaluation-data-locations.md) — **stale** on run-record → graph boundary; update when implementing
- [2026-06-03 WASM UX review](../2026-06-03_ploke-egui-wasm-ux-review/README.md)
- Representative reviews: [completed ripgrep](../run-reviews/2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-burntsushi-ripgrep-2209-run-1779706500140-eval.md), [aborted isolated](../run-reviews/2026-05-25-p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746-burntsushi-ripgrep-2209-run-1779709154252-eval.md)
