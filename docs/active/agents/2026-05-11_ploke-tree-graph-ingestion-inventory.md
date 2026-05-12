# ploke-tree Graph Ingestion Inventory

Date: 2026-05-11

Purpose: companion tracker for
[`typed-persistence-spine/inventory.md`](../plans/self-improvement-loop/typed-persistence-spine/inventory.md).
That inventory tracks typed-persistence compliance. This document tracks whether
those accepted source families are ingested into `ploke-tree::graph::Graph`.

This is not a second compliance inventory and not a request for one graph node
per persisted file. The graph should stay small: sealed History and stable loop
entities form the core graph; the many persisted record families attach as
typed evidence when they can be joined without guessing.

Policy update: every persisted loop-relevant surface belongs in the graph
boundary eventually. The question is not whether to include it, but how to
represent it: core relation, evidence attachment, metadata, locator, digest,
diagnostic, secondary ordering, or explicit ambiguity. Projection/debug/log
surfaces still must not become History authority.

## Status Legend

- `core`: contributes to graph identity, ordering, or semantic relations.
- `evidence`: attached through `EvidenceAttachment` / `EvidenceLocator`.
- `loaded-not-ingested`: typed records are loaded by `ploke-tree`, but
  `Graph::from_records` does not currently index them.
- `implemented-under-review`: implementation reports say typed records/loaders
  exist, but reviewer findings still block acceptance.
- `not-loaded`: no `RunRecordSet` / graph input path exists yet.
- `projection-target`: derived projection over graph/loaders, not a source for
  graph construction.
- `defer`: live-runtime or crate-local surface that should become graph input
  only through a passive persisted record or typed evidence locator.

## Current Code Boundary

Current graph construction starts at:

- `crates/ploke-tree/src/graph/build.rs`
- `crates/ploke-tree/src/graph/build/history.rs`
- `crates/ploke-tree/src/graph/build/selection.rs`
- `crates/ploke-tree/src/graph/build/selection/membership.rs`
- `crates/ploke-tree/src/graph/build/passive.rs`
- `crates/ploke-tree/src/graph/build/{branch,channel,handoff,journal,scheduler}.rs`
- `crates/ploke-tree/src/graph/types.rs`
- `crates/ploke-tree/src/graph/types/**`

Current inputs consumed by `Graph::from_records(&RunRecordSet)`:

- `RunRecordSet.history_blocks`
- `RunRecordSet.forest_input.scheduler`
- `RunRecordSet.forest_input.node_records`
- `RunRecordSet.forest_input.parent_identity`
- `RunRecordSet.forest_input.successor_ready`
- `RunRecordSet.forest_input.successor_completion`
- `RunRecordSet.transition_journal`
- `RunRecordSet.forest_input.passive_evidence.evaluations`
- `RunRecordSet.forest_input.passive_evidence.protocol_artifacts`
- passive evidence summaries for branch registry, transition journal, History
  storage, channel envelopes, and child-plan messages

Current loaded inputs not yet consumed by `Graph::from_records`:

- none identified for the agent-turn family. Agent-turn trace/summary evidence
  is loaded into `RunRecordSet` and attached by `Graph::from_records` as
  passive evidence, but its dependency boundary remains unresolved.

## Family Rollup

| Family | Current graph ingestion | Notes |
|---|---|---|
| `protocol-artifacts` | partial `evidence` | Parsed protocol artifacts attach by path and protocol coordinate. Decode/store implementation surfaces are not separate core graph authority. |
| `tool-calls-results` | partial `evidence` / `not-loaded` | Agent-turn trace/summary records have typed `ploke_records::agent_turn` ownership, direct run-root `RunRecordSet` loading, and passive graph evidence attachment. Remaining gap: `ploke-tree` reaches the passive reader through `ploke-records/tool-contracts`, which pulls in canonical tool DTOs from `ploke-tui`. Do not resolve this by copying tool DTOs into `ploke-records`; move passive transport DTOs once to a shared canonical home before further cleanup. Tool call/result operation nodes remain pending. |
| `monitor-projections` | partial `evidence` / `not-loaded` | Scheduler/node/status records are now attached as secondary evidence. Preview/slice/log projections should not become authority. |
| `edit-surface-patch-evidence` | partial `core` / `evidence` | History selection payloads provide candidate artifact, surface, patch, and selection evidence. Proposal registry and live grant/check surfaces are deferred. |
| `llm-attempts` | `not-loaded` | Provider attempts, timeouts, retries, and full response logs are not covered by the agent-turn record family. They need a separate passive owner before `ploke-tree` can load or ingest them. |
| `database-context` | `not-loaded` | Prompt/context evidence should attach to runtime turns, tool calls, or operations after a typed evidence carrier exists. |
| `evaluation-oracle-targets` | partial `core` / `evidence` | History selection payloads and evaluation artifacts are currently the strongest covered areas. Scheduler/run-record packaging surfaces are not yet graph inputs. |

