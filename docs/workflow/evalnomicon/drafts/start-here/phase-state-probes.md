# Phase State Probes

This guide is for answering: "The loop is in phase X, but what is actually
happening now?" It focuses on durable files that can be read from outside the
running process without mutating the campaign.

The first rule is to separate three views:

- parent diagnosis: what `doctor` or the latest parent journal entry says;
- child attempt state: what the child wrote under its node/runtime paths;
- benchmark/eval state: what the child-created treatment campaign has written.

These views can lag each other. A `Before` journal entry proves a transition
started. It does not prove the transition completed.

## Starting Coordinates

For a child phase, start from:

```text
campaign id
node id
runtime id, if the child has reached spawn/observe
doctor phase
latest transition journal entry
```

Derive the common paths:

```bash
CAMPAIGN=<campaign>
NODE=<node-id>
RUNTIME=<runtime-id>
PROTO="$HOME/.ploke-eval/campaigns/$CAMPAIGN/prototype1"
NODE_DIR="$PROTO/nodes/$NODE"
JOURNAL="$PROTO/transition-journal.jsonl"
NODE_JSON="$NODE_DIR/node.json"
INVOCATION="$NODE_DIR/invocations/$RUNTIME.json"
CHANNEL="$NODE_DIR/channels/$RUNTIME/child-to-parent.jsonl"
ATTEMPT_RESULT="$NODE_DIR/results/$RUNTIME.json"
LATEST_RESULT="$NODE_DIR/runner-result.json"
STREAM_DIR="$NODE_DIR/streams/$RUNTIME"
```

Read these in this order:

1. `node.json`: current projected node status.
2. `transition-journal.jsonl`: latest parent transition plus child runtime
   records for this node/runtime.
3. `invocations/<runtime-id>.json`: bootstrap contract and channel root.
4. `channels/<runtime-id>/child-to-parent.jsonl`: child-written channel
   messages.
5. `streams/<runtime-id>/stderr.log`: child process timing and diagnostic
   output.
6. `results/<runtime-id>.json`: attempt-scoped terminal runner result.
7. `runner-result.json`: latest mutable node-level runner result projection.
8. treatment campaign artifacts, if a treatment campaign id is visible.

Avoid using `scheduler.json` as the first answer. It can be useful path context,
but it is a projection and can lag the child attempt.

## Journal Before And After

Most transition journal entries use `phase: before` and `phase: after`.

- `before` means the parent entered the transition and recorded its intent or
  wait boundary.
- `after` means the transition observed a terminal outcome and recorded the
  result.
- `before` without matching `after` means the transition is in progress, blocked
  inside the transition, or the process died before recording completion.

For `observe_child`, `before` is especially important: the parent has started
waiting for a child result and has recorded the expected runner result path.
The parent polls the child channel for the terminal `ToParent::Result`. Result
files are reconstruction evidence and should not advance parent/child lifecycle
state.

## Observe Child Probe

`observe_child before` means:

```text
parent has C4
child runtime id is known
parent appended ObserveChild(Before, result = None)
parent is polling child-to-parent.jsonl for ToParent::Result
```

The poll interval is 100ms. If no channel `result` appears within the admitted
profile's `execution.observe_child_stale_after_secs`, the observe transition
reports a stale or hung child.

The child side may be in one of several states.

### 1. Channel Missing

If `channels/<runtime-id>/child-to-parent.jsonl` does not exist, the child has
not written any channel evidence for this runtime, or the channel root is wrong.
Check the invocation file and child streams next.

Expected nearby files:

```text
nodes/<node-id>/invocations/<runtime-id>.json
nodes/<node-id>/streams/<runtime-id>/stdout.log
nodes/<node-id>/streams/<runtime-id>/stderr.log
```

### 2. Ready But Not Evaluating

If the channel has `ready` but no `evaluating`, the child started and
acknowledged itself, but has not written the evaluation-start message.

This is a narrow window in normal execution. If it persists, inspect child
stderr/stdout and the child process.

### 3. Evaluating But No Result

If the channel has `evaluating` but no `result`, and neither result file exists,
the child is probably inside `run_prototype1_resolved_branch_treatment`.
This is the common interpretation of an operator report that says the expected
result path is known from the journal but the result file has not been written.

