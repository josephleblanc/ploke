# Prototype 1 Persistence Map: Loop-Native State

Worker: 1
Scope: live single-parent `ploke-eval loop prototype1-state`, primarily `crates/ploke-eval/src/cli/prototype1_state`.
Date: 2026-05-03

This map covers files written or consumed by the loop-native parent, child, and successor path. It excludes raw artifact payload dumps and treats benchmark run records under `~/.ploke-eval/instances/...` as referenced external run artifacts unless the Prototype 1 path writes a loop-owned projection.

Campaign root:

```text
~/.ploke-eval/campaigns/<campaign-id>/
```

Prototype root:

```text
~/.ploke-eval/campaigns/<campaign-id>/prototype1/
```

## Join Keys

- `campaign_id`: joins campaign manifest, scheduler, branch registry, runner results, journal records, parent identity, invocations, successor records, and reports.
- `node_id`: main scheduler/runtime join. Root parent node id is `node-<sha256(branch_id,generation)[0..16]>`; treatment nodes use the same `prototype1_node_id(branch_id, generation)` rule (`crates/ploke-eval/src/intervention/scheduler.rs:223`).
- `generation`: separates parent/child turns and is validated against parent identity for direct-child lineage.
- `branch_id`: joins branch registry, scheduler node, evaluation report, runner result, and successor selection.
- `runtime_id`: joins attempt-scoped invocation, attempt result, child/successor journal entries, streams, successor ready/completion.
- `source_state_id`, `target_relpath`, `candidate_id`: branch-registry/source-node joins and branch id derivation.
- `parent_id`/`parent_node_id`/`previous_parent_id`: parent identity and lineage joins.
- `transition_id`: journal-local join for before/after transition entries.

## Persisted Items

