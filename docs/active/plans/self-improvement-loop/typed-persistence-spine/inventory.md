# Typed Persistence Inventory

Updated: 2026-05-10

Accepted survey rows live in [`inventory.jsonl`](inventory.jsonl). Rows are accepted only after the corresponding sub-agent report validates with `jq` and the main thread spot-checks cited source lines.

## Rollup

| Surface ID | Family | Compliance | Desired type home | Report |
|---|---|---|---|---|
| `protocol.artifact.decode` | `protocol-artifacts` | `value-staging`, `anonymous field-walking` | `ploke-records` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) |
| `protocol.artifact.test.roundtrip` | `protocol-artifacts` | typed/test fixture | `test-only` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) |
| `protocol.artifact.store` | `protocol-artifacts` | `value-fields`, `field-walking` | `ploke-records` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) |
| `protocol.artifact.aggregate.output` | `protocol-artifacts` | typed | `ploke-eval` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) |
| `protocol.artifact.playback` | `protocol-artifacts` | typed | `ploke-tree` | [`survey-a`](reports/2026-05-10-protocol-artifacts.survey-a.jsonl) |
| `tool.call.record.arguments` | `tool-calls-results` | typed | `ploke-records` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) |
| `tool.request.arguments.capture` | `tool-calls-results` | typed | `ploke-eval` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) |
| `tool.execution.record` | `tool-calls-results` | typed | `ploke-eval` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) |
| `tool.response.full_response_trace` | `tool-calls-results` | `typed-reader-missing` | `ploke-eval` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) |
| `tool.result.trace.projection` | `tool-calls-results` | typed | `ploke-eval` | [`survey-b`](reports/2026-05-10-tool-calls-results.survey-b.jsonl) |
| `prototype1.scheduler_json` | `monitor-projections` | typed | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.node_request_projection` | `monitor-projections` | typed | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.runner_result_projection` | `monitor-projections` | typed | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.metrics_projection` | `monitor-projections` | typed | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.history_preview_document` | `monitor-projections` | `value-field`, `projection-needs-type` | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.history_preview_slice` | `monitor-projections` | `value-field`, `value-staging`, `ad-hoc-reader`, `projection-needs-type` | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.agent_turn_trace` | `monitor-projections` | typed | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.observation_jsonl` | `monitor-projections` | typed | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `prototype1.slice_jsonl` | `monitor-projections` | `value-field`, `ad-hoc-reader`, `projection-needs-type` | `ploke-eval` | [`survey-c-v2`](reports/2026-05-10-monitor-projections.survey-c-v2.jsonl) |
| `edit_surface.grant_check` | `edit-surface-patch-evidence` | typed | `ploke-eval` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.checked_surface_evidence` | `edit-surface-patch-evidence` | typed | `ploke-eval` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.surface_evidence_record` | `edit-surface-patch-evidence` | typed | `ploke-records` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.surface_attempt_record` | `edit-surface-patch-evidence` | typed | `ploke-records` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.candidate_artifact_record` | `edit-surface-patch-evidence` | typed | `ploke-records` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.surface_commitment_record` | `edit-surface-patch-evidence` | typed | `ploke-records` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.parent_identity_record` | `edit-surface-patch-evidence` | typed | `ploke-records` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.invocation_record` | `edit-surface-patch-evidence` | typed | `ploke-records` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.proposal_registry` | `edit-surface-patch-evidence` | typed | `ploke-tui` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `edit_surface.patch_artifact` | `edit-surface-patch-evidence` | typed | `ploke-eval` | [`survey-d-v2`](reports/2026-05-10-edit-surface-patch-evidence.survey-d-v2.jsonl) |
| `llm.attempt.request` | `llm-attempts` | typed | `ploke-llm` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `llm.attempt.response` | `llm-attempts` | typed | `ploke-llm` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `llm.attempt.provider_error` | `llm-attempts` | `value-field`, `ad-hoc-reader`, `projection-needs-type` | `ploke-eval` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `llm.attempt.timeout` | `llm-attempts` | `value-field`, `projection-needs-type` | `ploke-eval` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `llm.attempt.timeline` | `llm-attempts` | `stringly-json trace payload`, `projection-needs-type` | `ploke-llm` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `llm.attempt.full_response_logs` | `llm-attempts` | typed | `ploke-eval` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `llm.dto.openai_response` | `llm-attempts` | `serde_json::Value logprobs`, `HashMap<String, serde_json::Value> metadata` | `ploke-llm` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `llm.tool_bridge.records` | `llm-attempts` | `value-field`, `stringly-json` | `ploke-llm` | [`survey-e`](reports/2026-05-10-llm-attempts.survey-e.jsonl) |
| `db.context.intent_and_tool_result` | `database-context` | typed | `ploke-core` | [`survey-f`](reports/2026-05-10-database-context.survey-f.jsonl) |
| `db.context.assembly` | `database-context` | typed | `ploke-core` | [`survey-f`](reports/2026-05-10-database-context.survey-f.jsonl) |
| `db.context.embedding_and_node_refs` | `database-context` | typed | `ploke-core / ploke-db` | [`survey-f`](reports/2026-05-10-database-context.survey-f.jsonl) |
| `db.context.ui_projection` | `database-context` | typed | `ploke-tui` | [`survey-f`](reports/2026-05-10-database-context.survey-f.jsonl) |
| `eval.artifact.branch` | `evaluation-oracle-targets` | typed | `ploke-records` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.metrics.run` | `evaluation-oracle-targets` | typed | `ploke-records` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.selection.dto` | `evaluation-oracle-targets` | typed | `ploke-records` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.history.payload.selection` | `evaluation-oracle-targets` | typed | `ploke-records` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.history.evidence` | `evaluation-oracle-targets` | typed | `ploke-records` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.instance.registry` | `evaluation-oracle-targets` | typed | `ploke-eval` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.scheduler.state` | `evaluation-oracle-targets` | typed | `ploke-eval` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.run_record.metadata_setup` | `evaluation-oracle-targets` | typed | `ploke-eval` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.run_record.patch_packaging` | `evaluation-oracle-targets` | typed | `ploke-eval` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |
| `eval.tree.playback` | `evaluation-oracle-targets` | typed | `ploke-tree` | [`survey-g`](reports/2026-05-10-evaluation-oracle-targets.survey-g.jsonl) |

## Current Counts

| Family | Accepted surfaces | Non-compliant surfaces | Notes |
|---|---:|---:|---|
| `protocol-artifacts` | 5 | 3 | Decode/store/aggregate still stage or retain `serde_json::Value`; tree playback is typed. |
| `tool-calls-results` | 5 | 4 | Tool arguments and trace projections still need typed carriers/readers. |
| `monitor-projections` | 9 | 5 | Scheduler/node/result/metrics projections are typed; preview/log/dataset readers still need typed surfaces. |
| `edit-surface-patch-evidence` | 10 | 0 | Typed surfaces exist; remaining work is ownership splits and replay joins. |
| `llm-attempts` | 8 | 5 | Request/response are typed; provider observation, DTO metadata/logprobs, and tool bridge records still need typed surfaces. |
| `database-context` | 4 | 0 | Typed surfaces exist; remaining work is shared prompt-evidence and replay joins. |
| `evaluation-oracle-targets` | 10 | 0 | Typed DTOs exist; target identity and selection/evidence joins remain scattered. |