The child-side sequence is:

1. materialize the selected treatment branch in the child workspace;
2. resolve the baseline campaign;
3. create a treatment campaign;
4. prepare an instance target cache under the node directory;
5. run treatment eval closure;
6. run treatment protocol closure;
7. load treatment closure state;
8. build treatment evidence;
9. validate patch projection;
10. optionally attach MBE oracle evidence;
11. build and write the runner result;
12. send terminal `result` on the channel.

To refine the state, look for a treatment campaign whose id starts with:

```text
<baseline-campaign>-treatment-<branch-id>-
```

The branch id is in `node.json`. Once the treatment campaign exists, inspect:

```text
$HOME/.ploke-eval/campaigns/<treatment-campaign>/campaign.json
$HOME/.ploke-eval/campaigns/<treatment-campaign>/closure-state.json
```

The treatment campaign `closure-state.json` tells you whether the child is
still in eval closure, protocol closure, or post-closure evidence assembly.

### 4. Result File Exists But Channel Has No Result

This is a projection-only state. It is useful evidence for reconstruction, but
it is not parent/child lifecycle authority.

The child writes both:

```text
nodes/<node-id>/results/<runtime-id>.json
nodes/<node-id>/runner-result.json
```

before it sends the terminal channel message. During `observe_child`, the parent
waits for the channel `result` message.

A result file without a channel `result` means the child reached at least the
result-write projection, but the parent should keep waiting until the channel
`result` arrives or the observe timeout expires. A succeeded runner result
without channel treatment evidence cannot be compared against baseline.

Check:

- whether the attempt result and latest result agree;
- whether the disposition is `Succeeded` or a failure disposition;
- child stderr/stdout for a post-result channel error;
- whether a matching `ObserveChild(After, ...)` later appeared in the journal.

### 5. Channel Result Exists

If `child-to-parent.jsonl` contains `result`, the parent should soon append
`ObserveChild(After, ...)`.

The channel `result` body includes:

- `runner_result`: `Succeeded`, `TreatmentFailed`, or `CompileFailed`;
- `treatment`: present only for successful treatment evaluation.

If `runner_result.disposition` is not `Succeeded`, the parent records an
`ObservedChildResult::Failed` and the node should become `Failed`.

If the channel `result` has `runner_result.disposition = Succeeded` but
`treatment` is missing, the parent observe transition errors with
`MissingTreatmentEvidence`. A successful runner result without treatment
evidence is not enough for comparison.

If `runner_result.disposition` is `Succeeded` and `treatment` is present, the
parent moves to `C5`, appends `ObserveChild(After, TreatmentComplete)`, and then
compares treatment evidence against the parent baseline.

### 6. After Exists

If the journal has `ObserveChild(After, ...)`, the observe transition completed.
Then check whether parent comparison ran:

```text
prototype1/evaluations/<branch-id>.json
```

That evaluation file is parent-written after successful observation. Its absence
after a successful `After` entry means the next question is parent-side
comparison, not child-side evaluation.

## Treatment Subphase Probes

Once the child has entered `evaluating`, there are more durable surfaces than
the channel and runner result. The fastest way to inspect them is:

1. Find the treatment campaign id.
2. Open its `closure-state.json`.
3. Follow `instances[].artifacts` into run registrations, run roots, and
   protocol artifacts.
4. Use child `stderr.log` for timing and in-flight clues when files are not yet
   complete.

### Child Runtime Journal Records

The child writes `JournalEntry::Child` records to the same
`transition-journal.jsonl` used by the parent. These records are durable
projections of child state transitions:

- `child:ready`: child acknowledged the invocation.
- `child:evaluating`: child entered `run_prototype1_resolved_branch_treatment`.
- `child:result_written`: child persisted `results/<runtime-id>.json`.

These records are separate from `child-to-parent.jsonl`. If the channel and
journal disagree, treat that as a real diagnostic signal, not as harmless
duplication.

### Child Stream Logs

The child process writes stdout and stderr under:

```text
nodes/<node-id>/streams/<runtime-id>/stdout.log
nodes/<node-id>/streams/<runtime-id>/stderr.log
```

`stderr.log` includes timing markers for the whole treatment branch evaluation,
for example:

