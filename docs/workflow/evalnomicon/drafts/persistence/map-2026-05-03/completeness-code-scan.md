# Prototype 1 Persistence Completeness Code Scan

Date: 2026-05-03

Scope: read-only code scan for persistence writers/readers that may sit outside the five evidence-family reports. Focused on `crates/ploke-eval`, with adjacent crates noted only where they feed eval records or create likely false-positive persistence surfaces. I did not inspect persisted artifact contents.

## Scan Inputs

Code search covered these persistence surfaces:

- JSON/JSONL/gzip writers and readers: `File::create`, `OpenOptions`, `serde_json::to_writer`, `serde_json::to_vec_pretty`, `serde_json::to_string_pretty`, `write_all`, `fs::write`, `fs::read_to_string`.
- Filenames and concepts: `record.json`, `record.json.gz`, `journal`, `artifact`, `sidecar`, `state.json`, `repo-state.json`, `closure-state.json`, `scheduler.json`, `runner-request.json`, `runner-result.json`, `llm-full-responses.jsonl`.
- CLI readers: monitor/report/history/metrics/preview paths, run-record resolution, sidecar response readers.

## Additional Loop-Run Persistence Surfaces

These appear directly included in Prototype 1 loop runs or in the loop controller's live observation surface.

| Surface | Path shape | Candidate types | Writers | Readers / projections | Inclusion |
| --- | --- | --- | --- | --- | --- |
| Scheduler state | `<campaign>/prototype1/scheduler.json` | `Prototype1SchedulerState`, `Prototype1NodeRecord`, `Prototype1ContinuationDecision` | `save_scheduler_state`, `register_root_parent_node`, `register_treatment_evaluation_node`, `update_node_status`, `record_continuation_decision` in `crates/ploke-eval/src/intervention/scheduler.rs` | `load_or_default_scheduler_state`, `load_scheduler_state`; monitor report, history preview, metrics | In loop; mutable JSON overwritten as frontier/status changes. |
| Node mirror | `<campaign>/prototype1/nodes/<node-id>/node.json` | `Prototype1NodeRecord`, `Prototype1NodeStatus` | `save_node_record` via node registration/status/workspace updates | `load_node_record`; history preview imports as `NodeRecord`; monitor lists/describes | In loop; scheduler-owned mirror, not transition authority by itself. |
| Runner request | `<campaign>/prototype1/nodes/<node-id>/runner-request.json` | `Prototype1RunnerRequest` | `save_runner_request` via node registration and workspace updates | `load_runner_request`; history preview imports as `RunnerRequest`; child runner consumes | In loop; mutable configuration for node runtime. |
| Latest runner result | `<campaign>/prototype1/nodes/<node-id>/runner-result.json` | `Prototype1RunnerResult`, `Prototype1RunnerDisposition` | `record_runner_result`, `save_runner_result`, `write_runner_result_at` | `load_runner_result`, `load_runner_result_at`; parent reads back after child process; monitor/history preview | In loop; latest node result, may be overwritten or cleared. |
| Attempt runner result | `<campaign>/prototype1/nodes/<node-id>/results/<runtime-id>.json` | `Prototype1RunnerResult` | `record_attempt_runner_result` writes attempt result before updating latest result | History preview imports as `AttemptResult`; monitor locations list it | In loop; more complete than latest result because it is attempt-scoped. |
| Runtime invocation | `<campaign>/prototype1/nodes/<node-id>/invocations/<runtime-id>.json` | `Invocation`, `ChildInvocation`, `SuccessorInvocation`, `InvocationAuthority` | `write_child_invocation`, `write_successor_invocation_for_retired_parent` through `write_invocation` | `invocation::load`, child/successor launch argv, history preview imports as `Invocation` | In loop; attempt-scoped authority token for spawned runtime. |
| Successor ready ack | `<campaign>/prototype1/nodes/<node-id>/successor-ready/<runtime-id>.json` | `SuccessorReadyRecord` | `write_successor_ready_record` | `load_successor_ready_record`; parent wait/handoff; history preview imports | In loop for successor handoff. |
| Successor completion | `<campaign>/prototype1/nodes/<node-id>/successor-completion/<runtime-id>.json` | `SuccessorCompletionRecord`, `SuccessorCompletionStatus` | `write_successor_completion_record` | monitor/history preview; successor completion reporting | In loop for successor rehydration result. |
| Transition journal | `<campaign>/prototype1/transition-journal.jsonl` | `JournalEntry` plus `Entry`, `BuildEntry`, `SpawnEntry`, `ReadyEntry`, `CompletionEntry`, `ParentStartedEntry`, `resource::Sample`, `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, `SuccessorHandoffEntry`, `child::Record`, `successor::Record` | `PrototypeJournal::append` uses `OpenOptions::append` and `write_all`; many call sites in `prototype1_process`, `c1`-`c4`, `child`, `successor`, `cli_facing` | `PrototypeJournal::load_entries`; `prototype1-monitor report`; `history-preview`; `history-metrics` | In loop; append-only evidence, separate from sealed History. |
| History block store | `<campaign>/prototype1/history/blocks/segment-000000.jsonl`, `<campaign>/prototype1/history/index/by-hash.jsonl`, `<campaign>/prototype1/history/index/by-lineage-height.jsonl`, `<campaign>/prototype1/history/index/heads.json` | `FsBlockStore`, `Block<Sealed>`, `StoredBlock`, `LineageHeight`, `LineageState` | `FsBlockStore::append` appends JSONL indexes and writes `heads.json`; live handoff calls `history_store.append` before successor spawn | `FsBlockStore::lineage_state`, `read_heads`, `stored_record_by_hash`, History commands | In loop for successor handoff; authority-bearing relative to current implementation, but still local prototype store. |
| Parent identity artifact | `<repo>/.ploke/prototype1/parent_identity.json` | `ParentIdentity` | `write_parent_identity` during setup, child artifact persistence, successor/parent transitions | `load_parent_identity`, `load_parent_identity_optional`; active parent validation/monitor | In loop and committed into Artifact checkouts; not under campaign `prototype1/` root. |
| Branch registry | `<campaign>/prototype1/branches.json` | `Prototype1BranchRegistry`, branch records, latest evaluation summary | `save_branch_registry`; synthesis/apply/evaluation update paths | `load_or_default_branch_registry`; monitor report/history preview/loop selection | In loop; mutable JSON, important for branch selection and evaluation summary. |
| Branch evaluation reports | `<campaign>/prototype1/evaluations/<branch-id>.json` | `Prototype1BranchEvaluationReport`, `Prototype1LoopBranchEvaluationSummary` | `write_json_file_pretty` in `run_prototype1_branch_evaluation` | `load_prototype1_branch_evaluation_report`; monitor report/history preview; runner result links to path | In loop; per-branch JSON evaluation artifact. |
| Legacy loop trace | `<campaign>/prototype1-loop-trace.json` | `Prototype1StateReport` | `write_json_file_pretty` in `run_prototype1_state` path | monitor location/report surfaces | In loop/controller projection; mutable JSON trace/report, not authority. |
| Child/successor streams | `<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stdout.log`, `stderr.log` | `Streams` | `open_runtime_streams` in `prototype1_process`; `open_child_streams` in `c3` | monitor peek/watch and failure excerpts | In loop; logs are persisted but should be treated as diagnostic, not structured records. |

## Eval Run Records Feeding Prototype 1

These are not unique to Prototype 1, but Prototype 1 branch evaluation invokes the normal eval runner and then references its records.

| Surface | Path shape | Candidate types | Writers | Readers / projections | Inclusion |
| --- | --- | --- | --- | --- | --- |
| Compressed run record | `<run>/record.json.gz` | `RunRecord`, `RunRecordBuilder`, `RunArtifactRefs` | `write_compressed_record` via `runner.rs` | `read_compressed_record`; many CLI subcommands; monitor run artifact loader | Included through branch evaluation/treatment runs. |
| Setup/phase sidecars | `<run>/repo-state.json`, `execution-log.json`, `indexing-status.json`, `parse-failure.json`, `snapshot-status.json` | `RepoStateArtifact`, `ExecutionLogArtifact`, `IndexingStatusArtifact`, `ParseFailureArtifact`, snapshot status types | `write_json` in `runner.rs` | `closure.rs`, `run_history.rs`, CLI inspection, run registry status | Included for baseline/treatment eval runs; sidecars may exist even if `record.json.gz` fails later. |
| Agent turn trace/summary | `<run>/agent-turn-trace.json`, `<run>/agent-turn-summary.json` | `AgentTurnArtifact`, `ObservedTurnEvent`, `Tool*Record`, `TurnFinishedRecord` | `run_benchmark_turn` repeatedly overwrites trace; summary written after turn | run registry refs; monitor run artifact loader indirectly | Included for agent-mode treatment runs. |
| Raw full response sidecar | `<run>/llm-full-responses.jsonl` | `RawFullResponseRecord`; adjacent `ploke_tui::llm::manager::session::FullResponseTraceRecord` | `ploke-tui` emits JSON to tracing target; `runner.rs` slices current `llm_full_response_*.log` into run-local JSONL | `load_run_responses`, `load_all_full_response_records`, usage/response CLI commands | Included for agent runs when full-response tracing is active; sidecar may undercount final stop responses per current code comment. |
| MSB submission artifact | `<run>/multi-swe-bench-submission.jsonl`; batch aggregate at `<batch>/multi-swe-bench-submission.jsonl` | MSB submission record, `SubmissionArtifactState` | `write_msb_submission_artifact`; batch runner initializes aggregate | closure/protocol/CLI reports; `RunRecord.phases.packaging` tracks state/path | Included for treatment runs and batch aggregates; may be empty. |
| Protocol artifacts | `<run>/protocol-artifacts/*.json` | `StoredProtocolArtifact`, `StoredProtocolArtifactFile`, protocol aggregate types | `write_protocol_artifact` | `list_protocol_artifacts`, `load_protocol_artifact`, `load_protocol_aggregate` | Included when protocol procedures run during eval/compare; adjacent but feeds Prototype 1 branch evaluation and closure. |
| Run registration | registry path under eval home, with refs to run root/artifacts | `RunRegistration`, `RunArtifactRefs`, `RunLifecycle` | `persist_registration`, `RunRegistration::persist` | `load_registration_for_record_path`, run resolution and status sync | Included around normal eval attempts; important because it names artifacts not present in `RunRecord`. |
| Last run pointer | `<eval-home>/last-run.json` | `LastRunRecord` | `record_last_run_at` | `load_last_run_at`, default CLI record resolution | Outside loop authority; useful default selector only. |

## Campaign And Setup State

These are campaign/setup level surfaces that Prototype 1 uses but that may be outside narrow evidence-family reports.

| Surface | Path shape | Candidate types | Writers | Readers / projections | Inclusion |
| --- | --- | --- | --- | --- | --- |
| Campaign manifest | `<eval-home>/campaigns/<campaign>/campaign.json` | `CampaignManifest` | `save_campaign_manifest` | `load_campaign_manifest`, `adopt_campaign_manifest_from_closure_state` | Campaign-level input to loop. |
| Closure state | `<eval-home>/campaigns/<campaign>/closure-state.json` | `ClosureState`, `StoredClosureStateEnvelope` | `recompute_closure_state` | `load_closure_state`; campaign adoption; Prototype 1 branch evaluation compare | Campaign-level reduced snapshot; branch evaluation reads baseline/treatment closure. |
| Slice dataset | `<campaign>/slice.jsonl` | selected dataset rows as JSONL values | slice-writing code in `cli_facing` (`kept.join("\n")`) | setup/list/read paths in `cli_facing` | Campaign setup input; do not dump raw rows. |
| Active selection | `<eval-home>/selection.json` | `ActiveSelection` | `save_active_selection` | `load_active_selection_at` | Operator convenience, outside loop authority. |
| Model registry / active model | model registry JSON and active model JSON under eval config paths | `ModelRegistry`, `ActiveModelSelection` | `save_model_registry_at`, `save_active_model_at` | `load_model_registry_at`, `load_active_model_at` | Configuration, outside loop records. |
| Provider prefs | provider prefs JSON under eval config paths | `ProviderPrefs` | `save_provider_prefs_at` | `load_provider_prefs_at` | Configuration, outside loop records. |
| Target registry | target registry JSON under eval home/config paths | `TargetRegistry` | `recompute_target_registry` | `load_target_registry` | Dataset/registry setup, outside loop authority but feeds prepared runs. |
| Prepared run manifests | `run.json`, `batch.json` | `PreparedSingleRun`, `PreparedMsbBatch` | `PreparedSingleRun::write_manifest`, `PreparedMsbBatch::write_manifest` | normal runner load paths | Setup records for eval runs; included indirectly in run registration and `RunRecord`. |

## CLI Read Surfaces To Include In Map

- `prototype1-monitor report` loads scheduler, branch registry, transition journal, and evaluation directory (`report.rs`).
- `history-preview` imports transition journal line-by-line plus scheduler, branch registry, evaluations, invocations, attempt results, successor ready/completion, node records, runner requests, and runner results (`history_preview.rs`).
- `history-metrics` uses `FsEvidenceStore` documents plus transition journal, then assembles source attribution over journal/documents (`metrics.rs`).
- Prototype 1 monitor location reporting explicitly names scheduler, branch registry, legacy loop trace, transition journal, evaluations, nodes, node record, runner request, latest runner result, runtime invocation, attempt result, successor ready/completion, child worktree, child build products, and parent identity (`cli_facing.rs`).
- Run-artifact loading scans for `record.json.gz`, then attaches `llm-full-responses.jsonl` when present (`cli_facing.rs`).
- Standard eval CLI commands resolve default records through last-run/registration paths and then read `record.json.gz`, protocol artifacts, full-response sidecars, and run sidecars (`cli.rs`, `run_history.rs`, `run_registry.rs`).

## Adjacent Crate Surfaces

- `ploke-tui` emits full-response trace JSON through tracing target `FULL_RESPONSE_TARGET`; `ploke-eval` captures/slices that into run-local `llm-full-responses.jsonl`. This is relevant to eval records.
- `ploke-eval/src/tracing_setup.rs` writes normal logs `ploke_eval_<run_id>.log`, raw full-response logs `llm_full_response_<run_id>.log`, and optional Prototype 1 observation JSONL `prototype1_observation_<run_id>.jsonl` when `PLOKE_PROTOTYPE1_TRACE_JSONL` is set. These are observation/diagnostic logs, not loop records, but they can be used by monitor tooling.
- `ploke-rag` / `ploke-db` have BM25 sidecar save/load commands. The DB actor writes a lightweight JSON object with tokenizer version and doc count. This is outside Prototype 1 loop records unless a run explicitly uses BM25 sidecar commands.
- `ploke-llm` has provider/model test/debug JSON writers. They are outside Prototype 1 persistence and should be treated as noise for this map.
- `ingest/syn_parser` diagnostic JSON writers are parser diagnostics, not Prototype 1 loop persistence.

## False Positives And Noisy Areas

- `record.json` without gzip appears only in tests/legacy compatibility scaffolding; production writer is `record.json.gz`.
- Exact `state.json` was not a live Prototype 1 filename in `crates/ploke-eval`; relevant matches are `repo-state.json`, `closure-state.json`, scheduler state, and History state projections.
- `serde_json::to_string_pretty` in CLI render commands often prints JSON to stdout rather than persisting. Treat as persistence only when paired with `fs::write`, `write_json_file_pretty`, or a specific writer.
- Test fixtures under `crates/ploke-eval/src/tests/fixtures` and test-only writes create JSON/JSONL files but are not live persistence.
- Large artifact/log content should not be inspected directly: `slice.jsonl`, `record.json.gz`, raw response sidecars, transition journals, runner request/result payloads, observation streams, and protocol artifacts can contain prompts, patches, tool payloads, or benchmark issue bodies. Prefer path lists and field-level extraction.
- Mutable projections are not authority by themselves: `scheduler.json`, `branches.json`, `prototype1-loop-trace.json`, `heads.json`, latest `runner-result.json`, and node mirrors should be classified separately from append-only journals and sealed History blocks.

## Completeness Notes

- The highest-risk missed family is attempt-scoped runtime records under `nodes/<node-id>/`: `invocations`, `results`, `successor-ready`, `successor-completion`, and `streams`. They are easy to miss if evidence reports only track scheduler/latest result/journal.
- History persistence is a separate local store from transition journal persistence. The map should not collapse `transition-journal.jsonl` and `history/blocks/*.jsonl`; they have different writers, record types, and authority claims.
- Parent identity lives inside Artifact checkouts at `.ploke/prototype1/parent_identity.json`, not just under campaign state. It should be represented as Artifact-carried persistence.
- Normal eval-run sidecars and protocol artifacts feed Prototype 1 branch evaluation results. They are outside the Prototype 1 campaign root but are referenced by branch evaluation reports and runner results.
