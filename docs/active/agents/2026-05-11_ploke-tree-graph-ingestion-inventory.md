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

- `RunRecordSet.forest_input.passive_evidence.run_profile`

## Family Rollup

| Family | Current graph ingestion | Notes |
|---|---|---|
| `protocol-artifacts` | partial `evidence` | Parsed protocol artifacts attach by path and protocol coordinate. Decode/store implementation surfaces are not separate core graph authority. |
| `tool-calls-results` | `not-loaded` | Tool calls/results should attach to runtime turns, operations, provider attempts, and patch/evaluation evidence after typed run-record loaders exist. Agent-turn records are blocked by an unresolved nested tool UI payload shape. |
| `monitor-projections` | partial `evidence` / `not-loaded` | Scheduler/node/status records are now attached as secondary evidence. Preview/slice/log projections should not become authority. |
| `edit-surface-patch-evidence` | partial `core` / `evidence` | History selection payloads provide candidate artifact, surface, patch, and selection evidence. Proposal registry and live grant/check surfaces are deferred. |
| `llm-attempts` | `not-loaded` | Provider attempts, timeouts, retries, and full response logs should attach as operation/runtime-turn evidence later. |
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
| `tool.response.full_response_trace` | `not-loaded` | Attach provider response evidence through typed trace records, not raw logs. |
| `tool.result.trace.projection` | `not-loaded` | Projection should be derived from typed tool evidence or attached as weak evidence. |
| `prototype1.scheduler_json` | `evidence` | Loaded by `RunForestInput`; graph uses it for labels/status evidence only, not ordering authority. |
| `prototype1.node_request_projection` | `not-loaded` | Candidate/runtime request evidence, if typed, should attach to runtime or operation. |
| `prototype1.runner_result_projection` | `not-loaded` | Candidate/runtime result evidence, if typed, should attach to runtime or operation. |
| `prototype1.metrics_projection` | `not-loaded` | Attach metrics as evaluation/candidate evidence after typed loader exists. |
| `prototype1.history_preview_document` | `projection-target` | Should be derived from sealed History / graph, not used to build graph. |
| `prototype1.history_preview_slice` | `projection-target` | Should be derived from sealed History / graph, not used to build graph. |
| `prototype1.agent_turn_trace` | `blocked` | Passive owner deferred: current tool UI payload can include arbitrary nested LLM retry JSON. Needs a typed tool UI/error boundary before graph loading. |
| `prototype1.observation_jsonl` | `not-loaded` | Attach as weak typed observation evidence only when it can be joined safely. |
| `prototype1.slice_jsonl` | `projection-target` | Projection/debug surface; should not create graph authority. |
| `edit_surface.grant_check` | `defer` | Live grant/check authority stays outside graph unless mirrored by passive evidence. |
| `edit_surface.checked_surface_evidence` | partial `core` | Surface evidence inside History selection payloads contributes patch/artifact surface facts. |
| `edit_surface.surface_evidence_record` | partial `core` | Candidate payload surface evidence supplies `artifact_after` / `patch_id` when present. |
| `edit_surface.surface_attempt_record` | partial `evidence` | Present inside candidate payloads but not yet separately indexed. |
| `edit_surface.candidate_artifact_record` | `core` | Current graph extracts candidate artifact, patch, branch, and surface-derived artifact facts. |
| `edit_surface.surface_commitment_record` | `core` | History block surface commitment is indexed on `HistoryBlockNode`. |
| `edit_surface.parent_identity_record` | `evidence` | Loaded by `RunForestInput`; graph attaches parent identity evidence without treating it as History authority. |
| `edit_surface.invocation_record` | partial `evidence` | Successor ready/completion records are loaded and attached to handoff/runtime evidence. Full invocation files remain a loader gap. |
| `edit_surface.proposal_registry` | `defer` | TUI-local proposal state should enter graph only through passive proposal/patch evidence. |
| `edit_surface.patch_artifact` | `not-loaded` | Patch artifact snapshots should attach to PatchAttempt/Patch or candidate artifact evidence. |
| `llm.attempt.request` | `not-loaded` | Attach to provider attempt / runtime turn evidence. |
| `llm.attempt.response` | `not-loaded` | Attach to provider attempt / runtime turn evidence. |
| `llm.attempt.provider_error` | `not-loaded` | Attach to failed provider attempt evidence. |
| `llm.attempt.timeout` | `not-loaded` | Attach to timeout/retry evidence, not scheduler status. |
| `llm.attempt.timeline` | `not-loaded` | Attach as typed provider attempt timeline once available. |
| `llm.attempt.full_response_logs` | `not-loaded` | Attach via typed full-response records, not raw log parsing. |
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
| `eval.run_profile` | `loaded-not-ingested` | `run-profile.toml` and `run-profile.commitment.json` now have passive records and store loading; graph attachment is still pending. |
| `eval.run_record.metadata_setup` | `not-loaded` | Attach to run/campaign/runtime evidence once run records are loaded. |
| `eval.run_record.patch_packaging` | `not-loaded` | Attach to patch/MBE submission evidence; patch bytes come from packaging diff artifacts. |
| `eval.tree.playback` | `projection-target` | Playback should be derived from sealed History / graph, not treated as graph source. |

## Immediate Graph Gaps

1. `RunRecordSet` now loads run profile records, but `Graph` does not yet
   attach them as reproducibility metadata/evidence.
2. Tool calls/results and agent-turn files are not loaded into `RunRecordSet`.
   Agent-turn passive ownership is blocked on the nested tool UI/error payload
   shape.
3. Full invocation, runner request, and runner result files have passive owners
   but are not yet loaded as individual attempt evidence.
4. Patch artifact snapshots and MBE packaging evidence are not loaded into
   `RunRecordSet`.
5. Provider attempts/retries/timeouts are not loaded into `RunRecordSet`.
6. Database context/prompt evidence is not loaded into `RunRecordSet`.
7. `crates/ploke-tree/src/lib.rs` still owns too much store loading and should
   be split into `store/**` before more loader families are added.
8. `crates/ploke-records/src/history/payload.rs` is now large enough that the
   next surface/request-policy addition should split payload submodules first.

## Next Update Rule

When `Graph::from_records` starts consuming a new family, update this file in
the same change. The update should answer:

- which typed record/loader is the input,
- whether it creates a core relation or evidence attachment,
- what graph object it joins to,
- what ambiguity is preserved when the join fails.
