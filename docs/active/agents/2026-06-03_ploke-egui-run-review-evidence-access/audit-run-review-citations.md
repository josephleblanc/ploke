# Audit: run-review citations (ground truth)

**Date:** 2026-06-03  
**Sources:** `ploke-run-review` skill; durable reviews [035000 eval](../run-reviews/2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-burntsushi-ripgrep-2209-run-1779706500140-eval.md), [113746 isolated](../run-reviews/2026-05-25-p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746-burntsushi-ripgrep-2209-run-1779709154252-eval.md), [multigen 223658](../run-reviews/2026-05-23-p1-gemini35-flash-multigen-2g3x3-20260523-223658.md); [run-review-negative-examples](../run-review-negative-examples/README.md).

## First-pass README inaccuracies (citations lens)

1. **Run registry** — Reviews cite `~/.ploke-eval/registries/runs/<run-id>.json` in Evidence roots; not optional. First pass rated “No” in egui without separating **path in closure snapshot** vs **UI panel**.
2. **`execution-log.json`** — Omitted from first inventory; all three samples use it to prove pipeline steps (`bootstrap_headless_runtime`, `benchmark_turn_completed`, `write_validation_audit`, …). **Gate-required for execution-path proof.**
3. **Trace reconstruction** — Reviews lean on `agent-turn-trace.json`, `llm-full-responses.jsonl`, and `run_trace_audit.py`; `record.json.gz` is for parity/existence, not the primary inline `jq` narrative in eval bodies.

## Quality gate vs artifact categories

| Category | Pattern | Gate | Notes (three samples) |
|----------|---------|------|------------------------|
| Campaign | `~/.ploke-eval/campaigns/<id>/` | Partial | All name id; 035000/isolated add `closure-state.json`, `slice.jsonl` |
| Instance manifest | `run.json` | **Yes** | Task, base, model, expected file |
| Run root | `.../runs/<run-id>/` | **Yes** | Anchor |
| Run registration | `registries/runs/<run-id>.json` | **Yes** | Lifecycle / patching / protocol flags |
| `record.json.gz` | run root | **Yes** (parity) | Audit parity; not main prose jq source |
| `agent-turn-summary.json` | run root | **Yes** | completed vs aborted |
| `agent-turn-trace.json` | run root | **Yes** | Event + call-id chains |
| `llm-full-responses.jsonl` | run root | **Yes** | Provider ledger |
| `validation-audit.json` | run root | **Yes** | Cargo table; critical for aborted |
| `execution-log.json` | run root | **Yes** | Named pipeline steps |
| MSB submission / patch projection | run root | **Yes** | Export witnesses |
| Checkout | `~/.ploke-eval/repos/...` | **Yes** | `git diff`, `sed`, cargo |
| `run_trace_audit.py` | skill script | **Yes** (workflow) | 035000/isolated show command; multigen narrates counts only |
| Protocol dir / artifacts | under protocol tree | Optional* | *Required for protocol accounting reviews |
| Campaign spine (`nodes/`, harness) | campaign tree | N/A | Broad-harness reviews; not these three instance evals |

**Minimum gate bundle:** Evidence roots → execution path (`run.json` + `execution-log` + entrypoint) → trace audit parity → concrete chain (trace + llm + call ids) → checkout verification → mechanical vs benchmark split → action items tied to gaps.

## Per-review profile (short)

- **035000:** Full roots + registry + audit command + checkout `sed` verification + protocol partial-suite finding.
- **113746:** Adds closure-state, validation-audit table, patch vs protocol disagreement, negative search for unpersisted stdout.
- **223658:** Slimmer roots; trace via agent-turn-trace + checkout cargo; no registry path or audit command block in doc.

## Negative-example anti-patterns

Inventory-only; no gate pass statement; audit counts without execution-log path proof; no checkout verification of suspicious tools; weak call/event ids; conflating eval complete with benchmark usefulness; action items not tied to registry/closure/protocol contracts.

## Implications for egui evidence-access

Phase 1 strip should include **registration path**, **execution-log path**, and **run root** from closure refs (already in snapshot). Phase 2 should prioritize **validation-audit table**, **agent-turn sidecar summary**, and **trace-audit summary** over promoting raw `jq` on `record.json.gz`. See [README.md](README.md) verified graph/UI table.