## Surface Tracker

| Surface ID | Graph status | Intended graph treatment |
|---|---|---|
| `protocol.artifact.decode` | `defer` | Parser/type work belongs in `ploke-records`; graph sees successfully loaded protocol artifacts. |
| `protocol.artifact.test.roundtrip` | `projection-target` | Test fixture surface, not graph input. |
| `protocol.artifact.store` | `defer` | Storage shape belongs to typed records/loaders; graph should attach parsed artifacts by typed locator. |
| `protocol.artifact.aggregate.output` | `not-loaded` | If persisted as a passive artifact, attach as protocol/evaluation evidence. |
| `protocol.artifact.playback` | `evidence` | Current graph attaches protocol artifact locators by path and protocol coordinate. |
| `tool.call.record.arguments` | `not-loaded` | Attach to tool call nodes/evidence once turn/tool records are loaded. |
| `tool.request.arguments.capture` | `not-loaded` | Attach to outbound tool request evidence, not to History authority. |
| `tool.execution.record` | `not-loaded` | Attach request/result pairs to runtime turn or operation evidence. |
| `tool.response.full_response_trace` | `not-loaded` | Attach provider response evidence through typed trace records, not raw logs. Current agent-turn work does not cover run-local full-response sidecars. |
| `tool.result.trace.projection` | `not-loaded` | Projection should be derived from typed tool evidence or attached as weak evidence. |
| `prototype1.scheduler_json` | `evidence` | Loaded by `RunForestInput`; graph uses it for labels/status evidence only, not ordering authority. |
| `prototype1.node_request_projection` | `evidence` | Runner request evidence is loaded through `PassiveEvidence.run_attempts` and attaches to artifact/runtime/operation evidence without becoming History authority. |
| `prototype1.runner_result_projection` | `evidence` | Latest and attempt-scoped runner results are loaded through `PassiveEvidence.run_attempts` / `attempt_runner_results` and attach to branch/artifact/operation evidence. |
| `prototype1.metrics_projection` | `not-loaded` | Attach metrics as evaluation/candidate evidence after typed loader exists. |
| `prototype1.history_preview_document` | `projection-target` | Should be derived from sealed History / graph, not used to build graph. |
| `prototype1.history_preview_slice` | `projection-target` | Should be derived from sealed History / graph, not used to build graph. |
| `prototype1.agent_turn_trace` | `evidence` | Passive owner is `ploke_records::agent_turn`; `ploke-tree` loads direct run-root trace/summary files into `RunRecordSet`; `Graph::from_records` attaches them as passive evidence. Dependency cleanup remains unresolved because the canonical tool DTOs currently live in `ploke-tui`. |
| `prototype1.observation_jsonl` | `not-loaded` | Attach as weak typed observation evidence only when it can be joined safely. |
| `prototype1.slice_jsonl` | `projection-target` | Projection/debug surface; should not create graph authority. |
| `edit_surface.grant_check` | `defer` | Live grant/check authority stays outside graph unless mirrored by passive evidence. |
| `edit_surface.checked_surface_evidence` | partial `core` | Surface evidence inside History selection payloads contributes patch/artifact surface facts. |
| `edit_surface.surface_evidence_record` | partial `core` | Candidate payload surface evidence supplies `artifact_after` / `patch_id` when present. |
| `edit_surface.surface_attempt_record` | partial `evidence` | Present inside candidate payloads but not yet separately indexed. |
| `edit_surface.candidate_artifact_record` | `core` | Current graph extracts candidate artifact, patch, branch, and surface-derived artifact facts. |
| `edit_surface.surface_commitment_record` | `core` | History block surface commitment is indexed on `HistoryBlockNode`. |
| `edit_surface.parent_identity_record` | `evidence` | Loaded by `RunForestInput`; graph attaches parent identity evidence without treating it as History authority. |
| `edit_surface.invocation_record` | partial `evidence` | Successor ready/completion records and full invocation files are loaded and attached as handoff/runtime/operation evidence. |
| `edit_surface.proposal_registry` | `defer` | TUI-local proposal state should enter graph only through passive proposal/patch evidence. |
| `edit_surface.patch_artifact` | `not-loaded` | Patch artifact snapshots should attach to PatchAttempt/Patch or candidate artifact evidence. |
| `llm.attempt.request` | `not-loaded` | Attach to provider attempt / runtime turn evidence. |
| `llm.attempt.response` | `not-loaded` | Attach to provider attempt / runtime turn evidence. |
| `llm.attempt.provider_error` | `not-loaded` | Attach to failed provider attempt evidence. |
| `llm.attempt.timeout` | `not-loaded` | Attach to timeout/retry evidence, not scheduler status. |
| `llm.attempt.timeline` | `not-loaded` | Attach as typed provider attempt timeline once available. |
| `llm.attempt.full_response_logs` | `not-loaded` | Attach via typed full-response records, not raw log parsing. `llm-full-responses.jsonl` still needs its own typed passive record/loader. |
| `llm.dto.openai_response` | `defer` | DTO cleanup belongs in `ploke-llm`; graph consumes typed attempt/response evidence. |
| `llm.tool_bridge.records` | `defer` | Bridge DTOs should reach graph through typed tool/provider evidence. |
| `db.context.intent_and_tool_result` | `not-loaded` | Attach to tool call / prompt / operation evidence after typed carrier exists. |
| `db.context.assembly` | `not-loaded` | Attach assembled context to prompt/runtime turn evidence. |
| `db.context.embedding_and_node_refs` | `not-loaded` | Attach cited DB/code nodes as context evidence. |
| `db.context.ui_projection` | `projection-target` | UI projection should derive from graph/context evidence, not build graph. |
| `eval.artifact.branch` | `evidence` | Current graph attaches evaluation artifacts to branch evidence by branch id. |
| `eval.metrics.run` | partial `evidence` | Metrics are present inside evaluation artifacts but not separately indexed. |
| `eval.selection.dto` | `core` | Selection decisions are indexed from sealed History payloads. |
| `eval.history.payload.selection` | `core` | Current graph indexes selection, candidate, membership, patch, and artifact facts. |
| `eval.history.evidence` | `core` / `evidence` | History block/entry refs and custody evidence are indexed. |
| `eval.instance.registry` | `not-loaded` | Attach as evaluation/campaign context if a typed loader becomes graph input. |
| `eval.scheduler.state` | `evidence` | Scheduler state is loaded and attached as secondary label/status evidence. |
| `eval.child_plan.message` | `evidence` | `messages/child-plan/*.json` now has a passive record owner, store loader, and summary-only graph evidence. It does not create child branch/runtime authority. |
| `eval.run_profile` | `evidence` | `run-profile.toml` and `run-profile.commitment.json` now have passive records, store loading, and graph metadata evidence. They do not create lineage/runtime/artifact authority. |
| `eval.run_record.metadata_setup` | `not-loaded` | Attach to run/campaign/runtime evidence once run records are loaded. |
| `eval.run_record.patch_packaging` | `not-loaded` | Attach to patch/MBE submission evidence; patch bytes come from packaging diff artifacts. |
| `eval.tree.playback` | `projection-target` | Playback should be derived from sealed History / graph, not treated as graph source. |