```text
loop.prototype1_branch.evaluate.<branch-id>.start
loop.prototype1_branch.evaluate.<branch-id>.end +<seconds>s
```

It can also contain provider retries, body timeouts, LLM errors, git failures,
or protocol retry messages. Use it when the child is still in `evaluating` and
the result file has not appeared.

### Create Treatment Campaign

Treatment campaign creation writes:

```text
$HOME/.ploke-eval/campaigns/<treatment-campaign>/campaign.json
```

The id has this shape:

```text
<baseline-campaign>-treatment-<branch-id>-<timestamp-ms>
```

The manifest points the treatment campaign at treatment-specific roots derived
from the baseline campaign:

```text
<baseline instances root>/treatments/<branch-id>/instances
<baseline batches root>/treatments/<branch-id>/batches
```

At this point `closure-state.json` may not exist yet. It is created by closure
recompute, not by campaign-manifest creation itself.

### Prepare Instance Target Cache

The child creates a node-local repo cache:

```text
nodes/<node-id>/instance-targets/<treatment-campaign>/
```

During eval planning, the required benchmark repos are cloned into that cache
under:

```text
nodes/<node-id>/instance-targets/<treatment-campaign>/<org>/<repo>
```

There is no JSON record whose only purpose is "instance target cache prepared".
The evidence is directory existence, cloned repo contents, and later run
manifests whose `repo_root` points into this node-local cache.

### Run Treatment Eval Closure

Eval closure recomputes treatment closure state before and after running eval.
The treatment campaign state is:

```text
$HOME/.ploke-eval/campaigns/<treatment-campaign>/closure-state.json
```

Important fields:

- `eval`: aggregate eval status and counts.
- `instances[].eval_status`: per-instance eval state.
- `instances[].eval_failure`: per-instance failure summary.
- `instances[].artifacts`: paths to the current run artifacts.
- `instances[].last_event_at`: newest known event timestamp for that instance.

Eval planning also writes manifests:

```text
<treatment instances root>/<instance-id>/run.json
<treatment batches root>/<batch-id>/batch.json
```

Running the batch writes:

```text
<treatment batches root>/<batch-id>/batch-run-summary.json
<treatment batches root>/<batch-id>/multi-swe-bench-submission.jsonl
```

The batch submission file is created at batch start. Treat it as meaningful
only when it has nonempty submission content.

For each attempted instance, prefer the run registration referenced from
`closure-state.json`:

```text
instances[].artifacts.registration_path
```

That registration is the authority for run identity, lifecycle status, selected
model/provider, and artifact paths. It points at a run root like:

```text
<treatment instances root>/<instance-id>/runs/<run-id>/
```

Common run-root artifacts include:

```text
repo-state.json
execution-log.json
indexing-status.json
parse-failure.json
snapshot-status.json
indexing-checkpoint.db
indexing-failure.db
final-snapshot.db
agent-turn-trace.json
agent-turn-summary.json
llm-full-responses.jsonl
validation-audit.json
record.json.gz
multi-swe-bench-submission.jsonl
benchmark-patch-projection.json
```

Not every file exists for every state. `record.json.gz` is the primary
complete-run record, but the run registration lifecycle still decides whether
the attempt completed or failed. `execution-log.json` or `snapshot-status.json`
without `record.json.gz` is partial eval evidence.

### Run Treatment Protocol Closure

Protocol closure starts only for eval-complete rows with `record.json.gz`.
It writes protocol artifacts for each run. Prefer the protocol directory named
by the run registration or treatment closure row:

```text
instances[].artifacts.protocol_artifacts_dir
```

Protocol artifact file names include the procedure, subject id, and timestamp:

```text
<timestamp>_tool_call_intent_segmentation_<subject>.json
<timestamp>_tool_call_review_<subject>.json
<timestamp>_tool_call_segment_review_<subject>.json
```

The registration also tracks the latest segmentation anchor as:

```text
instances[].artifacts.protocol_anchor
```

Treatment `closure-state.json` summarizes protocol progress through:

- `protocol`: aggregate protocol status and counts.
- `protocol.status_by_procedure`: aggregate status per required procedure.
- `instances[].protocol_status`: per-instance protocol status.
- `instances[].protocol_procedures`: per-instance procedure status.
- `instances[].protocol_counts`: reviewed calls and segment counts.
- `instances[].protocol_failure`: protocol aggregate or artifact failure.

