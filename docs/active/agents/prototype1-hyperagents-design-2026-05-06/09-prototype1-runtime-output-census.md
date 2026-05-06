# Agent 09: Prototype 1 Runtime Output Census

Date: 2026-05-06

Scope surveyed:

- `crates/ploke-eval/src/cli/prototype1_state`
- `crates/ploke-eval/src/campaign.rs`

Important boundary: several actual runtime writes are delegated from
`prototype1_state::cli_facing` into `crates/ploke-eval/src/cli/prototype1_process.rs`.
Those are included only where scoped code calls or reads their artifacts, because
otherwise the census would falsely report missing writers.

## Summary

Prototype 1 currently has three different persistence classes sharing one
campaign-local tree:

- Append-style evidence:
  - `prototype1/transition-journal.jsonl`
  - `prototype1/history/blocks/segment-000000.jsonl`
  - `prototype1/history/index/by-hash.jsonl`
  - `prototype1/history/index/by-lineage-height.jsonl`
  - parent/child channel JSONL files under `nodes/<node-id>/channels/<runtime-id>/`
- Attempt-scoped JSON evidence:
  - `nodes/<node-id>/invocations/<runtime-id>.json`
  - `nodes/<node-id>/results/<runtime-id>.json`
  - `nodes/<node-id>/successor-ready/<runtime-id>.json`
  - `nodes/<node-id>/successor-completion/<runtime-id>.json`
- Mutable projections:
  - `campaign.json`
  - `closure-state.json`
  - `prototype1/scheduler.json`
  - `prototype1/branches.json`
  - `nodes/<node-id>/node.json`
  - `nodes/<node-id>/runner-request.json`
  - `nodes/<node-id>/runner-result.json`
  - CLI report, monitor, metrics, timing, and history-preview renderings

The strongest provenance is in `FsBlockStore` sealed History append, then in
`PrototypeJournal` JSONL append. Scheduler, branch registry, node records, and
latest runner results are useful operating projections but are mutable and
should not override journal or sealed History evidence.

## Root And Naming Conventions

Let:

- `campaign_dir = campaigns_dir()?.join(<campaign_id>)`
- `campaign_manifest_path = campaign_dir.join("campaign.json")`
- `prototype_root = campaign_dir.join("prototype1")`

`campaign.rs` defines:

- `campaign_manifest_path(campaign_id)` -> `campaign_dir/campaign.json`
- `campaign_closure_state_path(campaign_id)` -> `campaign_dir/closure-state.json`

`prototype1_state` and imported intervention helpers use:

- `prototype1_transition_journal_path(manifest)` -> `prototype_root/transition-journal.jsonl`
- `prototype1_scheduler_path(manifest)` -> `prototype_root/scheduler.json`
- `prototype1_branch_registry_path(manifest)` -> `prototype_root/branches.json`
- `prototype1_node_dir(manifest, node)` -> `prototype_root/nodes/<node-id>`
- `invocation_path(node_dir, runtime)` -> `nodes/<node-id>/invocations/<runtime-id>.json`
- `result_path(node_dir, runtime)` -> `nodes/<node-id>/results/<runtime-id>.json`
- `successor_ready_path(node_dir, runtime)` -> `nodes/<node-id>/successor-ready/<runtime-id>.json`
- `successor_completion_path(node_dir, runtime)` -> `nodes/<node-id>/successor-completion/<runtime-id>.json`
- `FsBlockStore::for_campaign_manifest(manifest)` -> `prototype_root/history`

## Complete Output Map

### Campaign Manifest: `campaign.json`

Type/schema:

- `CampaignManifest`
- `CAMPAIGN_MANIFEST_SCHEMA_VERSION = "campaign-manifest.v1"`

Path:

- `campaign_manifest_path(campaign_id)`
- Concrete: `<campaign_dir>/campaign.json`

Written by:

- `save_campaign_manifest(manifest)` in `campaign.rs`
- Called by `prepare_prototype1_loop_campaign` and
  `prepare_prototype1_treatment_campaign` in `cli_facing.rs`

Read by:

- `load_campaign_manifest(campaign_id)` in `campaign.rs`
- `resolve_campaign_config`
- `prepare_prototype1_treatment_campaign`
- monitor/state setup paths in `cli_facing.rs`

Authority/provenance:

- Configuration authority for campaign selection and roots.
- Not runtime evidence and not History authority.

Freshness/fracture:

- Stable during a run unless explicit campaign setup/adoption rewrites it.
- Can be adopted from `closure-state.json`, so old campaigns may have legacy
  fields normalized into a newer manifest.

### Closure State: `closure-state.json`

Type/schema:

- Read through `StoredClosureStateEnvelope` in `campaign.rs`
- Full producer is outside this scope in closure/eval code.

Path:

- `campaign_closure_state_path(campaign_id)`
- Concrete: `<campaign_dir>/closure-state.json`

Written by:

- Not written in scoped files.
- Referenced by Prototype 1 branch evaluation and treatment campaign setup.

Read by:

- `adopt_campaign_manifest_from_closure_state`
- `cli_facing.rs` branch evaluation code via `load_closure_state`
- `build_prototype1_branch_evaluation_report`

Authority/provenance:

- Evaluation closure aggregate.
- Authoritative for baseline/treatment completed instances only within the eval
  subsystem, not for parent/child runtime transitions.

Freshness/fracture:

- Legacy source of campaign config.
- Branch evaluation trusts it as the source of completed records but then emits
  separate Prototype 1 evaluation artifacts.

### Slice Dataset: `slice.jsonl`

Type/schema:

- JSONL subset of the registry dataset, not a dedicated Prototype 1 Rust type.

Path:

- `prepare_prototype1_loop_campaign`: `campaign_dir.join("slice.jsonl")`

Written by:

- `write_prototype1_slice_dataset` in `cli_facing.rs`

Read by:

- Through the campaign manifest `dataset_sources`, later resolved by
  `resolve_campaign_config`

Authority/provenance:

- Input projection for a Prototype 1 campaign.
- Not runtime evidence.

Freshness/fracture:

- Fixed when campaign is created.
- Duplicates selected source dataset rows without carrying a separate digest in
  this scoped code.

### Scheduler State: `prototype1/scheduler.json`

Type/schema:

- `Prototype1SchedulerState`
- `PROTOTYPE1_SCHEDULER_SCHEMA_VERSION = "prototype1-scheduler.v1"`

Path:

- `prototype1_scheduler_path(manifest)`
- Concrete: `<campaign_dir>/prototype1/scheduler.json`

Written by:

- `save_scheduler_state` in `intervention/scheduler.rs`
- Called by:
  - `register_root_parent_node`
  - `update_scheduler_policy`
  - `update_node_status`
  - `update_node_workspace_root`
  - `record_runner_result`
  - `record_continuation_decision`
  - `load_or_register_treatment_evaluation_node`

Read by scoped code:

- `load_or_default_scheduler_state`
- `load_scheduler_state`
- `Parent::check` indirectly through node context
- `history_preview::FsEvidenceStore::documents`
- `metrics::run`
- `report::Report::load`
- monitor/list/peek/watch/timing paths in `cli_facing.rs`

Authority/provenance:

- Mutable scheduler projection.
- Useful for current frontier, node list, search policy, and last continuation
  decision.
- Not authoritative for History or Crown.

Freshness/fracture:

- Fractured with node-local `node.json` because both mirror node status.
- Fractured with `transition-journal.jsonl` because scheduler records latest
  status but journal records transition sequence.
- History preview explicitly marks scheduler as projection-only deferred
  evidence.

### Branch Registry: `prototype1/branches.json`

Type/schema:

- `Prototype1BranchRegistry`
- `PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION = "prototype1-branch-registry.v1"`

Path:

- `prototype1_branch_registry_path(manifest)`
- Concrete: `<campaign_dir>/prototype1/branches.json`

Written by:

- `save_branch_registry` in `intervention/branch_registry.rs`
- Called by:
  - `record_synthesized_branches`
  - `mark_treatment_branch_applied`
  - `select_treatment_branch`
  - `restore_treatment_branch`
  - `record_treatment_branch_evaluation`

