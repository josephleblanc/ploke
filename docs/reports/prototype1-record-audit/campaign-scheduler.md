# Prototype 1 Campaign/Scheduler Record Audit

Scope: targeted read of `crates/ploke-eval/src/intervention/**`, `crates/ploke-eval/src/campaign.rs`, and adjacent `ploke-eval` persistence code used by campaign, scheduler, branch, registry, and node records.

Path shorthand: `$EVAL_HOME` means `PLOKE_EVAL_HOME` or `~/.ploke-eval`; `$C` means `$EVAL_HOME/campaigns/<campaign-id>`.

## Campaign And Registry Records

### `crate::campaign::CampaignManifest`

- Rust type/path: `CampaignManifest`, `EvalCampaignPolicy`, `ProtocolCampaignPolicy` in `crates/ploke-eval/src/campaign.rs`.
- Persisted path(s): `$C/campaign.json`.
- Writer(s): `save_campaign_manifest`; CLI campaign init/show/update paths; `prepare_prototype1_treatment_campaign` creates branch treatment campaigns.
- Reader(s): `load_campaign_manifest`, `resolve_campaign_config`, `list_campaigns`, Prototype 1 loop setup/evaluation helpers.
- Semantic role: campaign-level configuration: dataset sources, model/provider, roots, eval/protocol policy.
- Duplicate/overlapping facts: overlaps `ClosureState.config` and `TargetRegistry.dataset_sources`; treatment campaign manifests copy baseline manifest fields and then rewrite roots.
- Gaps/risks: manifest can drift from `closure-state.json`; no durable reference to the registry snapshot/digest that produced a campaign.
- Cleanup recommendation: make campaign manifest the input config and persist only references/digests to registry and closure snapshots; avoid adopting closure config back into a new editable manifest without recording provenance.

### `crate::closure::ClosureState`

- Rust type/path: `ClosureState`, `ClosureConfig`, `ClosureInstanceRow` family in `crates/ploke-eval/src/closure.rs`.
- Persisted path(s): `$C/closure-state.json`.
- Writer(s): `recompute_closure_state`.
- Reader(s): `load_closure_state`; `adopt_campaign_manifest_from_closure_state`; branch evaluation compares baseline/treatment closure states.
- Semantic role: campaign closure snapshot: resolved config plus registry/eval/protocol instance status.
- Duplicate/overlapping facts: repeats campaign config, dataset sources, roots, model/provider, and per-instance record paths also available through run registries/records.
- Gaps/risks: closure state is a computed snapshot but is also used as a config source; stale closure state can seed a new manifest.
- Cleanup recommendation: treat closure state as derived output with an input manifest id/digest; do not use it as a peer source of campaign truth.

### `crate::target_registry::TargetRegistry`

- Rust type/path: `TargetRegistry`, `RegistryDatasetSource`, `RegistryEntry`, `RegistrySource`, `RegistryEntryState` in `crates/ploke-eval/src/target_registry.rs`.
- Persisted path(s): `$EVAL_HOME/registries/<benchmark-family>.json`, currently `multi-swe-bench-rust.json`.
- Writer(s): `recompute_target_registry`.
- Reader(s): `load_target_registry`; campaign init/validation; closure recompute.
- Semantic role: benchmark universe and instance-to-repo mapping.
- Duplicate/overlapping facts: `dataset_sources` are copied into `CampaignManifest` and `ClosureState.config`; instance/source facts are projected into closure rows.
- Gaps/risks: campaign state does not identify the exact registry content used except by copied fields.
- Cleanup recommendation: let campaigns reference a registry snapshot by benchmark family plus content digest/version; keep dataset source copies only as human-readable projection.

## Branch Registry Records

### `crate::intervention::branch_registry::Prototype1BranchRegistry`

- Rust type/path: `Prototype1BranchRegistry` in `crates/ploke-eval/src/intervention/branch_registry.rs`.
- Persisted path(s): `$C/prototype1/branches.json`.
- Writer(s): `save_branch_registry`, via `record_synthesized_branches`, `mark_treatment_branch_applied`, `select_treatment_branch`, `restore_treatment_branch`, `record_treatment_branch_evaluation`.
- Reader(s): `load_or_default_branch_registry`, `resolve_treatment_branch`, `active_branch_selection_for_target`, branch status/show/apply/evaluate commands, scheduler node registration, runnable-node inference.
- Semantic role: mutable campaign-local branch registry for source states, branch proposals, selected active targets, apply state, and latest evaluation summaries.
- Duplicate/overlapping facts: contains source nodes, branches, active target selection, and evaluation summary in one mutable document; selected branch appears both as `InterventionSourceNode.selected_branch_id` and `ActiveInterventionTarget.active_branch_id`.
- Gaps/risks: broad mutable aggregate makes unrelated facts overwrite together; no append-only transition history for branch selection/apply/evaluation.
- Cleanup recommendation: split into typed records: source artifact, branch proposal, selection, apply result, evaluation summary. Keep `branches.json` as a projection if needed.