If protocol artifacts exist but `closure-state.json` still shows protocol
missing, inspect the artifact directory before concluding that protocol made no
progress. The artifact directory can lead the closure projection.

### Load Treatment State

Loading treatment state does not write a new artifact. It reads:

```text
$HOME/.ploke-eval/campaigns/<treatment-campaign>/closure-state.json
```

If the child is after protocol closure and before result writing, this file is
the main durable state object.

### Build Treatment Evidence

Treatment evidence assembly is mostly in memory. It reads treatment
`closure-state.json`, then reads each complete run's `record.json.gz` to derive
operational metrics.

The assembled evidence becomes durable only when it is carried by the terminal
child channel `result`. The runner result file alone does not contain the full
treatment evidence payload.

### Validate Patch Projection

Patch projection validation is a gate, not a new success artifact. It reads:

```text
instances[].artifacts.registration_path
<run registration>.artifacts.patch_projection
benchmark-patch-projection.json
```

For nonempty MBE submissions, the gate requires:

- metrics report `patch_projection_check_state = Passed`;
- the patch projection artifact exists;
- the projection checkout cwd is under
  `nodes/<node-id>/instance-targets/<treatment-campaign>/`;
- the projection checkout cwd is not inside the child Artifact worktree.

If this gate fails, the failure is surfaced through the child runner result and
stderr, not through a separate validation-success file.

## Observe Child Decision Table

| Evidence | Most likely state |
| --- | --- |
| `ObserveChild(Before)`, no channel file | child not yet channel-visible, wrong channel root, or child died before first channel write |
| channel has `ready`, no `evaluating` | child started but has not entered evaluation |
| channel has `evaluating`, no result files | child is inside treatment eval/protocol/evidence steps |
| treatment campaign exists, eval incomplete | child is running treatment eval closure |
| treatment campaign eval complete, protocol incomplete | child is running treatment protocol closure |
| eval run root has partial artifacts, no `record.json.gz` | child is inside eval setup/indexing/agent turn/packaging or eval failed before durable completion |
| protocol artifacts exist, closure protocol still missing | protocol artifacts may lead the closure projection; inspect artifact dir and projection freshness |
| treatment campaign closure complete, no result file | child is building/validating treatment evidence or attaching oracle evidence |
| failed attempt result exists, channel has no `result` | child wrote a failure projection, but parent lifecycle observation still waits for channel `result` |
| succeeded attempt result exists, channel has no `result` | child wrote a success projection, but parent lifecycle observation still waits for channel `result` with treatment evidence |
| channel has `result`, no `ObserveChild(After)` | parent has not consumed the result yet, parent is stalled, or journal append failed |
| `ObserveChild(After, Failed)` | child terminal failure was observed |
| `ObserveChild(After, TreatmentComplete)` | child success was observed; parent comparison should follow |

## What Not To Conclude

- Do not conclude the child is stuck from `observe_child before` alone.
- Do not conclude success from `runner-result.json` alone; the parent requires
  channel `result` with treatment evidence for successful observation.
- Do not conclude selection can run just because the child wrote a result; the
  parent must compare treatment evidence against the baseline first.
- Do not treat `scheduler.json` as stronger than node records, attempt result,
  channel messages, or the transition journal.

## Source Anchors

- Parent observe transition:
  [`c4.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/c4.rs)
- Child runner result write and terminal channel send:
  [`prototype1_process.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_process.rs)
- Runtime channel message schema:
  [`channel.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/channel.rs)
- Attempt result paths:
  [`invocation.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/invocation.rs)
- Child runtime journal records:
  [`child.rs`](../../../../../crates/ploke-eval/src/cli/prototype1_state/child.rs)
- Closure state and closure artifact refs:
  [`closure.rs`](../../../../../crates/ploke-eval/src/closure.rs)
- Eval run artifact writers:
  [`runner.rs`](../../../../../crates/ploke-eval/src/runner.rs)
- Protocol artifact writers:
  [`protocol_artifacts.rs`](../../../../../crates/ploke-eval/src/protocol_artifacts.rs)
