# Typed Persistence Implementation Slices

Updated: 2026-05-10

This file is the queue of record for the typed-persistence-spine implementation lane.

Start from [`operating-console.md`](operating-console.md), then use this file to decide the current implementation slice.

Use this file, not `.codex/task-stack.jsonl`, to decide what comes next for this lane. The task stack may contain cross-thread local reminders; this queue is the durable typed-persistence sequence.

## Rule

Ploke-owned persisted or transmitted JSON/JSONL is read and written through named Rust `Serialize` / `Deserialize` types. Production code must not inspect owned shapes through `serde_json::Value`, anonymous field walking, or stringly JSON payloads.

## Status Values

- `next`: start here.
- `queued`: not started.
- `blocked`: needs a named decision before implementation.
- `done`: implemented, verified, and reflected in the coverage report.

`done` may mean foundation-complete for a typed-persistence row. Operator
completion requires the row's facts to attach to the run execution graph and be
visible through a browser/egui projection, or for that graph/browser landing
work to be queued explicitly.

## Queue

| Order | Slice ID | Status | Source surfaces | Source report | Primary crates | Goal | Smallest verification |
|---:|---|---|---|---|---|---|---|
| 1 | `protocol-artifacts.decode` | `done` | `protocol.artifact.decode` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) | `ploke-records` | Decode implemented tool-call protocol artifact payloads and typed decode failures; not complete coverage for every current writer procedure. | `cargo test -p ploke-records --features protocol protocol` |
| 2 | `protocol-artifacts.current-writer-dtos` | `done` | `protocol.artifact.decode`, `protocol.artifact.store` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) | `ploke-records`, `ploke-eval` | Passive DTO coverage exists for all current `write_protocol_artifact` procedures, and the intervention synthesis writer no longer stages its artifact through `serde_json::Value` before persistence. | `cargo test -p ploke-records --features protocol protocol && cargo test -p ploke-eval protocol` |
| 3 | `protocol-artifacts.listing-load-result` | `done` | `protocol.artifact.store` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) | `ploke-records`, `ploke-eval` | Tolerant artifact listing now returns `Loaded(DecodedProtocolArtifactFile)` or typed unloaded rows for decode/unsupported/future/malformed payloads and identity mismatches, so one bad `.json` does not hide the rest of the directory. | `cargo test -p ploke-records --features protocol protocol && cargo test -p ploke-eval protocol` |
| 4 | `protocol-artifacts.aggregate-tool-call` | `done` | `protocol.artifact.aggregate.output` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) | `ploke-records`, `ploke-eval` | Aggregate consumes tolerant decoded tool-call variants, separately reports decoded non-tool-call skips, decoded tool-call payload-shape skips, and typed unloaded rows, and no longer deserializes owned protocol `output` values on the aggregate side. | `cargo test -p ploke-records --features protocol protocol && cargo test -p ploke-eval protocol` |
| 5 | `tool.call.arguments` | `done` | `tool.call.record.arguments`, `tool.request.arguments.capture` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) | `ploke-records`, `ploke-eval`, `ploke-tui` | Persisted eval tool-call arguments use `ToolArgumentsJson`, decode through a closed `ToolCallArguments` enum over `ploke_records::tool_contracts`, and surface typed parse-failure records; TUI observability no longer stages requested arguments through `serde_json::Value`. | `cargo test -p ploke-records --features tool-contracts tool_call && cargo test -p ploke-eval --lib tool` |
| 6 | `tool.result.trace.projection` | `done` | `tool.result.trace.projection` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) | `ploke-eval` | Tool-result failure/truncation summaries, `agent-turn-trace.json`, and Prototype 1 observation JSONL replay now deserialize through named typed projection records instead of anonymous field walking. Generic raw-payload pretty printing remains display-only. Full provider response DTO cleanup remains queued under `llm-attempts.dto-tool-bridge`. | `cargo test -p ploke-eval --lib tool_result` |
| 7 | `llm-attempts.provider-observation-projection` | `done` | `llm.attempt.provider_error`, `llm.attempt.timeout`, `llm.attempt.timeline` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) | `ploke-eval`, `ploke-llm` | Provider embedded-error detection, provider error/status stderr records, timeout flags, and attempt timeline replay now route through named typed projections. New provider-attempt traces emit flat timeline fields; legacy embedded provider-attempt strings remain as a scoped compatibility bridge. | `cargo test -p ploke-eval observation_log_loads_provider_attempt && cargo test -p ploke-llm provider_attempt_timeline_roundtrips_attempt_facts` |
| 8 | `run-execution-graph.browser-spine` | `next` | browser model export, `tool.call.record.arguments`, `tool.result.trace.projection`, `llm.attempt.provider_error`, `llm.attempt.timeout`, `llm.attempt.timeline` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl), [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) | `ploke-eval`, `ploke-tree-egui` | Establish the browser-facing generative execution graph spine from Runtime, Artifact, OperationCoordinate, PatchAttempt, derived Artifact, hydrated Runtime, evaluation, selection, and successor handoff; then attach already-typed tool-call, tool-result, provider-attempt, retry, timeout, and provider-failure facts as evidence on the relevant graph objects. | `cargo test -p ploke-eval browser_model && cargo check -p ploke-tree-egui` |
| 9 | `llm-attempts.dto-tool-bridge` | `queued` | `llm.dto.openai_response`, `llm.tool_bridge.records` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) | `ploke-llm` | Remove `serde_json::Value` from owned response metadata/logprobs and tool bridge argument records, after the current provider/tool facts have a visible graph landing path. | `cargo test -p ploke-llm` |
| 10 | `edit-surface.source-records` | `queued` | `edit_surface.*` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) | `ploke-records`, `ploke-eval`, `ploke-tui` | Decide and encode passive source records for proposal/candidate-local patch evidence without moving live mutation authority into `ploke-records`. | `cargo test -p ploke-records edit_surface && cargo test -p ploke-eval edit_surface` |
| 11 | `database-context.prompt-evidence` | `queued` | `db.context.*` | [`survey-f`](reports/2026-05-10-database-context.survey-f.jsonl) | `ploke-core`, `ploke-db`, `ploke-tui` | Unify prompt-attached context and UI/database replay joins with typed prompt-evidence carriers. | `cargo test -p ploke-core context && cargo test -p ploke-db context` |
| 12 | `evaluation-oracle-targets.identity-joins` | `queued` | `eval.artifact.branch`, `eval.instance.registry`, `eval.scheduler.state`, `eval.run_record.metadata_setup` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) | `ploke-records`, `ploke-eval` | Unify target identity joins across record, scheduler, and tree layers. | `cargo test -p ploke-eval successor` |
| 13 | `evaluation-oracle-targets.selection-replay` | `queued` | `eval.selection.dto`, `eval.history.payload.selection`, `eval.history.evidence`, `eval.metrics.run`, `eval.run_record.patch_packaging`, `eval.tree.playback` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) | `ploke-records`, `ploke-eval`, `ploke-tree` | Connect typed selection, evidence, metrics, patch packaging, and tree playback into replayable selection traces. | `cargo test -p ploke-eval history && cargo test -p ploke-tree playback` |
| 14 | `runtime-artifact-lineage` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Add a typed lineage projection from any child artifact/runtime back through patch and successor edges to genesis. | `cargo test -p ploke-tree lineage && cargo test -p ploke-eval lineage` |
| 15 | `candidate-frontier.replay` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Preserve and replay complete candidate frontiers, including the final decision candidate set used for successor selection. | `cargo test -p ploke-eval candidate` |
| 16 | `timeline.concurrency` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Add typed span records/projections for parent, child, evaluation, LLM attempt, and tool-call timing so the UI can render concurrency. | `cargo test -p ploke-eval timeline` |
| 17 | `patch.diff-code-graph-impact` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-tree`, `ploke-eval` | Join patch diffs to files, ranges, and code graph items directly modified by each patch. | `cargo test -p ploke-tree patch_impact && cargo test -p ploke-eval patch` |
| 18 | `score.value-locus-analysis` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Compare parent observed value added, patch score effects, shared code graph loci, and repeated/baseline evidence while preserving correlation-vs-causation boundaries. | `cargo test -p ploke-eval score` |
| 19 | `patch.child-composability` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Classify patch composability and child mergeability from lineage, patch impact, shared base artifacts, and typed check results. | `cargo test -p ploke-eval mergeability` |

## Update Rules

Each slice should name the rows it advances in [`ui-drilldown-contract.md`](ui-drilldown-contract.md) and [`traceability-matrix.md`](traceability-matrix.md).

When a slice is started, change exactly one row to `next` unless the current `next` row is still active.

When a slice is completed:

1. Change its status to `done`.
2. Promote the next queued slice to `next`.
3. Update [`typed-data-coverage-report.md`](../typed-data-coverage-report.md).
4. Update [`ui-drilldown-contract.md`](ui-drilldown-contract.md) if the slice changes drilldown coverage, join keys, or verification targets.
5. Update [`traceability-matrix.md`](traceability-matrix.md) if the slice changes typed facts, joins, derived views, or evidence-strength handling.
6. Update the relevant rows in [`inventory.md`](inventory.md) only if compliance or ownership changed.
7. Record verification in the implementation commit or handoff.