### `InterventionSourceNode`

- Rust type/path: `InterventionSourceNode` in `branch_registry.rs`.
- Persisted path(s): embedded in `$C/prototype1/branches.json`.
- Writer(s): `record_synthesized_branches`; later refreshed by apply/select paths.
- Reader(s): branch resolution/status/reporting; scheduler generation calculation; candidate node inference.
- Semantic role: source state for one target file and instance, with candidate branches below it.
- Duplicate/overlapping facts: repeats `instance_id`, `source_state_id`, `target_relpath`, source content/hash, source artifact id, and parent branch relation used later by scheduler nodes.
- Gaps/risks: `parent_branch_id` is optional and can be patched by later synthesis; source content is stored inline rather than addressed by artifact.
- Cleanup recommendation: make source state an artifact-addressed record; store content inline only as cache/debug projection with hash validation.

### `TreatmentBranchNode`

- Rust type/path: `TreatmentBranchNode`, `TreatmentBranchStatus`, `TreatmentBranchEvaluationSummary` in `branch_registry.rs`.
- Persisted path(s): embedded in `$C/prototype1/branches.json`.
- Writer(s): `record_synthesized_branches`, `mark_treatment_branch_applied`, `select_treatment_branch`, `restore_treatment_branch`, `record_treatment_branch_evaluation`.
- Reader(s): `resolve_treatment_branch`, branch status/show/apply/evaluate, scheduler node registration.
- Semantic role: one synthesized branch/proposed patch from a source state.
- Duplicate/overlapping facts: `branch_id`, `candidate_id`, `patch_id`, target/provenance, proposed/applied hashes, status, apply id, and latest evaluation overlap with scheduler node records, runner results, and evaluation artifacts.
- Gaps/risks: status is a public mutable enum in the registry, not a transition-owned projection; latest evaluation loses history.
- Cleanup recommendation: preserve `Branch<Proposed/Selected/Applied/Evaluated>` structurally; have transition methods append records and update a projection.

### `ActiveInterventionTarget`

- Rust type/path: `ActiveInterventionTarget` in `branch_registry.rs`.
- Persisted path(s): embedded in `$C/prototype1/branches.json`.
- Writer(s): `mark_treatment_branch_applied`, `select_treatment_branch`, `restore_treatment_branch`.
- Reader(s): `active_branch_selection_for_target`, branch status/reporting.
- Semantic role: current selected/applied branch for a target file.
- Duplicate/overlapping facts: duplicates `selected_branch_id`, source/patch/apply/derived artifact ids from source and branch records.
- Gaps/risks: target-level active state can disagree with source-node selection or branch status.
- Cleanup recommendation: make active target a derived view from a single selection/apply transition record keyed by target and branch.

## Scheduler And Node Records

### `crate::intervention::scheduler::Prototype1SchedulerState`

- Rust type/path: `Prototype1SchedulerState`, `Prototype1SearchPolicy`, `Prototype1ContinuationDecision` in `crates/ploke-eval/src/intervention/scheduler.rs`.
- Persisted path(s): `$C/prototype1/scheduler.json`.
- Writer(s): `save_scheduler_state`, via `register_root_parent_node`, `register_treatment_evaluation_node`, `load_or_register_treatment_evaluation_node`, `update_scheduler_policy`, `update_node_status`, `update_node_workspace_root`, `record_runner_result`, `record_continuation_decision`.
- Reader(s): `load_scheduler_state`, `load_or_default_scheduler_state`, Prototype 1 runner report, monitor terminal-state logic, runnable-node inference, successor-continuation validation.
- Semantic role: campaign search policy, frontier/completed/failed node ids, continuation decision, and node summaries.
- Duplicate/overlapping facts: embeds full `Prototype1NodeRecord` values that are also stored as per-node `node.json`; frontier/completed/failed sets duplicate `node.status`; continuation selected branch duplicates branch evaluation/selection state.
- Gaps/risks: no controller lease/epoch; no first-class successor decision separate from branch evaluation; node list can diverge from node files.
- Cleanup recommendation: scheduler should own policy, queues, and decision records by node id; keep node records authoritative in per-node files or generate scheduler projection atomically.