## Immediate Graph Gaps

1. Agent-turn trace/summary files now have typed ownership in
   `ploke_records::agent_turn`, direct run-root `RunRecordSet` loading in
   `ploke-tree`, and passive `Graph::from_records` evidence attachment.
   Remaining cleanup is dependency architecture, not ingestion: passive
   tool transport DTOs still live under `ploke-tui` and are re-exported through
   `ploke-records/tool-contracts`. Do not copy or mirror `ToolName`,
   `ToolUiPayload`, `ToolErrorWire`, or related tool DTOs into
   `ploke-records`.
2. Patch artifact snapshots and MBE packaging evidence are not loaded into
   `RunRecordSet`.
3. Provider attempts/retries/timeouts are not loaded into `RunRecordSet`.
   Provider/full-response sidecars are outside the agent-turn family and need a
   separate passive record owner before graph ingestion.
4. Tool call/result operation nodes remain pending. The current agent-turn graph
   ingestion attaches artifact-level passive evidence only.
5. Database context/prompt evidence is not loaded into `RunRecordSet`.
6. `crates/ploke-tree/src/lib.rs` still owns store-focused tests and forest /
   browser projection assembly. `FsRunStore` itself has moved to `store/fs.rs`.
7. `crates/ploke-records/src/history/payload.rs` is now large enough that the
   next surface/request-policy addition should split payload submodules first.
8. Internal fine playback now preserves set-scoped membership identity. Future
   UI views should borrow membership facts from `ploke-tree::Graph` instead of
   treating compatibility fields as authority.

## Next Update Rule

When `Graph::from_records` starts consuming a new family, update this file in
the same change. The update should answer:

- which typed record/loader is the input,
- whether it creates a core relation or evidence attachment,
- what graph object it joins to,
- what ambiguity is preserved when the join fails.
