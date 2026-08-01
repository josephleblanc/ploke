# Prototype 1 Run10 Records and Observability

Scope: observed artifacts for `p1-3gen-15nodes-run10`, with spot comparisons
to `p1-3gen-15nodes-run8`, `p1-3gen-15nodes-run7`, and
`p1-3gen-15nodes-run5`. I inspected file inventories and compact JSON records,
not full large traces.

## Finding

Run10 did not leave evidence of a candidate child execution. It wrote baseline
agent/protocol records, synthesized candidate branches, and staged three
candidate nodes, but the central transition journal contains only a
`parent_started` entry. There is no durable transition showing that the parent
validated the staged candidates, stopped intentionally, or failed before
materializing a child.

The observable failure is therefore not "candidate ran and failed". It is
"candidate records were staged, then no candidate-transition record explains why
execution did not proceed".

## Run10 Records Written

Campaign and baseline records:

- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/campaign.json`
  exists and declares `stop_on_error: false`.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/closure-state.json`
  reports registry, eval, and protocol as `complete`; `eval.failed_total` and
  `protocol.failed_total` are both `0`.
- `/home/brasides/.ploke-eval/batches/prototype1/p1-3gen-15nodes-run10/clap-rs-clap-3670-single-clap-rs-clap-3670-eval-slice-20260427092108/batch-run-summary.json`
  reports `instances_succeeded: 1`, `instances_failed: 0`, and instance
  `status: "completed"`.
- `/home/brasides/.ploke-eval/instances/prototype1/p1-3gen-15nodes-run10/clap-rs__clap-3670/runs/run-1777281671476-structured-current-policy-f411c44a/record.json.gz`
  exists; compressed size is 175393 bytes, uncompressed size is 1126087 bytes.
- The same run root has `execution-log.json`, `indexing-status.json`,
  `repo-state.json`, `snapshot-status.json`, `multi-swe-bench-submission.jsonl`,
  `agent-turn-summary.json`, `agent-turn-trace.json`, `llm-full-responses.jsonl`,
  and protocol artifacts under `protocol-artifacts/`.

Prototype 1 records:

- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/transition-journal.jsonl`
  has exactly one entry: `parent_started` for parent
  `node-f7b9c198a79ca49a`, generation `0`, branch
  `prototype1-parent-p1-3gen-15nodes-run10-gen0`, repo root `"."`, pid
  `1716014`.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/scheduler.json`
  has four frontier nodes and no completed or failed nodes.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/branches.json`
  has one source node for target
  `crates/ploke-core/tool_text/read_file.md`, three synthesized branches, and
  selected branch `branch-bd104387ee90926a`.
- `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1-loop-trace.json`
  says `stage_reached: "target_selection"`, `dry_run: true`, and pending stages
  `intervention apply`, `treatment arm`, and `compare`.
- Four node directories exist under
  `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/nodes/`.
  Each has `node.json` and `runner-request.json`; the candidate `bin/`
  directories are present but contain no files.

## Expected Records Missing

Compared to run7 and run5, run10 is missing the records that distinguish a
planned candidate from an executed child:

- No `runner-result.json` for any run10 candidate node, for example expected at
  `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/nodes/node-a3b258a61220d9eb/runner-result.json`.
- No per-runtime `results/<runtime-id>.json`, `invocations/<runtime-id>.json`,
  or `streams/<runtime-id>/{stdout,stderr}.log` under any run10 candidate node.
- No branch evaluation under
  `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run10/prototype1/evaluations/`.
- No `materialize_branch`, `build_child`, `child_artifact_committed`,
  `spawn_child`, `child`, `observe_child`, `successor`, or
  `active_checkout_advanced` entries in the run10 transition journal.

Run7 has the expected executed-child shape for
`node-5a024f02bea680ab`: `runner-result.json`, a per-runtime result file,
invocation files, stream logs, branch evaluation
`/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run7/prototype1/evaluations/branch-5542560e96676cea.json`,
and journal entries for materialize, build, spawn, `Child<Ready>`,
`Child<Evaluating>`, `Child<ResultWritten>`, and observe. Run5 has the same
shape for `node-4bc34fe1b649386a`.

Run8 looks closer to run10: planned candidates and no execution records. The
difference is that run10 also has a `parent_started` journal entry and staged
generation-2 candidates from a parent node that is not present in the campaign
records.

## Record Inconsistencies

The three run10 candidate nodes have:

- `generation: 2`
- `parent_node_id: "node-c47370c1ae880bf4"`
- `source_state_id: "prototype1-parent-p1-3gen-15nodes-run10-gen0"`
- `parent_branch_id: "prototype1-parent-p1-3gen-15nodes-run10-gen0"`
- `workspace_root: "."`

But `node-c47370c1ae880bf4` does not appear under
`/home/brasides/.ploke-eval/campaigns/`, is not a local git branch, and is not
present in run10's scheduler except as the parent id on those three candidates.
The only run10 parent identity artifact I found is:

`/home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-7/.ploke/prototype1/parent_identity.json`

That file identifies `node-f7b9c198a79ca49a`, generation `0`, branch
`prototype1-parent-p1-3gen-15nodes-run10-gen0`. The scheduler also keeps this
generation-0 root parent in the frontier with status `planned`.

So run10 records do not agree on the parent/child lineage:

- The active parent identity is generation 0, node `node-f7b9c198a79ca49a`.
- The staged candidates claim generation 2 and parent
  `node-c47370c1ae880bf4`.
- The journal has no transition that advances from the generation-0 parent to a
  generation-1 or generation-2 parent.

## Hidden Errors

Baseline agent errors are present in the run record but hidden by aggregate
success records. In `record.json.gz`, the one agent turn has 20 tool calls:

- `code_item_lookup`: 6 calls, 6 errors, all `invalid_format`.
- `non_semantic_patch`: 1 call, 1 error, `invalid_format`, with partial patch
  failure.
- `read_file`: 9 calls, 0 errors.
- `list_dir`: 3 calls, 0 errors.
- `request_code_context`: 1 call, 0 errors.

Those errors are real protocol evidence, but campaign-level files report
baseline eval/protocol completion, not candidate failure:

- `closure-state.json`: eval and protocol `status: "complete"`.
- `batch-run-summary.json`: instance `status: "completed"`, `error: null`.
- `prototype1-loop-trace.json`: `protocol_failures: []`.

The candidate-stage error is hidden differently: the records never state whether
the three planned candidates were intentionally not run because `dry_run: true`,
blocked by parent identity mismatch, or skipped by another controller decision.
`prototype1-loop-trace.json` has pending stages, but `scheduler.json` leaves the
nodes as ordinary frontier `planned` nodes and the journal has no terminal
parent or child state for the stop.

## Central Journal Entries Needed

The central journal should make this diagnosable without reading scheduler,
trace, branch registry, and node files together.

Add records as projections of typed transitions, not monitor-only event names:

- `Parent<Ready>` after parent identity, scheduler node, selected instance, and
  checkout facts agree. It should include parent identity, active root, source
  node id, generation, branch id, and selected instance.
- `Parent<Stopped>` when a parent turn intentionally stops before candidate
  execution. For run10 this would record `dry_run_after_target_selection`,
  pending stages, selected target, selected branch, and candidate node ids.
- `Child<Planned>` for each staged candidate, carrying parent identity,
  candidate node id, generation, branch id, target artifact, patch id, and
  runner request path. This entry should fail or record `Child<Blocked>` if the
  parent node id is absent or does not match the active `Parent<Ready>`.
- Existing live-execution transitions should be required after `Child<Planned>`
  when not in dry-run mode: materialize, build, child artifact commit, spawn,
  `Child<Ready>`, `Child<Evaluating>`, `Child<ResultWritten>`, and observe.
- `Parent<Advanced>` or equivalent successor transition only after a selected
  child has a durable observed result and the active checkout is actually
  advanced.

Avoid adding another flattened `candidate_failure`, `admission`, `trace`, or
`heartbeat` layer. The missing facts are role/state facts: a parent was ready or
not, a child candidate was planned or blocked, and a parent turn stopped or
continued.

## Minimal Patch Plan

1. Make parent turn startup write a terminal parent readiness record:
   `Parent<Ready>` on success, `Parent<Stopped>` or `Parent<Blocked>` with
   structured reason on validation failure.
2. When staging candidates, validate every candidate's `parent_node_id`,
   generation, and `parent_branch_id` against the active `Parent<Ready>`. Write
   one `Child<Planned>` entry per candidate only after validation.
3. If the controller exits after target selection because of dry-run, append
   `Parent<Stopped>` with `dry_run_after_target_selection` and leave scheduler
   nodes in a distinct stopped/planned-for-dry-run state rather than ordinary
   frontier `planned`.
4. If candidate execution is requested, require the next transition journal
   entry after `Child<Planned>` to be materialization start or `Child<Blocked>`
   with a structured reason. Do not rely on absence of `runner-result.json` as
   the failure signal.
5. Update the monitor/summary path to derive terminal status from the journal
   first, then link out to scheduler, node, runner, and protocol files as
   evidence. Aggregates such as closure `complete` should not be displayed as
   candidate success.