### `Prototype1NodeRecord`

- Rust type/path: `Prototype1NodeRecord`, `Prototype1NodeStatus` in `scheduler.rs`.
- Persisted path(s): `$C/prototype1/nodes/<node-id>/node.json`; also mirrored in `scheduler.json.nodes`.
- Writer(s): `register_root_parent_node`, `register_treatment_evaluation_node`, `save_node_record`, `update_node_status`, `update_node_workspace_root`, `record_runner_result`.
- Reader(s): `load_node_record`; typed `c1`-`c4` transitions; `prototype1_process`; parent identity validation; runner and monitor commands.
- Semantic role: durable candidate node for a branch/generation evaluation.
- Duplicate/overlapping facts: repeats branch registry facts (`branch_id`, `candidate_id`, `source_state_id`, `target_relpath`, provenance ids); repeats request paths and workspace/binary paths from `Prototype1RunnerRequest`.
- Gaps/risks: mutable status writes are public scheduler helpers; `parent_node_id` is derived from branch id/generation and can diverge from actual parent identity.
- Cleanup recommendation: model node lifecycle as `Node<Planned/Staged/Built/Running/Terminal>` with private state transitions; persist node identity separately from volatile execution paths.

### `Prototype1RunnerRequest`

- Rust type/path: `Prototype1RunnerRequest` in `scheduler.rs`.
- Persisted path(s): `$C/prototype1/nodes/<node-id>/runner-request.json`.
- Writer(s): `register_root_parent_node`, `register_treatment_evaluation_node`, `save_runner_request`, `update_node_workspace_root`.
- Reader(s): `load_runner_request`; runner command; child process execution; spawn/build transitions.
- Semantic role: mutable launch contract for one node runner.
- Duplicate/overlapping facts: repeats node identity, branch id, source state, target path, provenance ids, workspace root, binary path.
- Gaps/risks: it is node-scoped, but process execution is attempt-scoped; request mutation can rewrite launch context after node creation.
- Cleanup recommendation: derive stable request fields from `Prototype1NodeRecord`; move runtime-specific launch fields into attempt-scoped invocation records.

### `Prototype1RunnerResult`

- Rust type/path: `Prototype1RunnerResult`, `Prototype1RunnerDisposition` in `scheduler.rs`.
- Persisted path(s): latest `$C/prototype1/nodes/<node-id>/runner-result.json`; attempt-scoped `$C/prototype1/nodes/<node-id>/results/<runtime-id>.json`.
- Writer(s): `record_runner_result`, `write_runner_result_at`, `record_attempt_runner_result`; build/treatment failure builders in `prototype1_process.rs`.
- Reader(s): `load_runner_result`, `load_runner_result_at`, runner report, parent-side child observation (`c4`), child execution supervisor.
- Semantic role: terminal child/node execution outcome.
- Duplicate/overlapping facts: duplicates node status, branch id/generation, evaluation artifact path, and some evaluation disposition detail.
- Gaps/risks: latest result is overwritten/cleared while attempt result is retained only on some paths; compile failures currently write latest directly.
- Cleanup recommendation: make all terminal results attempt-scoped; store latest result as a pointer/projection on node or scheduler state.

### `Prototype1BranchEvaluationReport`

