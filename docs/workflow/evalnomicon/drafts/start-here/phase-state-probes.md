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
```

Read these in this order:

1. `node.json`: current projected node status.
2. `transition-journal.jsonl`: latest transition for this node/runtime.
3. `invocations/<runtime-id>.json`: bootstrap contract and channel root.
4. `channels/<runtime-id>/child-to-parent.jsonl`: child-written channel
   messages.
5. `results/<runtime-id>.json`: attempt-scoped terminal runner result.
6. `runner-result.json`: latest mutable node-level runner result projection.
7. treatment campaign artifacts, if a treatment campaign id is visible.

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
The parent checks the child channel first and then the expected result file.

## Observe Child Probe

`observe_child before` means:

```text
parent has C4
child runtime id is known
parent appended ObserveChild(Before, result = None)
parent is polling child-to-parent.jsonl for ToParent::Result
parent is also checking results/<runtime-id>.json as a fallback
```

The poll interval is 100ms. If neither a channel `result` nor the expected
result file appears within `OBSERVE_CHILD_STALE_AFTER`, currently ten minutes,
the observe transition reports a stale or hung child.

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

This is a fallback state, not proof that the parent will wait forever.

The child writes both:

```text
nodes/<node-id>/results/<runtime-id>.json
nodes/<node-id>/runner-result.json
```

before it sends the terminal channel message. During `observe_child`, the parent
checks the channel first. If no `result` message is present but the expected
attempt result file exists, the parent loads the attempt result from disk.

That fallback is enough to observe a failed child result. It is not enough to
observe a successful treatment, because the result file does not carry the
treatment evidence payload needed for comparison. A succeeded runner result
without channel treatment evidence causes `MissingTreatmentEvidence`.

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

If `runner_result.disposition` is `Succeeded` but `treatment` is missing, the
parent observe transition errors with `MissingTreatmentEvidence`. A successful
runner result without treatment evidence is not enough for comparison.

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

## Observe Child Decision Table

| Evidence | Most likely state |
| --- | --- |
| `ObserveChild(Before)`, no channel file | child not yet channel-visible, wrong channel root, or child died before first channel write |
| channel has `ready`, no `evaluating` | child started but has not entered evaluation |
| channel has `evaluating`, no result files | child is inside treatment eval/protocol/evidence steps |
| treatment campaign exists, eval incomplete | child is running treatment eval closure |
| treatment campaign eval complete, protocol incomplete | child is running treatment protocol closure |
| treatment campaign closure complete, no result file | child is building/validating treatment evidence or attaching oracle evidence |
| failed attempt result exists, channel has no `result` | parent can observe the failure from the result-file fallback |
| succeeded attempt result exists, channel has no `result` | parent can load the result, but success observation lacks treatment evidence and will error |
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
