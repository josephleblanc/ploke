# Crate Boundary Inventory

Status: first pass, code-backed as of 2026-05-21.

This view maps the same runtime-playback data by crate boundary. Its purpose
is to keep future implementation work from adding records in the nearest crate
just because that crate happens to observe the fact first.

## Boundary Summary

| Crate | Current role in Prototype 1 playback evidence | Record authority to preserve |
| --- | --- | --- |
| `ploke-eval` | Active loop and current writer boundary for run roots, History admission, transition journal, channel envelopes, run attempts, agent-turn artifacts, replay probes, selection, and eval snapshots. | May create active state and emit records, but durable passive schemas should move or remain in `ploke-records` when shared with graph/playback/UI. |
| `ploke-records` | Passive persisted schema owner for History records, run records, agent-turn records, channel envelopes, llm response sidecars, protocol artifacts, evaluation artifacts, selection metrics, scheduler/node records, run profiles, and ids. | Canonical owner for JSON/JSONL/TOML/GZip shapes that are read outside the writer crate. Avoid smaller mirror DTOs. |
| `ploke-tree` | Record store, typed loaders, graph construction, graph indexes, graph snapshots, playback and read-side projections. | Owns read-side graph facts and borrowed playback views; should not become active-loop authority or parse raw CLI output. |
| `ploke-tui` | Live tool loop, chat/session behavior, tool execution, edit/proposal behavior, indexing/reindexing, DB state, request/response taps under test harness, and workspace DB registry commands. | Owns live behavior and local user config, not the shared persisted eval schema. Eval should record TUI observations through typed artifacts. |
| `ploke-llm` | Provider request/response types, router-specific request fields, model/provider registry and caches, request construction, response parsing, and recorded response tapes. | Owns wire/request/response shapes. Runtime playback needs a records-owned run witness for historical request decisions rather than reading current caches. |
| `ploke-db` / Cozo | Workspace index, parsed code graph storage, backup/snapshot files, validity timestamps, and DB time-travel query substrate. | Runtime playback should carry DB witnesses and timestamps first; DB content loading is an explicit drilldown, not default graph import. |
| `ploke-protocol` | Procedure logic and procedure-specific evidence that may be mirrored as protocol artifacts. | Procedure-owned facts should remain typed; `ploke-records` mirrors should be audited as persisted schema, not open-ended duplicate records. |

## `ploke-eval` Writer Surfaces

Current Prototype 1 run roots are mostly written from `ploke-eval`, especially
`runner.rs`, `record.rs`, `record_emission.rs`, and
`cli/prototype1_state/*`.

| Surface | Typical path | Current shared owner or reader |
| --- | --- | --- |
| run lifecycle artifacts | `execution-log.json`, `repo-state.json`, `record.json.gz` | eval writer shape plus `ploke_records::run_record::RunRecord` read-side shape |
| indexing artifacts | `indexing-status.json`, `parse-failure.json`, `snapshot-status.json` | run artifact refs and run-history helpers |
| DB snapshots | `indexing-checkpoint.db`, `indexing-failure.db`, `final-snapshot.db` | path refs in run records and History run evidence |
| agent-turn artifacts | `agent-turn-trace.json`, `agent-turn-summary.json` | `ploke_records::agent_turn::{AgentTurnTraceRecord, AgentTurnSummaryRecord}` |
| provider response sidecar | `llm-full-responses.jsonl` | `ploke_records::llm_response::RawFullResponseRecord` |
| benchmark submission export | `multi-swe-bench-submission.jsonl` | `ploke_eval::runner::MultiSweBenchSubmissionRecord`; needs records-owned playback witness if graph imports it |
| benchmark patch projection | `benchmark-patch-projection.json` | `ploke_records::evaluation::BenchmarkPatchProjectionRecord` |
| transition journal | `transition-journal.jsonl` | `ploke_records::journal::JournalEntry` through `ploke-tree` loaders |
| runtime channels | `nodes/<node-id>/channels/<runtime-id>/*.jsonl` | live eval channel types and passive `ploke_records::channel` types |
| child plans and invocations | `messages/child-plan/*.json`, `nodes/<node-id>/invocations/*.json` | `ploke_records` child-plan/invocation records loaded by `ploke-tree` |
| evaluation/protocol artifacts | `evaluations/*.json`, `protocol-artifacts/*.json` | `ploke_records::{evaluation, protocol}` |
| History blocks | `history/blocks/segment-*.jsonl` | `ploke_records::history::SealedBlockRecord` |
| campaign and batch anchors | `campaign.json`, `closure-state.json`, `slice.jsonl`, `batch.json`, `batch-run-summary.json` | eval-owned setup/projection records; registry and History remain stronger authorities |
| run registry and latest pointer | `registries/runs/<run-id>.json`, `last-run.json` | `RunRegistration` is lifecycle/discovery authority; `LastRunRecord` is operator convenience |