- Rust type/path: `Prototype1BranchEvaluationReport`, `Prototype1ComparedInstanceReport` in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`.
- Persisted path(s): `$C/prototype1/evaluations/<branch-id>.json`.
- Writer(s): `run_prototype1_branch_evaluation` builds and writes with `write_json_file_pretty`.
- Reader(s): `load_prototype1_branch_evaluation_report`, parent/child process outcome readers, `c4` observe transition.
- Semantic role: full branch-vs-baseline evaluation artifact.
- Duplicate/overlapping facts: summary is copied into `TreatmentBranchNode.latest_evaluation`; result path is copied into `Prototype1RunnerResult`.
- Gaps/risks: report is branch-id addressed, so repeated evaluations can replace prior results without attempt identity.
- Cleanup recommendation: key evaluations by branch plus evaluation attempt id; store latest summary as a projection with artifact path and digest.

## Adjacent Runtime/Journal Records

### `ParentIdentity`

- Rust type/path: `ParentIdentity` in `crates/ploke-eval/src/cli/prototype1_state/identity.rs`.
- Persisted path(s): checkout-local `.ploke/prototype1/parent_identity.json`.
- Writer(s): `write_parent_identity`; initialized from `ParentIdentity::from_node`; committed by parent identity and child artifact persistence paths.
- Reader(s): `load_parent_identity`, `load_parent_identity_optional`, parent role validation, campaign/node inference.
- Semantic role: artifact-carried identity proving which node/branch/generation a hydrated parent represents.
- Duplicate/overlapping facts: repeats scheduler node id, parent node id, branch id, generation, campaign id.
- Gaps/risks: no digest linking identity back to exact `node.json` content; active checkout state can drift if identity commit and scheduler update are not treated as one transition.
- Cleanup recommendation: keep this as the artifact-carried authority record, but include node record digest and make scheduler handoff consume it through a typed `Parent<Ready>` transition.

### `Invocation`, `SuccessorReadyRecord`, `SuccessorCompletionRecord`

- Rust type/path: `Invocation`, `Role`, `SuccessorReadyRecord`, `SuccessorCompletionRecord` in `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`.
- Persisted path(s): `$C/prototype1/nodes/<node-id>/invocations/<runtime-id>.json`, `successor-ready/<runtime-id>.json`, `successor-completion/<runtime-id>.json`.
- Writer(s): `write_child_invocation`, `write_successor_invocation`, `write_successor_ready_record`, `write_successor_completion_record`; process handoff helpers.
- Reader(s): `load_executable`, `load_authority`, successor validation/monitoring.
- Semantic role: attempt-scoped runtime authority and successor acknowledgement/completion.
- Duplicate/overlapping facts: repeats campaign/node/runtime ids already present in node, scheduler, journal, and runner result.
- Gaps/risks: invocation is attempt-scoped but runner request remains node-scoped; ready/completion also appear in typed successor journal records.
- Cleanup recommendation: promote invocation to the primary attempt record and make ready/completion files projections or journal-backed materialized views.

### `PrototypeJournal` / `JournalEntry`

- Rust type/path: `PrototypeJournal`, `JournalEntry`, `Entry`, `BuildEntry`, `SpawnEntry`, `ReadyEntry`, `CompletionEntry`, parent/child/successor entries in `crates/ploke-eval/src/cli/prototype1_state/journal.rs`, plus `child.rs` and `successor.rs`.
- Persisted path(s): `$C/prototype1/transition-journal.jsonl`.
- Writer(s): `PrototypeJournal::append`; typed `c1`-`c4`; child `Child<State>` transitions; successor handoff helpers; parent-started and artifact/checkout records.
- Reader(s): `load_entries`, replay helpers, monitor display, child observation helpers.
- Semantic role: append-only typed transition stream.
- Duplicate/overlapping facts: repeats node/branch/path/world/hash facts from branch registry, scheduler, runner request/result, parent identity, and invocation files.
- Gaps/risks: old mutable controller path and new journal path both record overlapping lifecycle facts; not all mutable writes have journal counterparts.
- Cleanup recommendation: make journal entries the durable transition source for lifecycle changes; keep JSON state files as replayable projections.

### `Prototype1LoopReport`

- Rust type/path: `Prototype1LoopReport` and nested report rows in `cli_facing.rs`.
- Persisted path(s): `$C/prototype1-loop-trace.json`.
- Writer(s): main `loop prototype1` path via `write_json_file_pretty`.
- Reader(s): monitor/peek surfaces display it as a legacy loop trace.
- Semantic role: operator trace for the legacy controller run.
- Duplicate/overlapping facts: collects campaign manifest paths, closure state path, branch registry path, scheduler path, selected targets, staged nodes, branch evaluation summaries, and continuation decision.
- Gaps/risks: overwritten per run and not a durable source of truth; tempting because it is convenient and denormalized.
- Cleanup recommendation: keep only as diagnostic output; do not let cleanup code depend on it for recovery.

## Main Cleanup Direction

The current surface has three competing authorities: mutable aggregate JSON (`branches.json`, `scheduler.json`), per-node/latest files, and the append-only typed journal. Cleanup should choose a single transition authority:

- Campaign/registry/closure records should be config/snapshot artifacts with explicit digest references.
- Branch/source/node records should preserve role/state structure: `Branch<State>`, `Node<State>`, `Child<State>`, `Parent<State>`, not public status mutation helpers.
- Scheduler should schedule node ids and decisions, not mirror whole node records.
- Runtime attempts should be first-class records keyed by `runtime_id`; latest files should be projections.
- Evaluation and runner outcomes should be attempt-addressed, with summary records derived from authoritative artifacts.
