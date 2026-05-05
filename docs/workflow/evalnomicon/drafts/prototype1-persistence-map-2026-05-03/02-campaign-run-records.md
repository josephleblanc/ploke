# Worker 2 - Campaign And Run-Record Persistence

Date: 2026-05-03

Scope: campaign/run-record persistence populated or consumed when the
Prototype 1 loop invokes normal eval/campaign execution. This covers the
baseline campaign created during `prototype1-setup`, treatment campaigns
created by child evaluation, instance run directories, runner request/result
artifacts, `record.json.gz`, full-response sidecars, and normal campaign
closure/export files.

This report treats CLI output as projection. Durable evidence is the typed file
that is written by the runtime or by a typed protocol/runner writer.

## Campaign Slice And Manifest

### `~/.ploke-eval/campaigns/<campaign>/slice.jsonl`

- Persisted schema/type: source dataset JSONL rows, not a dedicated Rust
  struct. Setup preserves selected rows whose `instance_id` is in the prepared
  batch.
- Writer: `write_prototype1_slice_dataset` filters the source dataset by
  `instance_id` and writes the kept JSONL to the campaign dir
  (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6179`,
  `:6203`, `:6229`). It is invoked by Prototype 1 campaign setup after
  resolving `campaign_dir.join("slice.jsonl")` (`cli_facing.rs:6121`).
- Readers/CLI: the campaign manifest stores this path as a
  `RegistryDatasetSource` (`cli_facing.rs:6128`-`:6134`). Existing Prototype 1
  campaign load falls back to `slice.jsonl` when the manifest has no dataset
  source (`cli_facing.rs:275`-`:299`). Normal `campaign show/validate` reads the
  manifest and resolves the campaign config (`crates/ploke-eval/src/cli.rs:4060`,
  `:4082`).
- Join IDs: `instance_id`; dataset label is set to
  `prototype1/<prepared_batch.batch_id>`.
- Authority: evidence for the selected campaign slice membership, but not a
  typed Prototype 1 transition record.
- Safe bounded inspection:

```bash
jq -r '.instance_id' ~/.ploke-eval/campaigns/<campaign>/slice.jsonl | sort | sed -n '1,40p'
```

### `~/.ploke-eval/campaigns/<campaign>/campaign.json`

- Persisted schema/type: `CampaignManifest`
  (`crates/ploke-eval/src/campaign.rs:25`). Key fields include `campaign_id`,
  `dataset_sources`, `model_id`, `provider_slug`, `instances_root`,
  `batches_root`, `eval`, `protocol`, and `framework`.
- Writer: normal `campaign init` saves with `save_campaign_manifest`
  (`campaign.rs:272`-`:286`; CLI path `crates/ploke-eval/src/cli.rs:3987`-`:4007`).
  Prototype 1 baseline setup writes it after configuring slice, model/provider,
  and Prototype 1-specific roots (`cli_facing.rs:6128`-`:6167`). Child
  treatment evaluation writes a treatment manifest with inherited dataset
  sources and treatment-specific roots
  (`cli_facing.rs:5663`-`:5697`).
- Readers/CLI: `load_campaign_manifest` validates the requested `campaign_id`
  matches the file (`campaign.rs:243`-`:269`). `campaign show`, `campaign
  validate`, and closure commands resolve the config from it
  (`cli.rs:4060`-`:4085`, `:4261`).
- Join IDs: `campaign_id`; `dataset_sources[*].path`; `instances_root`;
  `batches_root`; `model_id`; `provider_slug`.
- Authority: durable campaign configuration evidence.
- Safe bounded inspection:

```bash
jq '{campaign_id, model_id, provider_slug, dataset_sources, instances_root, batches_root, eval, protocol}' ~/.ploke-eval/campaigns/<campaign>/campaign.json
```

## Closure State And Campaign Closure Files

### `~/.ploke-eval/campaigns/<campaign>/closure-state.json`

- Persisted schema/type: `ClosureState` with `ClosureConfig`, summary rows, and
  per-instance `ClosureInstanceRow` plus `ClosureArtifactRefs`
  (`crates/ploke-eval/src/closure.rs:52`-`:62`, `:64`-`:81`, `:154`-`:200`).
- Writer: `recompute_closure_state` builds rows from registry entries, run
  registrations, run artifacts, and protocol artifacts; then writes pretty JSON
  (`closure.rs:250`-`:293`). `advance_eval_closure` recomputes before and after
  normal batch execution (`crates/ploke-eval/src/cli.rs:5090`-`:5158`).
  `advance_protocol_closure` does the same around protocol execution
  (`cli.rs:5170`-`:5233`). Prototype 1 baseline uses both during loop setup
  (`cli_facing.rs:744`-`:759`); child evaluation uses both for treatment
  campaigns (`crates/ploke-eval/src/cli/prototype1_process.rs:1838`-`:1858`).
- Readers/CLI: `load_closure_state` reads the file (`closure.rs:241`-`:247`).
  `closure status` displays it (`cli.rs:4280`-`:4286`). `inspect
  protocol-overview --campaign` and `inspect tool-overview --campaign` start
  from it (`cli.rs:7609`-`:7623`, `:8001`-`:8042`). `campaign
  export-submissions` reads it and exports from completed rows (`cli.rs:4109`-`:4147`).
- Join IDs: `campaign_id`; per row `instance_id`; `artifacts.record_path`;
  `artifacts.registration_path`; `artifacts.run_root`;
  `artifacts.protocol_anchor`.
- Authority: projection/cache. It is the normal campaign closure surface, but
  per-row eval/protocol status is derived from run registrations, existence of
  `record.json.gz`, status artifacts, batch failures, and protocol artifacts
  (`closure.rs:580`-`:664`, `:724`-`:833`).
- Safe bounded inspection:

```bash
jq '{campaign_id, updated_at, eval, protocol, instances: [.instances[] | {instance_id, eval_status, protocol_status, record_path: .artifacts.record_path, run_root: .artifacts.run_root}]}' ~/.ploke-eval/campaigns/<campaign>/closure-state.json
```

### `~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission*.jsonl`

- Persisted schema/type: JSONL of `MultiSweBenchSubmissionRecord` loaded from
  per-run submission artifacts. The campaign file itself is an export, not the
  primary run artifact.
- Writer: `campaign export-submissions` loads closure state, selects completed
  rows, reads each row's per-run submission artifact, and writes the campaign
  JSONL (`cli.rs:4109`-`:4142`). Default output paths are documented on the
  command (`cli.rs:2641`-`:2658`).
- Readers/CLI: external submission tooling; the repo-side command reports the
  export summary (`cli.rs:4163`-`:4188`).
- Join IDs: closure row `instance_id`; `artifacts.msb_submission`;
  `campaign_id`.
- Authority: projection/export. Stronger than raw batch aggregate for campaign
  submission collection, but still derived from closure rows and per-run
  submission artifacts.
- Safe bounded inspection:

```bash
wc -l ~/.ploke-eval/campaigns/<campaign>/multi-swe-bench-submission.jsonl
```

## Instance Directories And Run Manifests

### Baseline instance roots

- Path pattern: `~/.ploke-eval/instances/prototype1/<campaign>/<instance>/`.
  Prototype 1 baseline setup sets `instances_root` to
  `instances_dir()/prototype1/<campaign>` unless overridden
  (`cli_facing.rs:6137`-`:6144`).
- Persisted schema/type: directory carrier for `PreparedSingleRun` manifest and
  per-attempt `runs/run-*` dirs. `PreparedSingleRun` has `task_id`,
  `repo_root`, `output_dir`, `issue`, `budget`, `source`, and optional
  `campaign` context (`crates/ploke-eval/src/spec.rs:98`-`:110`).
- Writer: `advance_eval_closure` prepares batches, injects
  `PreparedCampaignContext`, and writes each run manifest
  (`cli.rs:5116`-`:5139`). `PreparedSingleRun::manifest_path` is
  `output_dir/run.json`; `write_manifest` writes the file (`spec.rs:510`-`:536`).
- Readers/CLI: runners load the run manifest before execution; closure
  recompute records `artifacts.run_manifest` if found (`closure.rs:605`-`:614`).
- Join IDs: `task_id`/`instance_id`; `campaign.campaign_id`; `output_dir`.
- Authority: run manifest is durable run input evidence. The instance directory
  itself is a storage convention.
- Safe bounded inspection:

```bash
jq '{task_id, output_dir, campaign, budget, source: .source.kind}' ~/.ploke-eval/instances/prototype1/<campaign>/<instance>/run.json
```

### Treatment instance roots

- Path pattern:
  `~/.ploke-eval/instances/prototype1/<baseline>/treatments/<branch-id>/instances/<instance>/`.
- Persisted schema/type: same `PreparedSingleRun` and run-attempt layout as
  baseline runs.
- Writer: child evaluation prepares a treatment campaign with
  `instances_root = baseline.instances_root/treatments/<branch-id>/instances`
  and `batches_root = baseline.batches_root/treatments/<branch-id>/batches`
  (`cli_facing.rs:5663`-`:5697`), then invokes normal `advance_eval_closure`
  (`prototype1_process.rs:1838`-`:1845`).
- Readers/CLI: treatment closure state points at treatment run records;
  branch-evaluation comparison reads baseline and treatment closure rows
  (`cli_facing.rs:5733`-`:5746`).
- Join IDs: baseline `campaign_id`; treatment `campaign_id`;
  `branch_id`; `instance_id`; `record_path`.
- Authority: durable normal-run evidence under a treatment-specific root.
- Safe bounded inspection:

```bash
find ~/.ploke-eval/instances/prototype1/<baseline>/treatments/<branch-id>/instances -maxdepth 3 -name record.json.gz -printf '%p\n' | sort | sed -n '1,40p'
```

## Normal Run Records And Sidecars

### `.../<instance>/runs/run-*/record.json.gz`

- Persisted schema/type: gzip JSON `RunRecord`
  (`crates/ploke-eval/src/record.rs:148`-`:171`) with schema
  `run-record.v1` (`record.rs:112`). It carries `manifest_id`, `metadata`,
  phase records, DB time-travel markers, conversation, and timing.
- Writer: normal single-agent run allocates a run dir and sets
  `record_path = run_output_dir.join("record.json.gz")`
  (`crates/ploke-eval/src/runner.rs:2085`-`:2108`). It appends turn data to the
  record (`runner.rs:2403`-`:2413`), writes execution log and other artifacts
  (`runner.rs:2499`-`:2524`), then writes the compressed record with
  `write_compressed_record` (`runner.rs:2526`-`:2534`). Non-agent single runs
  follow the same final writer (`runner.rs:1988`-`:2036`). The gzip writer and
  reader are `write_compressed_record` / `read_compressed_record`
  (`record.rs:1568`-`:1608`).
- Readers/CLI: closure classifies complete eval rows by record existence
  (`closure.rs:724`-`:739`). Branch evaluation reads baseline and treatment
  records and derives operational metrics (`cli_facing.rs:5747`-`:5774`).
  `inspect` subcommands resolve `--record` or latest registered `--instance`
  paths and read the compressed record (command docs around
  `cli.rs:3150`-`:3223`; representative reads at `cli.rs:3712`-`:3717`,
  `:8438`-`:8449`). Campaign tool/protocol overviews read row
  `artifacts.record_path` from closure state (`cli.rs:8001`-`:8042`,
  `:8438`-`:8458`).
- Join IDs: `manifest_id`; `metadata.benchmark.instance_id`;
  `metadata.run_arm`; run dir name/run id; `RunRegistration.artifacts.record_path`
  when registry is present.
- Authority: primary per-run execution evidence for normal eval behavior.
- Safe bounded inspection:

```bash
zcat <record.json.gz> | jq '{schema_version, manifest_id, instance_id: .metadata.benchmark.instance_id, run_arm: .metadata.run_arm, timing, turns: (.phases.agent_turns | length), tool_calls: ([.phases.agent_turns[]?.tool_calls[]?] | length)}'
```

### `.../<instance>/runs/run-*/llm-full-responses.jsonl`

- Persisted schema/type: JSONL of `RawFullResponseRecord`
  (`record.rs:1222`-`:1236`), with `assistant_message_id`,
  `response_index`, and normalized provider `response`.
- Writer: agent runs set `full_response_trace_path` to
  `run_output_dir.join("llm-full-responses.jsonl")` and capture the current
  tracing file offset (`runner.rs:2098`-`:2106`). After the turn completes,
  `persist_full_response_trace_slice` copies only the new slice into the run
  sidecar (`runner.rs:2381`-`:2395`, `:3739`-`:3775`). The registration and
  execution log carry the sidecar path when present (`runner.rs:2418`,
  `:2499`-`:2508`).
- Readers/CLI: `inspect` resolves the sidecar beside the record
  (`cli.rs:9868`-`:9885`) and loads records by turn or all records without
  forcing full payloads into `RunRecord` (`cli.rs:9888`-`:9936`). CLI notes that
  sidecar totals may undercount if the final stop response was not captured
  (`cli.rs:9942`, `:10029`).
- Join IDs: run dir / `record_path` parent; `assistant_message_id`;
  `response_index`.
- Authority: run-owned raw provider evidence sidecar, but with a documented
  capture gap for final-stop responses.
- Safe bounded inspection:

```bash
jq -r '[.assistant_message_id, .response_index, (.response.model // "model?"), (.response.usage.total_tokens // 0)] | @tsv' <llm-full-responses.jsonl> | sed -n '1,20p'
```

### `~/.ploke-eval/registries/runs/<run-id>.json`

- Persisted schema/type: `RunRegistration` / `RunArtifactRefs` from the inner
  registry. It is not Prototype 1-specific, but closure prefers it over
  fallback file discovery.
- Writer: runner registers a live run before execution and persists lifecycle
  changes (`runner.rs:2112`-`:2125`, `:2413`-`:2419`, `:2555`-`:2564`;
  registry helper `crates/ploke-eval/src/run_registry.rs:36`-`:48`).
- Readers/CLI: closure row construction calls
  `preferred_registration_for_instance`, then copies artifact refs into
  `ClosureArtifactRefs` (`closure.rs:580`-`:596`). Protocol artifact identity
  also resolves run identity from registration when available
  (`run_registry.rs:75`-`:104`).
- Join IDs: `run_id`; frozen `task_id`/`instance_id`;
  `artifacts.run_root`; `artifacts.record_path`; lifecycle status.
- Authority: durable registry evidence for run identity and current lifecycle,
  but execution details remain in run artifacts and `record.json.gz`.
- Safe bounded inspection:

```bash
jq '{run_id, task_id: .frozen_spec.task_id, execution_status: .lifecycle.execution_status, run_root: .artifacts.run_root, record_path: .artifacts.record_path, full_response_trace: .artifacts.full_response_trace}' ~/.ploke-eval/registries/runs/<run-id>.json
```

## Protocol Artifacts Consumed By Campaign Closure

### `.../<run-dir>/protocol-artifacts/*.json`

- Persisted schema/type: `StoredProtocolArtifact`
  (`crates/ploke-eval/src/protocol_artifacts.rs:16`-`:30`) with procedure,
  subject/run identity, model/provider, input, output, and artifact payload.
- Writer: protocol commands write artifacts via `write_protocol_artifact`, which
  resolves the run identity from `record_path`, writes under
  `protocol-artifacts`, and syncs registration protocol status
  (`protocol_artifacts.rs:248`-`:296`). Campaign protocol closure invokes
  normal protocol execution against each complete row's `record_path`
  (`cli.rs:5170`-`:5196`, `:5260`-`:5315`).
- Readers/CLI: `list_protocol_artifacts` reads/validates files for a
  `record_path` (`protocol_artifacts.rs:299`-`:334`). Closure assesses protocol
  only for complete eval rows (`closure.rs:663`-`:664`). `inspect
  protocol-overview --campaign` builds per-run summaries from closure rows and
  protocol artifacts (`cli.rs:7609`-`:7623`, `:8438`-`:8458`).
- Join IDs: `record_path`; `run_id`; `subject_id`/`instance_id`;
  `procedure_name`; `created_at_ms`.
- Authority: per-procedure protocol evidence. Campaign protocol summaries are
  projections over these artifacts plus `RunRecord` tool-call data.
- Safe bounded inspection:

```bash
find <run-dir>/protocol-artifacts -maxdepth 1 -name '*.json' -printf '%f\n' | sort | sed -n '1,40p'
```

## Prototype 1 Scheduler, Node, And Runner Request/Result

### `~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json`

- Persisted schema/type: `Prototype1SchedulerState`
  (`crates/ploke-eval/src/intervention/scheduler.rs:157`-`:174`) with
  policy, frontier/completed/failed node ids, last continuation decision, and
  node records.
- Writer: root setup registers the parent node and writes scheduler state
  (`scheduler.rs:229`-`:324`). Treatment node registration writes or updates
  scheduler state (`scheduler.rs:688`-`:833`). Status changes and runner
  results also update scheduler state (`scheduler.rs:515`-`:558`,
  `:600`-`:613`).
- Readers/CLI: monitor report loads it as a projection source
  (`crates/ploke-eval/src/cli/prototype1_state/report.rs:76`-`:101`).
  Existing Prototype 1 campaign/state flows load or default it through
  scheduler helpers (`scheduler.rs:344`-`:370`).
- Join IDs: `campaign_id`; `nodes[*].node_id`; `branch_id`; `generation`;
  `parent_node_id`.
- Authority: mutable scheduler projection. The history preview code explicitly
  classifies scheduler as `projection_only`
  (`crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:580`-`:619`).
- Safe bounded inspection:

```bash
jq '{campaign_id, policy, frontier_node_ids, completed_node_ids, failed_node_ids, nodes: [.nodes[] | {node_id, parent_node_id, generation, instance_id, branch_id, status, runner_request_path, runner_result_path}]}' ~/.ploke-eval/campaigns/<campaign>/prototype1/scheduler.json
```

### `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/node.json`

- Persisted schema/type: `Prototype1NodeRecord`
  (`scheduler.rs:75`-`:105`), including node/generation/instance/branch ids,
  graph provenance fields when available, workspace and binary paths, request
  and result paths, and node status.
- Writer: root parent registration and treatment node registration call
  `save_node_record` (`scheduler.rs:248`-`:307`, `:743`-`:814`). Status and
  workspace updates rewrite it (`scheduler.rs:515`-`:597`).
- Readers/CLI: child runners load it before execution
  (`prototype1_process.rs:1969`-`:1971`, `:2044`-`:2046`). Monitor/report views
  derive node status from scheduler and node records.
- Join IDs: `node_id`; `branch_id`; `generation`; `instance_id`;
  `runner_request_path`; `runner_result_path`.
- Authority: node-local mutable record/projection. It can be useful evidence
  for current node paths/status, but status is rewritten by transitions and is
  weaker than attempt-scoped runner result evidence.
- Safe bounded inspection:

```bash
jq '{node_id, parent_node_id, generation, instance_id, branch_id, status, workspace_root, binary_path, runner_request_path, runner_result_path}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/node.json
```

### `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-request.json`

- Persisted schema/type: `Prototype1RunnerRequest`
  (`scheduler.rs:132`-`:155`), including `campaign_id`, `node_id`,
  `generation`, `instance_id`, `source_state_id`, branch/provenance fields,
  workspace/binary paths, `stop_on_error`, and child runner argv.
- Writer: root and treatment node registration write the request
  (`scheduler.rs:278`-`:307`, `:778`-`:815`). Workspace updates rewrite it when
  the node worktree changes (`scheduler.rs:591`-`:593`).
- Readers/CLI: child runner execution loads it before running normal treatment
  eval/protocol closure (`prototype1_process.rs:1969`-`:1972`,
  `:2044`-`:2047`).
- Join IDs: `campaign_id`; `node_id`; `generation`; `instance_id`;
  `branch_id`; `source_state_id`.
- Authority: admitted preview/raw child invocation input. History preview
  classifies runner request as `admitted_preview_raw`
  (`history_preview.rs:606`-`:619`).
- Safe bounded inspection:

```bash
jq '{campaign_id, node_id, generation, instance_id, branch_id, source_state_id, workspace_root, binary_path, stop_on_error, runner_args}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-request.json
```

### `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/results/<runtime-id>.json`

- Persisted schema/type: `Prototype1RunnerResult`
  (`scheduler.rs:107`-`:130`) written at an attempt-specific result path.
- Writer: `record_attempt_runner_result` writes the attempt-scoped result first,
  then writes the latest node result projection (`prototype1_process.rs:1595`-`:1606`).
  It is used for direct child execution and invocation-based execution
  (`prototype1_process.rs:1996`-`:2008`, `:2096`-`:2106`), and also for a
  missing-result child failure fallback (`prototype1_process.rs:2261`-`:2279`).
- Readers/CLI: child lifecycle and history surfaces refer to the attempt path;
  `c4` uses invocation `result_path(&node.node_dir, runtime_id)`
  (`crates/ploke-eval/src/cli/prototype1_state/c4.rs:120`-`:128`).
- Join IDs: `runtime_id`; `campaign_id`; `node_id`; `branch_id`;
  `generation`; optional `treatment_campaign_id`; optional
  `evaluation_artifact_path`.
- Authority: attempt-scoped runner evidence. Prefer this over the latest
  `runner-result.json` when present.
- Safe bounded inspection:

```bash
jq '{campaign_id, node_id, generation, branch_id, status, disposition, treatment_campaign_id, evaluation_artifact_path, recorded_at}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/results/<runtime-id>.json
```

### `~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-result.json`

- Persisted schema/type: same `Prototype1RunnerResult`.
- Writer: `record_runner_result` writes this latest result file and then updates
  node status/scheduler (`scheduler.rs:600`-`:613`). It is called by
  `record_attempt_runner_result` after the attempt-specific write
  (`prototype1_process.rs:1595`-`:1606`).
- Readers/CLI: parent-side child execution documents that it reads back
  `runner-result.json` (`prototype1_process.rs:2125`-`:2134`). Generic
  scheduler helpers expose `load_runner_result` and `load_runner_result_at`
  (`scheduler.rs:476`-`:493`).
- Join IDs: `campaign_id`; `node_id`; `branch_id`; `generation`;
  `treatment_campaign_id`; `evaluation_artifact_path`.
- Authority: latest-result projection unless no attempt-scoped result exists.
  History preview classifies it as
  `projection_unless_attempt_result_missing` (`history_preview.rs:606`-`:619`).
- Safe bounded inspection:

```bash
jq '{campaign_id, node_id, generation, branch_id, status, disposition, treatment_campaign_id, evaluation_artifact_path, recorded_at}' ~/.ploke-eval/campaigns/<campaign>/prototype1/nodes/<node-id>/runner-result.json
```

## Branch Evaluation Report

### `~/.ploke-eval/campaigns/<baseline>/prototype1/evaluations/<branch-id>.json`

- Persisted schema/type: `Prototype1BranchEvaluationReport`
  (`cli_facing.rs:6421`-`:6433`) with compared-instance rows
  (`cli_facing.rs:6435`-`:6444`).
- Writer: child evaluation computes the path with
  `prototype1_branch_evaluation_path` (`cli_facing.rs:5713`-`:5723`), builds a
  report from baseline/treatment closure states and per-arm `RunRecord`
  operational metrics (`cli_facing.rs:5725`-`:5837`), and writes it during
  `run_prototype1_branch_evaluation` (`prototype1_process.rs:1872`-`:1897`).
  The child then records a branch-registry summary
  (`prototype1_process.rs:1909`-`:1929`).
- Readers/CLI: successful runner results include `evaluation_artifact_path`
  (`prototype1_process.rs:1570`-`:1592`). Parent-side child execution loads the
  report from that path on success (`prototype1_process.rs:2293`-`:2305`).
  Monitor report loads all JSON reports in the evaluations dir
  (`crates/ploke-eval/src/cli/prototype1_state/report.rs:76`-`:101`,
  `:625`-`:652`).
- Join IDs: baseline `campaign_id`; `branch_id`; treatment `campaign_id`;
  `evaluation_artifact_path`; compared row `instance_id`;
  `baseline_record_path`; `treatment_record_path`.
- Authority: admitted preview/raw branch evaluation evidence for the child run,
  derived from closure projections and `RunRecord` operational metrics. Its
  compared metrics are projections from record contents, not the full records.
- Safe bounded inspection:

```bash
jq '{baseline_campaign_id, branch_id, treatment_campaign_id, overall_disposition, compared: [.compared_instances[] | {instance_id, status, baseline_record_path, treatment_record_path, disposition: .evaluation.disposition}]}' ~/.ploke-eval/campaigns/<baseline>/prototype1/evaluations/<branch-id>.json
```

## Normal CLI Read Surfaces

- `campaign list/show/validate`: reads campaign dirs/manifests and resolved
  config; no run artifacts required (`cli.rs:4029`-`:4085`).
- `closure recompute/status/advance`: recompute writes
  `closure-state.json`; status reads it; advance eval/protocol both recompute
  before and after execution (`cli.rs:4259`-`:4292`, `:5090`-`:5233`).
- `inspect protocol-overview --campaign`: reads closure state, per-row
  `record.json.gz`, and protocol artifacts (`cli.rs:7385`-`:7404`,
  `:7609`-`:7623`, `:8438`-`:8458`).
- `inspect tool-overview --campaign`: reads closure state and per-row
  `record.json.gz` tool-call data (`cli.rs:8001`-`:8042`).
- `inspect ... --record/--instance`: resolves one record path and reads
  `record.json.gz`; some subcommands also read `llm-full-responses.jsonl`
  beside the record (`cli.rs:3150`-`:3223`, `:9868`-`:9936`).
- `protocol ...` and `closure advance protocol`: use `record_path` as the run
  identity anchor, write/read `protocol-artifacts/*.json`, and sync
  registration protocol refs (`protocol_artifacts.rs:248`-`:334`;
  `cli.rs:5260`-`:5315`).
- `loop prototype1-monitor report`: reads scheduler, branch registry, journal,
  and evaluation reports and presents a projection
  (`prototype1_state/report.rs:40`-`:101`).

## Join Key Summary

- Campaign: `campaign_id`.
- Dataset slice to campaign closure: `instance_id` plus manifest
  `dataset_sources[*].path`.
- Campaign config to run manifests: `campaign.campaign_id`, `model_id`,
  `provider_slug`, `instances_root`, `batches_root`.
- Instance row to run evidence: `instance_id`, `artifacts.registration_path`,
  `artifacts.run_root`, `artifacts.record_path`.
- Run record to sidecar: record path parent/run dir, `assistant_message_id`,
  `response_index`.
- Run record to protocol artifacts: `record_path`, `run_id`, `subject_id`.
- Prototype 1 node join: `node_id`, `branch_id`, `generation`, `runtime_id`.
- Branch evaluation join: baseline `campaign_id`, treatment `campaign_id`,
  `branch_id`, compared row `instance_id`, baseline/treatment `record_path`.

## Gaps And Unknowns

- `closure-state.json` is the normal campaign control surface, but it is a
  recomputed projection. Consumers should not treat closure status as stronger
  evidence than the underlying run registration, `record.json.gz`, and protocol
  artifacts.
- `scheduler.json`, `node.json`, and latest `runner-result.json` are mutable.
  Attempt-scoped `nodes/<node-id>/results/<runtime-id>.json` is the safer child
  result evidence when available.
- Full-response sidecars are run-owned evidence but may undercount final stop
  responses, per existing CLI note. The sidecar is not embedded in
  `RunRecord`.
- Branch evaluation reports store operational metrics derived from baseline and
  treatment records. They do not contain the full run record payloads and should
  be joined back to `baseline_record_path` / `treatment_record_path` for deeper
  evidence.
- Treatment campaign identity is path/id linked through branch evaluation and
  runner result records. I did not find a single typed persistence object that
  bundles baseline campaign, branch id, treatment campaign, per-arm record
  paths, and attempt runtime id as one authoritative record.
- The campaign export JSONL is an export projection over closure rows and
  per-run submissions, not the authoritative per-run submission artifact.