### Scheduler

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json`.
- Type/schema: `Prototype1SchedulerState`, schema `"prototype1-scheduler.v1"`; contains policy, frontier/completed/failed node ids, last continuation decision, and embedded `Prototype1NodeRecord` list (`crates/ploke-eval/src/intervention/scheduler.rs:12`, `:157`).
- Writers: `save_scheduler_state` writes pretty JSON (`crates/ploke-eval/src/intervention/scheduler.rs:372`). Main write sites include root setup (`register_root_parent_node`, `:229`), treatment node registration (`register_treatment_evaluation_node`, `:688`), status updates (`update_node_status`, `:515`), workspace-root update (`update_node_workspace_root`, `:562`), runner result recording (`record_runner_result`, `:600`), and continuation decisions (`record_continuation_decision`, `:676`).
- Readers/CLI: `load_scheduler_state`/`load_or_default_scheduler_state` (`:344`, `:361`); `loop prototype1-monitor report`, `history-preview`, `history-metrics`, `watch`, `timing`; `loop prototype1-runner --campaign ... --node-id ...`; `loop prototype1-state` startup and successor validation.
- Join IDs: `campaign_id`, `node_id`, `branch_id`, `generation`, `last_continuation_decision.selected_next_branch_id`.
- Evidence status: projection/cache. It is authoritative for the current scheduler frontier used by the live loop, but not History/Crown authority; `history_preview` classifies scheduler as `projection_only`.
- Safe inspection:

```bash
jq '{policy, last_continuation_decision, frontier_node_ids, completed_node_ids, failed_node_ids, nodes: [.nodes[] | {node_id, parent_node_id, generation, instance_id, status, branch_id, runner_request_path, runner_result_path}]}' ~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json
```

### Branch Registry

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/branches.json`.
- Type/schema: `Prototype1BranchRegistry`, schema `"prototype1-branch-registry.v1"`; contains `source_nodes`, `active_targets`, `TreatmentBranchNode`, and `TreatmentBranchEvaluationSummary` (`crates/ploke-eval/src/intervention/branch_registry.rs:16`, `:99`).
- Writers: `save_branch_registry` (`crates/ploke-eval/src/intervention/branch_registry.rs:202`), called by `record_synthesized_branches` (`:216`), `mark_treatment_branch_applied` (`:352`), `select_treatment_branch` (`:452`), `restore_treatment_branch` (`:613`), and `record_treatment_branch_evaluation` (`:682`). The loop controller records synthesized branches and selected branch state (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:821`, `:1001`).
- Readers/CLI: `load_or_default_branch_registry` and `resolve_treatment_branch` (`crates/ploke-eval/src/intervention/branch_registry.rs:185`, `:583`); `loop prototype1-branch status/show/apply`; `loop prototype1-monitor report/history-preview`; child evaluation resolves branch content before materialization.
- Join IDs: `campaign_id`, `source_state_id`, `target_relpath`, `candidate_id`, `branch_id`, `parent_branch_id`, `selected_branch_id`, optional `patch_id`/`derived_artifact_id`.
- Evidence status: projection plus evidence references. It stores proposed content and treatment summaries, but current code comments note some artifact ids are text-file fallbacks rather than whole-runtime artifact authority.
- Safe inspection:

```bash
jq '{updated_at, sources: [.source_nodes[] | {instance_id, source_state_id, parent_branch_id, target_relpath, selected_branch_id, branches: [.branches[] | {branch_id, candidate_id, status, apply_id, latest_evaluation}]}], active_targets}' ~/.ploke-eval/campaigns/<campaign>/prototype1/branches.json
```

### Transition Journal

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/transition-journal.jsonl`.
- Type/schema: JSONL of `JournalEntry` tagged by `kind`; includes `ParentStarted`, `Resource`, `Successor`, `SuccessorHandoff`, `MaterializeBranch`, `BuildChild`, `SpawnChild`, typed `Child<State>`, `ChildReady`, and `ObserveChild` (`crates/ploke-eval/src/cli/prototype1_state/journal.rs:332`).
- Writers: `PrototypeJournal::append` appends and `sync_data`s (`crates/ploke-eval/src/cli/prototype1_state/journal.rs:655`). `prototype1-state` appends `ParentStarted`, resource samples, transition entries, successor selection, and successor handoff (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5177`, `:5404`; `crates/ploke-eval/src/cli/prototype1_process.rs:1326`, `:1354`). Child runtime appends typed `Child` state records (`crates/ploke-eval/src/cli/prototype1_state/child.rs:183`).
- Readers/CLI: `PrototypeJournal::load_entries` and replay helpers; `loop prototype1-monitor report/watch/history-preview/history-metrics`; `ObserveChild` polls typed child result records from the journal before loading the attempt result (`crates/ploke-eval/src/cli/prototype1_state/c4.rs:227`).
- Join IDs: `campaign_id`, `node_id`, `branch_id`, `generation`, `runtime_id`, `transition_id`, `parent_identity.parent_id`.
- Evidence status: strongest loop-native transition evidence currently available, but still a journal/admission preview, not sealed History authority. Some variants are explicitly legacy storage labels.
- Safe inspection:

```bash
jq -R 'fromjson? | {kind, node_id: (.node_id // .refs.node_id // .parent_identity.node_id), runtime_id: (.runtime_id // null), transition_id: (.transition_id // null), phase: (.phase // .state // null)}' ~/.ploke-eval/campaigns/<campaign>/prototype1/transition-journal.jsonl | tail -n 40
```

### Parent Identity

- Path pattern: `<active-parent-worktree>/.ploke/prototype1/parent_identity.json`.
- Type/schema: `ParentIdentity`, schema `"prototype1-parent-identity.v1"` (`crates/ploke-eval/src/cli/prototype1_state/identity.rs:16`, `:22`).
- Writers: `write_parent_identity` writes pretty JSON (`crates/ploke-eval/src/cli/prototype1_state/identity.rs:140`); setup writes and commits it (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:198`); successor/parent preparation also writes it through process helpers.
- Readers/CLI: `load_parent_identity` and optional load (`crates/ploke-eval/src/cli/prototype1_state/identity.rs:116`); `prototype1-state` resolves identity from active checkout and validates it against scheduler node and checkout (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5157`; `crates/ploke-eval/src/cli/prototype1_state/parent.rs:374`).
- Join IDs: `campaign_id`, `parent_id`, `node_id`, `generation`, `previous_parent_id`, `parent_node_id`, `branch_id`, `artifact_branch`.
- Evidence status: artifact-carried identity witness committed into the checkout; not yet a full artifact-local provenance manifest.
- Safe inspection:

```bash
jq '{schema_version, campaign_id, parent_id, node_id, generation, previous_parent_id, parent_node_id, branch_id, artifact_branch, created_at}' <active-parent-worktree>/.ploke/prototype1/parent_identity.json
```

### Node Record

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/node.json`.
- Type/schema: `Prototype1NodeRecord`, schema `"prototype1-treatment-node.v1"`; durable node summary with workspace, binary, runner request/result paths (`crates/ploke-eval/src/intervention/scheduler.rs:75`).
- Writers: `save_node_record` writes `node.json` (`crates/ploke-eval/src/intervention/scheduler.rs:386`); called during root/treatment registration and status/workspace updates.
- Readers/CLI: `load_node_record` (`crates/ploke-eval/src/intervention/scheduler.rs:452`); parent validation, child plan validation, runner command, successor handoff, monitor report/history-preview.
- Join IDs: `node_id`, `parent_node_id`, `generation`, `instance_id`, `source_state_id`, `branch_id`, `candidate_id`, `parent_branch_id`, `runner_request_path`, `runner_result_path`.
- Evidence status: scheduler-owned durable projection. It is operationally required, but it can duplicate scheduler state and is not sealed.
- Safe inspection:

```bash
jq '{node_id, parent_node_id, generation, instance_id, source_state_id, branch_id, candidate_id, status, workspace_root, binary_path, runner_request_path, runner_result_path}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/node.json
```

### Node Runner Request

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-request.json`.
- Type/schema: `Prototype1RunnerRequest`, schema `"prototype1-treatment-node.v1"` (`crates/ploke-eval/src/intervention/scheduler.rs:132`).
- Writers: `save_runner_request` (`crates/ploke-eval/src/intervention/scheduler.rs:413`); root registration creates a `prototype1-state` request, treatment registration creates a `prototype1-runner` request, workspace updates rewrite it.
- Readers/CLI: `load_runner_request` (`crates/ploke-eval/src/intervention/scheduler.rs:464`); child invocation execution, legacy runner-node execution, runner inspection, child plan file checks.
- Join IDs: `campaign_id`, `node_id`, `generation`, `instance_id`, `source_state_id`, `branch_id`, `workspace_root`, `binary_path`, `runner_args`.
- Evidence status: admitted-preview raw execution contract for the node-level runner seam; mutable at node registration/workspace update.
- Safe inspection:

```bash
jq '{campaign_id, node_id, generation, instance_id, source_state_id, branch_id, workspace_root, binary_path, stop_on_error, runner_args}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-request.json
```

### Latest Node Runner Result

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-result.json`.
- Type/schema: `Prototype1RunnerResult`, schema `"prototype1-treatment-node.v1"` (`crates/ploke-eval/src/intervention/scheduler.rs:107`).
- Writers: `record_runner_result` writes latest result, then updates node/scheduler status (`crates/ploke-eval/src/intervention/scheduler.rs:600`). Child attempt completion writes the attempt result first and then the latest projection (`crates/ploke-eval/src/cli/prototype1_process.rs:1595`).
- Readers/CLI: `load_runner_result`/`load_runner_result_at` (`crates/ploke-eval/src/intervention/scheduler.rs:476`); `loop prototype1-runner`; monitor/history-preview.
- Join IDs: `campaign_id`, `node_id`, `generation`, `branch_id`, `treatment_campaign_id`, `evaluation_artifact_path`.
- Evidence status: latest projection/cache unless no attempt-scoped result exists; attempt result is a better runtime-attempt join.
- Safe inspection:

```bash
jq '{campaign_id, node_id, generation, branch_id, status, disposition, treatment_campaign_id, evaluation_artifact_path, detail, exit_code, recorded_at}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-result.json
```

### Attempt Invocation

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/invocations/<runtime-id>.json`.
- Type/schema: `Invocation`, schema `"prototype1-invocation.v1"`, role `child` or `successor` (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:80`, `:93`).
- Writers: `write_child_invocation` writes child invocations before spawn (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:373`; `crates/ploke-eval/src/cli/prototype1_state/c3.rs:530`). `write_successor_invocation_for_retired_parent` writes successor invocations after predecessor retirement (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:389`; `crates/ploke-eval/src/cli/prototype1_process.rs:950`).
- Readers/CLI: `loop prototype1-runner --invocation <path>` loads child invocation for execution or inspection; successor invocations are loaded by `loop prototype1-state --handoff-invocation <path>` and rejected by runner execution (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1347`).
- Join IDs: `campaign_id`, `node_id`, `runtime_id`, `journal_path`, optional `active_parent_root`, `role`.
- Evidence status: admitted-preview raw bootstrap contract; the successor file is gated by retired parent construction, but the JSON itself is not Crown authority.
- Safe inspection:

```bash
jq '{schema_version, role, campaign_id, node_id, runtime_id, journal_path, active_parent_root, created_at}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/invocations/<runtime-id>.json
```

### Attempt Result

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/results/<runtime-id>.json`.
- Type/schema: `Prototype1RunnerResult`; same schema as latest runner result.
- Writers: `record_attempt_runner_result` writes attempt result via `write_runner_result_at`, then writes latest runner-result projection (`crates/ploke-eval/src/cli/prototype1_process.rs:1595`).
- Readers/CLI: `ObserveChild` discovers attempt result through `Child<ResultWritten>` journal records and loads it (`crates/ploke-eval/src/cli/prototype1_state/c4.rs:227`, `:304`); history-preview imports `AttemptResult`.
- Join IDs: `runtime_id` from filename and child journal, plus `campaign_id`, `node_id`, `branch_id`, `evaluation_artifact_path`.
- Evidence status: attempt-scoped raw result; more authoritative for a concrete runtime attempt than latest `runner-result.json`.
- Safe inspection:

```bash
jq '{campaign_id, node_id, generation, branch_id, status, disposition, treatment_campaign_id, evaluation_artifact_path, detail, exit_code, recorded_at}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/results/<runtime-id>.json
```

### Child Plan Message

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/messages/child-plan/<parent-node-id>.json`.
- Type/schema: `ChildPlanFiles` message body; contains message path, scheduler path, branch registry path, parent node id, child generation, and child node/request file addresses (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:96`).
- Writers: `write_child_plan_file` writes pretty JSON after `Open<ChildPlan>::lock` validation (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:408`, `:481`).
- Readers/CLI: `receive_existing_child_plan` uses `Locked<ChildPlan>::from_box` and `read_child_plan_message` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:430`, `:491`); `validate_received_child_plan` ensures the selected node is included.
- Join IDs: `parent_node_id`, `child_generation`, child `node_id`, scheduler/branch/node/request paths.
- Evidence status: typed cross-runtime message/projection of a parent planning step; it binds file addresses but does not seal their contents.
- Safe inspection:

```bash
jq '{parent_node_id, child_generation, scheduler, branches, children: [.children[] | {node_id, node, runner_request}]}' ~/.ploke-eval/campaigns/<campaign>/prototype1/messages/child-plan/<parent-node-id>.json
```

### Evaluation Report

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/evaluations/<branch-id>.json`.
- Type/schema: `Prototype1BranchEvaluationReport` with compared baseline/treatment instances, metrics, branch disposition, and artifact paths (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6421`).
- Writers: child evaluation builds the report and writes it directly with `write_json_file_pretty` (`crates/ploke-eval/src/cli/prototype1_process.rs:1872`, `:1891`). It also writes a compact `TreatmentBranchEvaluationSummary` back to `branches.json`.
- Readers/CLI: parent `ObserveChild` loads the report from `runner_result.evaluation_artifact_path` (`crates/ploke-eval/src/cli/prototype1_state/c4.rs:374`); monitor report loads evaluation dir; branch evaluation CLI prints it.
- Join IDs: `baseline_campaign_id`, `branch_id`, `treatment_campaign_id`, `branch_registry_path`, `evaluation_artifact_path`, compared `instance_id`.
- Evidence status: admitted-preview raw comparison artifact; its source metrics point to baseline/treatment run records outside loop-native state.
- Safe inspection:

```bash
jq '{baseline_campaign_id, branch_id, treatment_campaign_id, overall_disposition, reasons_count: (.reasons|length), compared: [.compared_instances[] | {instance_id, status, has_baseline: (.baseline_metrics != null), has_treatment: (.treatment_metrics != null), disposition: .evaluation.disposition}]}' ~/.ploke-eval/campaigns/<campaign>/prototype1/evaluations/<branch-id>.json
```

### Successor Ready

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/successor-ready/<runtime-id>.json`.
- Type/schema: `SuccessorReadyRecord`, schema `"prototype1-successor-ready.v1"` (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:402`).
- Writers: successor runtime calls `record_prototype1_successor_ready`, writes the ready file, and appends a `SuccessorRecord::ready` journal entry (`crates/ploke-eval/src/cli/prototype1_process.rs:332`).
- Readers/CLI: predecessor waits for this path and loads it before acknowledging handoff (`crates/ploke-eval/src/cli/prototype1_process.rs:1065`); monitor/history-preview import it.
- Join IDs: `campaign_id`, `node_id`, `runtime_id`, `pid`.
- Evidence status: successor acknowledgement; history-preview classifies it as raw or ingress because it can be written by the detached successor after predecessor handoff.
- Safe inspection:

```bash
jq '{schema_version, campaign_id, node_id, runtime_id, pid, recorded_at}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/successor-ready/<runtime-id>.json
```