Read by scoped code:

- `resolve_treatment_branch`
- `load_or_default_branch_registry`
- `prototype1_branch_status_report`
- `history_preview::FsEvidenceStore::documents`
- `metrics::run`
- `report::Report::load`
- child planning and branch selection paths in `cli_facing.rs`

Authority/provenance:

- Mutable branch catalog and latest-evaluation summary.
- Authoritative enough to resolve branch content for current operations.
- Not authoritative for promotion, History, or Crown.

Freshness/fracture:

- `latest_evaluation` is a mutable summary of `prototype1/evaluations/<branch-id>.json`.
- `selected_branch_id` and `active_targets` can be stale relative to journaled
  successor selection records.
- History preview defers the whole registry as a mutable catalog.

### Node Record: `prototype1/nodes/<node-id>/node.json`

Type/schema:

- `Prototype1NodeRecord`
- `PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION = "prototype1-treatment-node.v1"`

Path:

- `prototype1_node_record_path(manifest, node_id)`
- Concrete: `<prototype_root>/nodes/<node-id>/node.json`

Written by:

- `save_node_record` in `intervention/scheduler.rs`
- Called by registration and node update helpers:
  - `register_root_parent_node`
  - `update_node_status`
  - `update_node_workspace_root`
  - `load_or_register_treatment_evaluation_node`

Read by scoped code:

- `load_node_record`
- `C1::load`, `C2::load`, `C3::load`, `ObserveChild::transition`
- `Parent::<Unchecked>::load`
- `identity::ParentIdentity::from_node`
- `invocation` and successor-ready/completion producers via node lookup
- `history_preview::FsEvidenceStore::documents`
- CLI runner/state/monitor/report paths

Authority/provenance:

- Scheduler-owned mirror for a node.
- Projection of node status and paths, not an append-only fact.

Freshness/fracture:

- Duplicates scheduler node entries.
- `runner_result_path` points to latest node result; attempt-specific results
  are separate.
- History preview imports it as `projection_degraded_pre_history`.

### Runner Request: `prototype1/nodes/<node-id>/runner-request.json`

Type/schema:

- `Prototype1RunnerRequest`

Path:

- `prototype1_runner_request_path(manifest, node_id)`
- Concrete: `<prototype_root>/nodes/<node-id>/runner-request.json`

Written by:

- `save_runner_request` in `intervention/scheduler.rs`
- Called by `register_root_parent_node`, `update_node_workspace_root`, and
  `load_or_register_treatment_evaluation_node`

Read by scoped code:

- `load_runner_request`
- `Prototype1RunnerCommand::run`
- `C3::transition`
- `execute_prototype1_runner_node` in out-of-scope producer
- `history_preview::FsEvidenceStore::documents`

Authority/provenance:

- Launch/request projection for a node.
- Not an invocation authority token; attempt authority is in
  `invocations/<runtime-id>.json`.

Freshness/fracture:

- Mutable because `workspace_root` can be updated after materialization.
- Can drift from node record if writes partially fail.

### Latest Runner Result: `prototype1/nodes/<node-id>/runner-result.json`

Type/schema:

- `Prototype1RunnerResult`

Path:

- `prototype1_runner_result_path(manifest, node_id)`
- Concrete: `<prototype_root>/nodes/<node-id>/runner-result.json`

Written by:

- `record_runner_result` -> `save_runner_result` in `intervention/scheduler.rs`
- Called from out-of-scope producer `record_attempt_runner_result` and build
  failure paths in `prototype1_process.rs`
- `clear_runner_result` may remove it before a fresh attempt.

Read by scoped code:

- `load_runner_result`
- `Prototype1RunnerCommand::run`
- monitor/timing/status paths in `cli_facing.rs`
- `history_preview::FsEvidenceStore::documents`
- `metrics::run`

Authority/provenance:

- Latest node-level outcome projection.
- Weaker than attempt result because it is overwritten/cleared.

Freshness/fracture:

- Fractured with `nodes/<node-id>/results/<runtime-id>.json`.
- History preview marks latest runner-result copy as a mutable projection.