## `ploke-tui` Observed Surfaces

`ploke-tui` is where the model-facing tools actually run. For playback, that
means TUI facts should be recorded as observations of live behavior, not as a
second authority path.

| TUI surface | Type or module | Current persistence relation | Playback need |
| --- | --- | --- | --- |
| chat session request | `llm::manager::session::ChatSession` over `ChatCompRequest<R>` | request tap currently captures messages in test-harness paths | full provider-request witness if exact replay/debugging requires model/tools/tool-choice/router fields |
| provider response | `RecordedResponse`, parsed response data | eval converts captured responses into `RawFullResponseRecord` lines | join response sidecar to agent turn and request witness |
| tool execution | `tools` and TUI session events | eval records observed turn events into agent-turn artifacts | per-tool request/result/failure steps under the agent-turn timeline |
| edit/proposal lifecycle | TUI proposal/apply surfaces and eval `tui_adapter` | current evidence appears through agent-turn events, patch artifacts, surface evidence, and replay probes | unify as edit lifecycle steps with patch/proposal ids |
| proposal registry | `Vec<EditProposal>` in `proposals.json` | local config path or `PLOKE_PROPOSALS_PATH`; latest checked run had a redirected run-root copy | playback evidence only when the run records the redirected path or proposal ids |
| index/reindex | app-state indexing handlers and database state | eval snapshots DB/status files; TUI also has normal workspace DB save/load paths | DB/index witness plus explicit drilldown into snapshot DB when requested |
| workspace registry | `WorkspaceRegistry`, `WorkspaceRegistryEntry` | `workspaces.toml` under config or `PLOKE_WORKSPACE_REGISTRY_PATH` | outside-run-root; only playback evidence if captured as part of the run |

## `ploke-llm` Observed Surfaces

`ploke-llm` owns provider-facing request and response shapes, but current run
playback does not yet persist the complete request per provider call.

| LLM surface | Type | Persistence relation | Playback need |
| --- | --- | --- | --- |
| common request core | `ChatCompReqCore` | present in older eval run records and request builders | can explain common fields, but not full router request |
| full router request | `ChatCompRequest<R>` | live/session object; not currently a canonical persisted run record | future provider-request record or `AgentTurnArtifactRecord` extension |
| provider response envelope | `RecordedResponse`, `OpenAiResponse` | persisted through `RawFullResponseRecord` in `llm-full-responses.jsonl` | model-exchange response step and replay tape |
| model/endpoint caches | `EndpointCache`, `ModelCache` | local serialized cache files | not historical authority; persist chosen model/provider/endpoint in run evidence |
| route/model selection | router calibration and selected endpoint provenance | eval records selected model/provider/endpoint in run metadata | model/provenance aggregate and cost breakdown |

## Read-Side Direction

The durable direction is:

```text
writer crate emits canonical records
  -> ploke-records owns shared persisted schema
  -> ploke-tree loads typed records into Graph
  -> RuntimePlaybackRef borrows graph facts
  -> CLI, egui, replay probes, and model-facing tools render/query projections
```

When a crate observes a fact before `ploke-records` has a schema for it, the
next design question is whether that fact belongs in an existing owner record,
a new canonical record, or a graph-only derived witness. It should not become a
CLI-only or UI-only DTO.