### Successor Completion

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/successor-completion/<runtime-id>.json`.
- Type/schema: `SuccessorCompletionRecord`, schema `"prototype1-successor-completion.v1"` (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:416`).
- Writers: `record_prototype1_successor_completion` writes completion and appends a successor completion journal record (`crates/ploke-eval/src/cli/prototype1_process.rs:359`). `prototype1-state` writes success completion at the end of a handoff invocation (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5567`) or failure completion from `record_failed_successor_turn`.
- Readers/CLI: monitor/history-preview; predecessor-visible terminal record for bounded successor rehydration.
- Join IDs: `campaign_id`, `node_id`, `runtime_id`, `status`, optional `trace_path`.
- Evidence status: attempt-scoped successor terminal record; raw/admitted-preview, not final Crown authority.
- Safe inspection:

```bash
jq '{schema_version, campaign_id, node_id, runtime_id, status, trace_path, detail, recorded_at}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/successor-completion/<runtime-id>.json
```

### Loop Controller Trace

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1-loop-trace.json` (campaign root, not inside `prototype1/`).
- Type/schema: `Prototype1LoopReport` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6236`) written by `run_prototype1_loop_controller`.
- Writers: `prototype1_trace_path` constructs the path (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6908`); controller overwrites it with `write_json_file_pretty` (`:1075`).
- Readers/CLI: printed by `loop prototype1` and used as a path in successor completion; monitor labels it as a legacy loop controller trace.
- Join IDs: `campaign_id`, `branch_registry_path`, `scheduler_path`, `trace_path`, staged `node_id`s, selected `branch_id`.
- Evidence status: overwritten trace/projection, useful for operator visibility but not authority.
- Safe inspection:

```bash
jq '{stage_reached, dry_run, campaign_id, branch_registry_path, scheduler_path, trace_path, staged_nodes: [.staged_nodes[] | {node_id, generation, branch_id, status}], selected_next_branch_id, continuation_decision}' ~/.ploke-eval/campaigns/<campaign>/prototype1-loop-trace.json
```

### History Block Store

- Path patterns:
  - `~/.ploke-eval/campaigns/<campaign>/prototype1/history/blocks/segment-000000.jsonl`
  - `~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/by-hash.jsonl`
  - `~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/by-lineage-height.jsonl`
  - `~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/heads.json`
- Type/schema: filesystem `FsBlockStore` storing sealed `Block<block::Sealed>` plus `StoredBlock`, `LineageHeight`, and heads map (`crates/ploke-eval/src/cli/prototype1_state/history.rs:620`).
- Writers: successor handoff seals a minimal History block and appends it before spawning the successor (`crates/ploke-eval/src/cli/prototype1_process.rs:1098`, `:1176`). `FsBlockStore::append` appends block and index JSONL and rewrites `heads.json` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:831`, `:861`).
- Readers/CLI: parent startup checks genesis/predecessor lineage through `FsBlockStore::for_campaign_manifest` (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:443`, `:500`); history preview/metrics.
- Join IDs: `lineage_id` (currently campaign id), `block_hash`, `block_height`, predecessor head, successor `runtime_id` evidence refs.
- Evidence status: sealed local History block store, but current docs/code still caution that it is not distributed consensus, process uniqueness, or full Crown authority for all projections.
- Safe inspection:

```bash
find ~/.ploke-eval/campaigns/<campaign>/prototype1/history -maxdepth 3 -type f -printf '%s %p\n' | sort -n
jq '{heads: .}' ~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/heads.json
tail -n 5 ~/.ploke-eval/campaigns/<campaign>/prototype1/history/index/by-lineage-height.jsonl | jq -R 'fromjson? | {lineage_id, block_height, block_hash}'
```

### Runtime Streams

- Path pattern: `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stdout.log` and `stderr.log`.
- Type/schema: unstructured process stdout/stderr logs.
- Writers: child spawn creates streams in `c3::open_streams` (`crates/ploke-eval/src/cli/prototype1_state/c3.rs:123`, `:135`); successor spawn creates runtime streams in `prototype1_process::open_runtime_streams` (`crates/ploke-eval/src/cli/prototype1_process.rs:1024`, `:1035`).
- Readers/CLI: journal `SpawnEntry`/`SuccessorHandoffEntry` records stream paths; monitor timing/evidence parsing may collect named logs.
- Join IDs: `runtime_id`, `node_id`, journal `streams`.
- Evidence status: diagnostic artifact, not structured authority.
- Safe inspection:

```bash
find ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams -maxdepth 3 -type f -printf '%s %p\n' | sort -n
tail -n 40 ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log
```

### Node Workspace And Build Artifacts

- Path patterns:
  - `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/worktree/`
  - `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/bin/ploke-eval`
  - `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/target/`
- Type/schema: filesystem worktree and build outputs, not JSON schema.
- Writers: staging/build/materialization path updates node workspace and binary path through scheduler and backend; live path calls `persist_prototype1_buildable_child_artifact` after `BuildChild` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5281`).
- Readers/CLI: runner request and node record carry `workspace_root`/`binary_path`; `SpawnChild` validates the binary path against the request before spawning (`crates/ploke-eval/src/cli/prototype1_state/c3.rs:504`).
- Join IDs: `node_id`, `branch_id`, `workspace_root`, `binary_path`, optional artifact commit/history refs.
- Evidence status: mutable runtime/build artifacts; useful runtime evidence, not authoritative scheduler state.
- Safe inspection:

```bash
find ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id> -maxdepth 2 \( -path '*/worktree' -o -path '*/bin' -o -path '*/target' \) -printf '%y %p\n'
```

### Observation JSONL

- Path pattern: default `~/.ploke-eval/logs/prototype1_observation_<run-id>.jsonl`, or explicit path from `PLOKE_PROTOTYPE1_TRACE_JSONL`.
- Type/schema: tracing JSONL events, not a single domain schema.
- Writers: tracing setup enables this only when `PLOKE_PROTOTYPE1_TRACE_JSONL` is nonempty; `auto/default/1/true` chooses the default logs path (`crates/ploke-eval/src/tracing_setup.rs:179`, `:206`).
- Readers/CLI: monitor timing/evidence collectors scan `prototype1_observation_*.jsonl` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3604`).
- Join IDs: event span fields commonly include `campaign_id`, `node_id`, `branch_id`, `generation`, `runtime_id`, `request_id`.
- Evidence status: operational telemetry; not authoritative.
- Safe inspection:

```bash
tail -n 80 ~/.ploke-eval/logs/prototype1_observation_<run-id>.jsonl | jq -R 'fromjson? | {timestamp, target, event, span, spans}'
```

## Reader Surfaces

- `ploke-eval loop prototype1-state --repo-root . [--handoff-invocation <path>]`: live parent/successor path. It reads parent identity, scheduler, branch registry, child plan, node/request/result files, transition journal, and History state. This command advances state; do not use it as inspection.
- `ploke-eval loop prototype1-runner --invocation <path>`: inspects an invocation; with `--execute`, executes a child invocation and writes attempt/latest runner results.
- `ploke-eval loop prototype1-runner --campaign <campaign> --node-id <node>`: inspects a node/request/result; with `--execute`, legacy node runner execution path.
- `ploke-eval loop prototype1-branch status/show`: reads branch registry without changing it. `apply` mutates checkout and registry.
- `ploke-eval loop prototype1-monitor list/report/watch/peek/timing/history-metrics/history-preview`: read-only operator surfaces. Prefer `report` and `history-preview` over raw artifact reads.
- `ploke-eval history ...`: adjacent History projection/inspection path, not the live state loop itself.

## Gaps And Unknowns

- Scheduler and node records duplicate state. The source treats scheduler as projection-only and node records as projection/degraded evidence; there is no sealed invariant that they cannot drift.
- Branch registry preserves useful branch/content/evaluation state, but whole-artifact provenance is still incomplete. Several ids are text-file or branch-derived fallbacks.
- Parent identity is committed into the artifact checkout and validated, but it is not a full artifact-local provenance manifest.
- Transition journal is append-only and synced, but not cryptographically sealed. Legacy variants still flatten semantic state into storage labels.
- History block store exists and is written during successor handoff, but current implementation does not prove distributed consensus, process uniqueness, or global canonical state.
- Attempt result is stronger than latest runner result, but monitor/report readers still consume both; exact precedence should be kept explicit in future recovery code.
- `prototype1-loop-trace.json` is overwritten per controller run and should be treated as a convenience projection.
- Observation JSONL and stream logs are useful telemetry but are not loop authority.
- Evaluation reports include operational metrics copied from baseline/treatment run records; those source records are outside loop-native state and should be separately mapped by another worker.
- Child-plan files bind expected paths but not content digests. A receiver reads the named files to prove presence, not integrity.
- Cleanup/retention policy for `nodes/<node-id>/worktree`, `bin`, `target`, `streams`, and old invocation/result attempts is unclear from this pass.