### Attempt Runner Result: `prototype1/nodes/<node-id>/results/<runtime-id>.json`

Type/schema:

- Same `Prototype1RunnerResult` payload as latest runner result.

Path:

- `result_path(node_dir, runtime_id)`
- Concrete: `<prototype_root>/nodes/<node-id>/results/<runtime-id>.json`

Written by:

- `write_runner_result_at` via out-of-scope
  `record_attempt_runner_result` in `prototype1_process.rs`

Read by scoped code:

- `ObserveChild::transition` expects this through `RunnerResultSurface`
- `history_preview::FsEvidenceStore::documents`
- `metrics::run`
- `run_prototype1_branch_evaluation_via_child` reads it after child exit

Authority/provenance:

- Stronger than latest runner-result because it is attempt-scoped.
- Still child self-report/evaluation output, not promotion authority.

Freshness/fracture:

- If latest node result and attempt result disagree, attempt result has better
  runtime provenance but parent observation still needs journal linkage.

### Invocation: `prototype1/nodes/<node-id>/invocations/<runtime-id>.json`

Type/schema:

- `Invocation`
- `SCHEMA_VERSION = "prototype1-invocation.v1"`
- Classified into `InvocationAuthority::Child` or `InvocationAuthority::Successor`

Path:

- `invocation_path(node_dir, runtime_id)`

Written by:

- `write_child_invocation`
- `write_successor_invocation_for_retired_parent`
- Child invocation written in `C3::transition`
- Successor invocation written in out-of-scope
  `spawn_and_handoff_prototype1_successor`

Read by:

- `invocation::load`
- `invocation::load_authority`
- `invocation::load_executable`
- `Prototype1RunnerCommand::run`
- `acknowledge_prototype1_state_handoff`
- `history_preview::FsEvidenceStore::documents`
- `metrics::run`

Authority/provenance:

- Attempt-scoped bootstrap contract.
- It is a role limiter for the launched process, not Crown/History authority by
  itself.

Freshness/fracture:

- Attempt-scoped and normally retained.
- Successor invocation is only executable because its writer requires
  `Parent<Retired>`; the file alone must not be treated as the handoff proof.

### Parent/Child Channel JSONL

Type/schema:

- `Envelope<ToParent>` and `Envelope<ToChild>`
- `SCHEMA_VERSION = "prototype1-runtime-channel.v1"`

Paths:

- `channel_root(node_dir, runtime_id)`
- `nodes/<node-id>/channels/<runtime-id>/child-to-parent.jsonl`
- `nodes/<node-id>/channels/<runtime-id>/parent-to-child.jsonl`

Written by:

- `FileTransport::append`
- `Channel<Child<_>>::send_ready`
- `send_evaluating`
- `send_result_written`
- `send_failed`
- `send_exited`
- `Channel<Parent<_>>::send_cancel`

Read by:

- `Channel::<...>::recv_from_child`
- `Channel::<...>::recv_from_parent`
- `C3::wait_for_ready`
- `ObserveChild::observed_result_path`

Authority/provenance:

- Typed transport evidence for child state messages.
- Includes body hash and endpoint validation.
- Not currently indexed by history preview.

Freshness/fracture:

- Parallel to journal `JournalEntry::Child` and legacy `ChildReady`.
- If both exist, the journal has better integration with current reports.

### Transition Journal: `prototype1/transition-journal.jsonl`

Type/schema:

- `PrototypeJournal`
- `JournalEntry` tagged enum

Path:

- `prototype1_transition_journal_path(manifest)`

Written by:

- `PrototypeJournal::append`
- Direct scoped writers:
  - `C1 -> C2`: `JournalEntry::MaterializeBranch`
  - `C2 -> C3`: `JournalEntry::BuildChild`
  - `C3 -> C4`: `JournalEntry::SpawnChild` and legacy `ChildReady`
  - `Child<State>`: `JournalEntry::Child`
  - `C4 -> C5`: `JournalEntry::ObserveChild`
  - parent turn start/resource/successor selection in `cli_facing.rs`
- Imported out-of-scope writers:
  - `append_prototype1_journal_entry`
  - `append_successor_record`
  - `persist_prototype1_buildable_child_artifact`
  - successor ready/completion/handoff recorders

