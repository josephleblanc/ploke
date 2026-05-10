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

## Queue

| Order | Slice ID | Status | Source surfaces | Source report | Primary crates | Goal | Smallest verification |
|---:|---|---|---|---|---|---|---|
| 1 | `protocol-artifacts.decode` | `next` | `protocol.artifact.decode` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) | `ploke-records` | Replace protocol artifact `serde_json::Value` staging with typed payload decoding. | `cargo test -p ploke-records protocol` |
| 2 | `protocol-artifacts.storage-aggregate` | `queued` | `protocol.artifact.store`, `protocol.artifact.aggregate.output` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) | `ploke-records`, `ploke-eval` | Replace stored raw payload fields and aggregate output cloning with typed payload DTOs. | `cargo test -p ploke-records protocol && cargo test -p ploke-eval protocol` |
| 3 | `tool.call.arguments` | `queued` | `tool.call.record.arguments`, `tool.request.arguments.capture` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) | `ploke-records`, `ploke-eval` | Replace tool-call argument `serde_json::Value` / raw string fields with a closed persisted carrier over `ploke_records::tool_contracts` DTOs plus typed parse-failure records. | `cargo test -p ploke-records --features tool-contracts tool_call && cargo test -p ploke-eval tool` |
| 4 | `tool.result.trace.projection` | `queued` | `tool.response.full_response_trace`, `tool.result.trace.projection` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) | `ploke-eval` | Replace tool-result and turn-trace projection field walking with named typed projection records. | `cargo test -p ploke-eval tool_result` |
| 5 | `llm-attempts.provider-observation-projection` | `queued` | `llm.attempt.provider_error`, `llm.attempt.timeout`, `llm.attempt.timeline` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) | `ploke-eval`, `ploke-llm` | Replace provider error, timeout, and attempt timeline JSON projection with typed records. | `cargo test -p ploke-eval llm_attempt` |
| 6 | `llm-attempts.dto-tool-bridge` | `queued` | `llm.dto.openai_response`, `llm.tool_bridge.records` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) | `ploke-llm` | Remove `serde_json::Value` from owned response metadata/logprobs and tool bridge argument records. | `cargo test -p ploke-llm` |
| 7 | `edit-surface.source-records` | `queued` | `edit_surface.*` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) | `ploke-records`, `ploke-eval`, `ploke-tui` | Decide and encode passive source records for proposal/candidate-local patch evidence without moving live mutation authority into `ploke-records`. | `cargo test -p ploke-records edit_surface && cargo test -p ploke-eval edit_surface` |
| 8 | `database-context.prompt-evidence` | `queued` | `db.context.*` | [`survey-f`](reports/2026-05-10-database-context.survey-f.jsonl) | `ploke-core`, `ploke-db`, `ploke-tui` | Unify prompt-attached context and UI/database replay joins with typed prompt-evidence carriers. | `cargo test -p ploke-core context && cargo test -p ploke-db context` |
| 9 | `evaluation-oracle-targets.identity-joins` | `queued` | `eval.artifact.branch`, `eval.instance.registry`, `eval.scheduler.state`, `eval.run_record.metadata_setup` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) | `ploke-records`, `ploke-eval` | Unify target identity joins across record, scheduler, and tree layers. | `cargo test -p ploke-eval successor` |
| 10 | `evaluation-oracle-targets.selection-replay` | `queued` | `eval.selection.dto`, `eval.history.payload.selection`, `eval.history.evidence`, `eval.metrics.run`, `eval.run_record.patch_packaging`, `eval.tree.playback` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) | `ploke-records`, `ploke-eval`, `ploke-tree` | Connect typed selection, evidence, metrics, patch packaging, and tree playback into replayable selection traces. | `cargo test -p ploke-eval history && cargo test -p ploke-tree playback` |
| 11 | `runtime-artifact-lineage` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Add a typed lineage projection from any child artifact/runtime back through patch and successor edges to genesis. | `cargo test -p ploke-tree lineage && cargo test -p ploke-eval lineage` |
| 12 | `candidate-frontier.replay` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Preserve and replay complete candidate frontiers, including the final decision candidate set used for successor selection. | `cargo test -p ploke-eval candidate` |
| 13 | `timeline.concurrency` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Add typed span records/projections for parent, child, evaluation, LLM attempt, and tool-call timing so the UI can render concurrency. | `cargo test -p ploke-eval timeline` |
| 14 | `patch.diff-code-graph-impact` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-tree`, `ploke-eval` | Join patch diffs to files, ranges, and code graph items directly modified by each patch. | `cargo test -p ploke-tree patch_impact && cargo test -p ploke-eval patch` |
| 15 | `score.value-locus-analysis` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Compare parent observed value added, patch score effects, shared code graph loci, and repeated/baseline evidence while preserving correlation-vs-causation boundaries. | `cargo test -p ploke-eval score` |
| 16 | `patch.child-composability` | `queued` | UI contract derived row | [`ui-drilldown-contract`](ui-drilldown-contract.md) | `ploke-records`, `ploke-eval`, `ploke-tree` | Classify patch composability and child mergeability from lineage, patch impact, shared base artifacts, and typed check results. | `cargo test -p ploke-eval mergeability` |

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