Read by:

- `PrototypeJournal::load_entries`
- Replay helpers:
  - `replay_materialize_branch`
  - `replay_build_child`
  - `replay_spawn_child`
  - `replay_observe_child`
  - `replay_all`
- `history_preview::FsEvidenceStore::transition_journal`
- `metrics::run`
- `report::Report::load`
- monitor/timing/state inspection paths

Authority/provenance:

- Best current pre-History transition evidence.
- Append-only by API and `sync_data` on each append.
- Still not sealed Crown authority.

Freshness/fracture:

- Contains legacy flattened variants:
  - `ChildArtifactCommitted`
  - `ActiveCheckoutAdvanced`
  - `SuccessorHandoff`
  - `ChildReady`
- Newer structural records exist in `child::Record` and `successor::Record`.
- History preview imports it line-by-line as pre-History evidence.

### Child Materialization / Build / Spawn Outputs

Paths:

- Workspace: `nodes/<node-id>/worktree`
- Promoted child binary: `nodes/<node-id>/bin/ploke-eval`
- Cargo scratch: `nodes/<node-id>/target`
- Streams: `nodes/<node-id>/streams/<runtime-id>/stdout.log`
- Streams: `nodes/<node-id>/streams/<runtime-id>/stderr.log`

Written by:

- `MaterializeBranch::transition` via `GitWorktreeBackend::realize`
- `BuildChild::transition` creates `target`, copies binary to `bin/ploke-eval`
- `SpawnChild::transition` creates stream files and spawns child process
- Out-of-scope `build_prototype1_runner_binary` and
  `spawn_prototype1_child_runner` provide similar legacy/live path behavior

Read by:

- `C2::load`, `C3::load`
- `BuildChild::transition`
- `SpawnChild::transition`
- monitor/list/peek/watch/timing

Authority/provenance:

- Runtime artifacts, not durable authority.
- Binary path is launch evidence only; active parent authority comes from
  parent identity plus History startup checks.

Freshness/fracture:

- Worktree/bin/target are temporary and cleanup may remove them.
- Streams are operational evidence and not imported by history preview in this
  scoped code.

### Branch Evaluation Artifact: `prototype1/evaluations/<branch-id>.json`

Type/schema:

- `Prototype1BranchEvaluationReport`

Path:

- `prototype1_branch_evaluation_path(manifest, branch_id)`
- Concrete: `<prototype_root>/evaluations/<branch-id>.json`

Written by:

- Out-of-scope producer `run_prototype1_branch_evaluation` calls
  `write_json_file_pretty`
- Scoped path/type builder lives in `cli_facing.rs`

Read by:

- `ObserveChild::load_report`
- `load_prototype1_branch_evaluation_report`
- `history_preview::FsEvidenceStore::documents`
- `metrics::run`
- `report::Report::load`

Authority/provenance:

- Policy/evaluation judgment artifact for a branch.
- Stronger than the branch registry summary.
- Still derived from baseline/treatment closure states and compressed run
  records rather than sealed History.

Freshness/fracture:

- Per-branch file can be replaced by later evaluations.
- Branch registry stores a mutable latest summary separately.

### Treatment Campaign Outputs

Paths:

- Treatment campaign manifest:
  `<campaigns>/<baseline>-treatment-<branch-id>-<millis>/campaign.json`
- Treatment closure state:
  `<treatment_campaign_dir>/closure-state.json`
- Treatment instances/batches roots under manifest-configured
  `instances_root/treatments/<branch-id>/...` and
  `batches_root/treatments/<branch-id>/...`

Written by:

- `prepare_prototype1_treatment_campaign` writes treatment `campaign.json`
- `advance_eval_closure` and `advance_protocol_closure` write closure/eval
  artifacts outside this scoped code

Read by:

- `build_prototype1_branch_evaluation_report`
- `run_prototype1_branch_evaluation`

Authority/provenance:

- Evaluation/protocol artifacts for comparing a treatment branch.
- Not parent/child protocol authority.

Freshness/fracture:

- A successful runner result points back to the branch evaluation artifact, not
  every lower-level treatment artifact.

### Successor Ready: `prototype1/nodes/<node-id>/successor-ready/<runtime-id>.json`

Type/schema:

- `SuccessorReadyRecord`
- `SUCCESSOR_READY_SCHEMA_VERSION = "prototype1-successor-ready.v1"`

Path:

- `successor_ready_path(node_dir, runtime_id)`

Written by:

- `write_successor_ready_record`
- Called by out-of-scope `record_prototype1_successor_ready`, itself called by
  scoped `acknowledge_prototype1_state_handoff`

Read by:

- `load_successor_ready_record`
- `wait_for_prototype1_successor_ready`
- `history_preview::FsEvidenceStore::documents`
- monitor/timing paths

Authority/provenance:

- Successor acknowledgement evidence.
- Not Crown authority; the predecessor sealed and appended History before
  spawning successor.

Freshness/fracture:

- Attempt-scoped.
- Successor record is also appended to `transition-journal.jsonl`.

### Successor Completion: `prototype1/nodes/<node-id>/successor-completion/<runtime-id>.json`

Type/schema:

- `SuccessorCompletionRecord`
- `SUCCESSOR_COMPLETION_SCHEMA_VERSION = "prototype1-successor-completion.v1"`

Path:

- `successor_completion_path(node_dir, runtime_id)`

Written by:

- `write_successor_completion_record`
- Called by scoped `record_failed_successor_turn` and successful end of
  `Prototype1StateCommand::run_turn` through out-of-scope
  `record_prototype1_successor_completion`

Read by:

- `history_preview::FsEvidenceStore::documents`
- monitor/timing terminal-state logic

Authority/provenance:

- Terminal status for successor rehydration attempt.
- Procedure-run evidence, not parent-selection authority.

Freshness/fracture:

- Attempt-scoped but currently separate from sealed History.
- Also journaled as `successor::Record::Completed`.

### Successor Handoff Journal Records

Type/schema:

- `successor::Record` with states:
  - `Selected`
  - `Checkout`
  - `Spawned`
  - `Ready`
  - `TimedOut`
  - `ExitedBeforeReady`
  - `Completed`
- Legacy `SuccessorHandoffEntry`

Path:

- `prototype1/transition-journal.jsonl`

Written by:

- Scoped `Prototype1StateCommand::run_turn` appends `Successor::Selected`
- Out-of-scope `spawn_and_handoff_prototype1_successor` appends `Spawned`,
  timeout/exit, and legacy `SuccessorHandoff`
- Out-of-scope ready/completion helpers append `Ready` and `Completed`

Read by:

- `history_preview`
- `metrics`
- monitor/timing terminal-state logic

Authority/provenance:

- Good handoff evidence.
- Actual Crown/History authority is the sealed block append, not these records.

Freshness/fracture:

- Both legacy flattened and newer structural successor records coexist.
- Some states are JSON files plus journal entries, so consumers need a join by
  runtime id.

### Sealed History Store

Types:

- `FsBlockStore`
- `Block<block::Sealed>`
- `StoredBlock`
- `LineageState`
- `BlockHead`

Paths:

- `prototype1/history/blocks/segment-000000.jsonl`
- `prototype1/history/index/by-hash.jsonl`
- `prototype1/history/index/by-lineage-height.jsonl`
- `prototype1/history/index/heads.json`

Written by:

- `FsBlockStore::append`
- Called by out-of-scope `spawn_and_handoff_prototype1_successor` after
  scoped parent has reached `Parent<Selectable>`

Read by:

- `FsBlockStore::lineage_state`
- `FsBlockStore::sealed_head_block`
- `Startup::<Genesis>::from_history`
- `Startup::<Predecessor>::from_history`

Authority/provenance:

- Strongest current authority surface.
- Append checks expected `LineageState`, verifies block hash, appends indexes,
  then overwrites `heads.json` projection.

Freshness/fracture:

- `segment-000000.jsonl`, `by-hash.jsonl`, and `by-lineage-height.jsonl` are
  append-style evidence.
- `heads.json` is a mutable head projection and can be missing/stale relative
  to append indexes; code detects missing projection cases.
- Stored block loader currently rejects non-empty stored blocks, so current
  live handoff blocks are minimal and cannot yet reconstruct rich entry sets.

### Parent Identity: `.ploke/prototype1/parent_identity.json`

Type/schema:

- `ParentIdentity`
- `PARENT_IDENTITY_SCHEMA_VERSION = "prototype1-parent-identity.v1"`

Path:

- `parent_identity_path(repo_root)`
- Concrete: `<checkout>/.ploke/prototype1/parent_identity.json`

Written by:

- `write_parent_identity`
- Called by:
  - `prepare_prototype1_parent_setup`
  - `initialize_prototype1_parent_identity`
  - out-of-scope `persist_prototype1_buildable_child_artifact`

Read by:

- `load_parent_identity`
- `load_parent_identity_optional`
- `resolve_prototype1_parent_identity`
- `infer_campaign_from_parent_identity`
- `Parent::<Unchecked>::check`

Authority/provenance:

- Artifact-carried identity for a parent-capable checkout.
- Required for parent startup but not sufficient without scheduler and History
  checks.

Freshness/fracture:

- Git-tracked artifact file that changes with active checkout.
- It is deliberately outside `prototype_root`; monitor treats it as active
  checkout artifact identity.

### Child Plan Message Box

Type/schema:

- `ChildPlanFiles`
- `ChildPlanFile`
- `Open<ChildPlan>` / `Locked<ChildPlan>` / `Received<ChildPlan>`

Path:

- `prototype1/messages/child-plan/<parent-node-id>.json`

Written by:

- The typed box API `Open::<ChildPlan>::lock` accepts a writer closure.
- Scoped planning code creates `ChildPlanFiles::for_parent`; actual file IO is
  through helper functions in `cli_facing.rs` using this typed carrier.

Read by:

- `Locked::<ChildPlan>::from_box`
- `read_child_plan_file`
- `receive_existing_child_plan`
- validation in `ChildPlan::ready_receiver`

Authority/provenance:

- Typed cross-runtime obligation for parent child-plan handoff.
- Stronger than ad hoc file paths because sender/receiver state is encoded in
  `MessageBox` transitions.

Freshness/fracture:

- Current plan file names by parent node id and can be reused/overwritten per
  parent generation.
- It names scheduler, branch registry, node, and runner-request paths, so it is
  a structured projection over mutable files.

### Legacy Loop Trace: `prototype1/prototype1-loop-trace.json`

Type/schema:

- CLI-facing trace path only in scoped code.

Path:

- `prototype1_trace_path(campaign_manifest_path)`
- Concrete: `<prototype_root>/prototype1-loop-trace.json`

Written by:

- Not written in scoped files.
- Monitor labels it as overwritten by the legacy loop controller path.

Read by:

- Monitor/location/timing views.
- History preview may reference trace paths embedded in successor completion
  records.

Authority/provenance:

- Projection/diagnostic.

Freshness/fracture:

- Mutable and overwritten per legacy loop run.

### Observation Logs And Provider Attempts

Types:

- Parsed into `ObservationEvidence`, `ProviderHttpEvidence`,
  `RunArtifacts`, `RawFullResponseRecord`, and timing rows in `cli_facing.rs`

Paths:

- Not fixed under `prototype_root` by the scoped path constructors.
- Tests reference JSONL attempt logs such as `/tmp/attempts.jsonl`.
- Run artifacts are discovered from closure/run record paths.

Written by:

- Not written in scoped files.
- Produced by tracing/LLM/eval subsystems outside the requested scope.

Read by:

- `load_observation_evidence`
- `parse_observation_log`
- `load_run_artifacts`
- `load_run_responses`
- monitor timing views

Authority/provenance:

- Telemetry and provider-attempt evidence.
- Useful for timing and debugging, not protocol authority.

Freshness/fracture:

- Fractured from runtime IDs unless trace spans carry campaign/node/runtime
  context.
- `RuntimeTelemetry::install_for_chat_requests` improves this for child and
  parent phases.

### CLI Projections

Projection functions:

- `render_resolved_campaign_config`
- `print_prototype1_setup_report`
- `Prototype1RunnerCommand::run`
- `Prototype1StateCommand::run_turn` table/JSON report
- `report::run`
- `history_preview::run` / `history_preview::build`
- `metrics::run`
- monitor `list`, `peek`, `watch`, `timing`

Persisted files:

- None for the projection output itself.
- Projections read the files above and render stdout JSON/table output.

Authority/provenance:

- Projection-only.
- `history_preview` explicitly distinguishes raw/degraded/pre-History imports
  from sealed History.

Freshness/fracture:

- Most CLI projections recompute on read from mutable stores.
- They should not introduce source facts; when they do infer terminal state,
  that inference should stay labeled as projection.

### Active Selection And Monitor Target

Types:

- `ActiveSelection`
- `ActivePrototype1MonitorTarget`

Paths:

- `active_selection_file()` -> `$PLOKE_EVAL_HOME/selection.json`
- `prototype1_monitor_target_file()` ->
  `$PLOKE_EVAL_HOME/prototype1-monitor-target.json`

Written by:

- `save_active_selection` outside scoped Prototype 1 state code
- `save_active_prototype1_monitor_target`
- `record_active_prototype1_monitor_target` in `cli_facing.rs`

Read by:

- `load_active_selection`
- `load_active_prototype1_monitor_target`
- campaign resolution helpers in `cli_facing.rs`

Authority/provenance:

- Operator convenience state.
- Not campaign/runtime evidence.

Freshness/fracture:

- May point at an old campaign or repo root.
- Only useful as a default selector.

## Provenance Ranking

1. `FsBlockStore` sealed block append:
   - highest current authority;
   - transition-checked local History;
   - still not distributed consensus.
2. `PrototypeJournal` JSONL:
   - append-only transition evidence;
   - imports legacy and structural records;
   - pre-History/degraded relative to Crown.
3. Attempt-scoped JSON:
   - invocation, attempt result, successor ready/completion;
   - strong runtime locality, weak global authority.
4. Mutable node/scheduler/branch projections:
   - operationally necessary;
   - must not override journal/History evidence.
5. CLI reports/monitor/history-preview:
   - projections over the above;
   - no independent authority.

## Fracture Points

- Latest-vs-attempt runner results:
  - `nodes/<node-id>/runner-result.json` is mutable latest state;
  - `nodes/<node-id>/results/<runtime-id>.json` is attempt-scoped evidence.
- Scheduler-vs-node mirrors:
  - `scheduler.json` embeds node records;
  - each node also has `node.json`;
  - partial writes can disagree.
- Branch registry-vs-evaluation artifact:
  - `branches.json.latest_evaluation` is a mutable summary;
  - `evaluations/<branch-id>.json` is the full report.
- Successor handoff duplication:
  - invocation JSON, ready JSON, completion JSON, `successor::Record`, legacy
    `SuccessorHandoffEntry`, and sealed History all describe adjacent slices.
- History head projection:
  - append indexes are evidence;
  - `heads.json` is mutable projection and can be stale/missing.
- Channel-vs-journal child state:
  - channel JSONL and `JournalEntry::Child` both carry child messages;
  - history preview currently indexes the journal, not channel files.
- Legacy flattened journal entries:
  - `ChildReady`, `ChildArtifactCommitted`, `ActiveCheckoutAdvanced`, and
    `SuccessorHandoff` remain for replay/import but should be normalized before
    future History admission.

## Design Implication For HyperAgents

The durable runtime output model should treat `prototype_root` as a mixed store,
not a single authority tree. A HyperAgent reader should join files by
`campaign_id`, `node_id`, and `runtime_id`, then classify each source before
using it:

- sealed History block: authority candidate;
- transition journal row: transition evidence;
- attempt JSON: runtime-local evidence;
- scheduler/branch/node/latest result: mutable projection;
- CLI/monitor/history-preview output: view only.

The missing structural object is a single typed "runtime attempt evidence
bundle" or equivalent index that ties invocation, channel messages, attempt
result, latest projection, journal rows, successor files, and any sealed
History references without letting the mutable projections become authoritative.
