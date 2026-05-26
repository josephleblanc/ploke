# Prototype 1 Child-Plan Authority: Zero-Admission Flow

Status: source-grounded note for the 2026-05-25 zero-admission child-plan bug.

This document traces the parent-side execution path that failed in campaign
`p1-gemini35-flash-direct-15g2x3-20260525-140904`. The bug was not in the
runtime child `Channel`. It happened earlier, while the parent was trying to
produce or recover its durable `ChildPlan` message.

## Related Documents

The go-to documents for this pipeline are:

- `crates/ploke-eval/docs/prototype1-child-plan-authority/zero-admission-flow.md`
  - This note: the direct child-plan authority trace for the zero-admission
    failure.
- `crates/ploke-eval/docs/prototype1-child-plan-authority/README.md`
  - Directory index for child-plan authority notes.

If those do not answer the question, use these supporting documents for
progressive disclosure:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
  - Defines the local vocabulary for boxes, messages, buffers, Crown authority,
    and why child-plan publication is the first concrete message-box example.
- `docs/workflow/evalnomicon/drafts/runtime/parent-child-channel.md`
  - Separates parent-owned candidate publication from the later per-runtime
    child `Channel`, and marks scheduler/result projections as reconstruction
    surfaces rather than lifecycle authority.
- `docs/workflow/evalnomicon/drafts/runtime/authority.md`
  - Names the role/state authority model and the rule that transition records
    are projections of allowed transitions.
- `docs/workflow/evalnomicon/drafts/start-here/stage-overview.md`
  - Gives the operator-facing stage map, including child planning and the
    child-plan receiver checks.
- `docs/workflow/evalnomicon/drafts/history/crown-authority-background.md`
  - Places `messages/child-plan/*.json` among transitional persisted records
    and distinguishes typed message boxes from mutable projections.

## MessageBox Pattern

The child-plan path uses the `MessageBox` pattern, not a bare JSON-file
convention and not the older `LockBox` trait directly.

The underlying pieces are:

```text
File
  Static filesystem address schema.

Transition
  One directed type-state edge: From -> To.

MessageBox
  File + one Lock transition + one Unlock transition.

Message
  Cross-runtime obligation: one role/state writes one payload into the box, and
  another role/state reads that payload before taking the next transition.
```

For child planning, the concrete box is:

```text
Message:
  ChildPlan

Body:
  ChildPlanFiles

File:
  ChildPlanFile
  prototype1/messages/child-plan/<parent-node-id>.json

Lock transition:
  LockChildPlan
  Parent<Ready> -> Parent<Planned>

Unlock transition:
  UnlockChildPlan
  Parent<Planned> -> Parent<Selectable>
```

`Open<ChildPlan>` means the sender state and body have been combined, but the
message has not yet been durably packed into `ChildPlanFile`. Constructing it
with `Open::<ChildPlan>::from_sender(parent, files)` consumes the only valid
sender type-state for that message. If the `Open` value is dropped before it is
locked or converted into a typed failure, it panics intentionally. That prevents
code from silently consuming `Parent<Ready>` and losing the authority-bearing
message.

`open.lock(at, write_fn)` writes `ChildPlanFiles` into the resolved
`ChildPlanFile`. If the write succeeds, `ChildPlan::close_sender` advances the
sender:

```text
Parent<Ready> -> Parent<Planned>
```

and returns:

```text
(Parent<Planned>, Locked<ChildPlan>)
```

`Locked<ChildPlan>` means the payload is in the static durable box, but the
intended receiver has not consumed it yet. A fresh sender can get this value
directly from `open.lock(...)`; a restarted parent can reconstruct it with
`Locked::<ChildPlan>::from_box(at, read_child_plan_message)`.

Unlocking requires a `Parent<Planned>`. `locked.unlock(planned)` validates the
message against the receiver by checking:

- the `At<ChildPlanFile>` path matches the `ChildPlanFiles.message` field;
- the receiver parent node id matches `ChildPlanFiles.parent_node_id`;
- `receiver.generation + 1 == ChildPlanFiles.child_generation`.

If those checks pass, the receiver advances:

```text
Parent<Planned> -> Parent<Selectable>
```

and the code receives a `Received<ChildPlan>` containing the typed body.

The intended shape is therefore:

```text
typed sender state
  -> Open<Message>
  -> durable static box
  -> Locked<Message>
  -> typed receiver validation
  -> Received<Message> + next receiver state
```

The important rule for this bug: a failed broad-harness batch still needs a
durable `ChildPlan` message when it has attempted the child-plan phase. The
message may contain zero runnable children, but it must carry
`rejected_surface_attempts` so replay can distinguish "the batch ran and failed"
from "no child-plan attempt has happened yet."

The current retry bridge deserves scrutiny. `receive_existing_child_plan`
reconstructs `Locked<ChildPlan>` from disk, then calls
`Parent<Ready>::planned_from_locked_child_plan()` to recreate the receiver state
needed for unlock. That is probably the intended restart path, but it is also
where constructor visibility and receiver validation need to stay tight: retry
should recover the type-state transition from a valid locked message, not mint
authority from raw paths or projections.

## Reconstructed Entry Point

The exact shell invocation is not embedded in the campaign artifacts. The best
reconstruction is:

```bash
./target/debug/ploke-eval loop prototype1-step --repo-root . --format json
```

run from:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-20260525-140904
```

Evidence:

- Bash history contains that exact command shape.
- The campaign had already completed baseline eval/protocol and doctor phase
  had advanced to `child_plan`.
- Logs for `ploke_eval_20260525_142753_2.log` show
  `Parent<Ready>->ChildPlan` for parent node `node-18f71c7f3b1718b8`.
- The same log shows `Parent<Ready>->Parent<AwaitingHarnessPlan>` for
  `broad-harness-request:node-18f71c7f3b1718b8`.
- The campaign contains many broad request files for the same parent node, but
  no `prototype1/messages/child-plan/` directory. That is the persistence gap.

## Historical Run State

Campaign:

```text
p1-gemini35-flash-direct-15g2x3-20260525-140904
```

Parent node:

```text
node-18f71c7f3b1718b8
```

Scheduler policy:

```text
max_generations = 15
child_budget.min = 2
child_budget.max = 3
child_schedule_mode = full-batch
```

Run profile commitment:

```text
source_path = /home/brasides/.ploke-eval/profiles/prototype1/gemini35-flash-direct-15g2x3-20260525-140904.toml
admitted_at = 2026-05-25T21:09:22.465169693+00:00
```

The first historical broad request was:

```text
prototype1/messages/edit-harness-request/node-18f71c7f3b1718b8.json
request_id = broad-harness-request:node-18f71c7f3b1718b8
```

Its headless-TUI diagnostic summary ended with:

```text
terminal = TimedOut { secs: 240 }
```

Later retry logs also show fresh `r10` through `r18` broad request slots failing
during workspace preparation. That later noise is a symptom of the same
authority problem: retry saw no existing child-plan message and therefore kept
minting fresh request slots.

## Execution Flow

The control command path is:

```text
prototype1-step
-> run::core::step
-> diagnose
-> advance(DiagnosedPhase::ChildPlan)
-> advance_child_plan
-> active_parent_ready
-> resolve_profile_child_plan
-> resolve_child_plan
```

`advance_child_plan` first ensures the baseline closure state, constructs an
active parent, reserves the admitted profile child budget, then calls
`resolve_profile_child_plan`.

The parent type-state path before child planning is:

```text
Parent<Unchecked>
  -> check(...) -> Parent<Checked>
  -> ready(Startup<Genesis | Predecessor>) -> Parent<Ready>
```

For the historical generation-zero parent, startup was the genesis path. At
this point there is still no child runtime and no child channel.

`resolve_child_plan` then enters the parent-owned child-plan authority stage:

```text
Parent<Ready>
  -> resolve ChildPlanFile address
  -> if message exists: receive_existing_child_plan
  -> if message is absent: run parent target selection
```

In the failing run, the child-plan message was absent, so broad-harness target
selection published a batch.

## Broad-Harness Batch Type States

`publish_broad_harness_child_plan_request` publishes fresh request slots and
changes the parent state:

```text
Parent<Ready>
  -> awaiting_harness_plan_for_request(...)
  -> Parent<AwaitingHarnessPlan>
```

The returned `HarnessRequestBatch` carries:

```text
Parent<AwaitingHarnessPlan>
slots: Vec<HarnessRequestSlot>
child_budget: Prototype1ChildBudget::new(2, 3)
```

The loop then attempts slots until it reaches `max` admitted children or runs
out of slots. A slot failure, timeout, malformed result, or admission rejection
is not itself the controller bug. Those are valid reasons for a slot to produce
no child transaction.

The parent patch-generation unit is the request slot, not the final child
runtime. Each slot owns a provisional edit-harness checkout under:

```text
prototype1/workspaces/edit-harness/<request-id>
```

`prepare_broad_harness_workspace` creates or resets that checkout from the
active parent `HEAD`, then the parent runtime asks the headless TUI/model loop
to edit that candidate checkout. Admission serially validates the completed
slot, commits the changed files in the candidate checkout, and turns the
admitted transaction into `ChildPlan` material.

Only after admission does the controller have a runnable child candidate. In
the broad-harness path, `MaterializeBranch::transition_with_harness` validates
the admitted candidate workspace and records that workspace as the child
artifact root. It does not ask the model to patch `nodes/<node>/worktree`
after the child already exists.

The parent patch-generation loop may run multiple slot attempts concurrently,
bounded by `[search.children].parallel_targets`. If omitted, that cap defaults
to `min(3, search.children.max)`. Admission remains parent-serialized and still
writes exactly one `ChildPlan` message. This keeps parallel model/edit work
from becoming parallel child-plan authority.

Temporary build products are not durable evidence:

- edit-harness `target/` directories can be removed after each slot attempt
  has produced diagnostics or a submitted result;
- `nodes/<node>/target/` can be removed after the parent promotes the built
  child `ploke-eval` binary into `nodes/<node>/bin/`;
- `nodes/<node>/instance-targets/` exists for child self-evaluation and can be
  removed after child evaluation artifacts and terminal channel evidence have
  been recorded.

The durable surfaces are the submitted/admitted request evidence, the committed
candidate artifact, the promoted child binary until spawn no longer needs it,
the per-runtime channel result, and the run/protocol artifacts used for later
review.

The same `parallel_targets` cap also bounds parent-side child execution fanout:
`full-batch` still means "eventually run the full admitted batch", but it must
do so in capped chunks rather than starting every child runtime at once.

The controller bug starts when all attempted slots admit fewer than
`child_budget.min`.

## Broken Path

Before the fix, the below-minimum branch returned:

```text
InvalidBatchSelection:
  broad harness admitted 0 child transaction(s), fewer than required minimum 2
```

without first writing a `ChildPlan` message.

That left the system in an invalid restart shape:

```text
Parent<AwaitingHarnessPlan> existed only in memory
no Locked<ChildPlan> was written
no Received<ChildPlan> could be reconstructed
no rejected_surface_attempts were available to the parent
```

The next `prototype1-step` or `prototype1-continue` therefore entered
`resolve_child_plan`, checked the same `ChildPlanFile` address, found no file,
and treated the phase as fresh work. That is why the historical campaign
accumulated many request files for one parent node.

## Fixed Path

The fixed below-minimum branch must complete a parent-owned message transition
before returning the error:

```text
Parent<AwaitingHarnessPlan>
  -> accept_harness_plan() -> Parent<Ready>
  -> Open<ChildPlan>::from_sender(...)
  -> lock(...) -> Locked<ChildPlan>
  -> write ChildPlanFiles { children: [], rejected_surface_attempts: [...] }
  -> return InvalidBatchSelection
```

This does not make the failed batch successful. It makes the failed batch
durable and replayable.

On retry:

```text
resolve_child_plan
  -> ChildPlanFile exists
  -> receive_existing_child_plan
  -> Locked<ChildPlan>::from_box(...)
  -> Parent<Ready>::planned_from_locked_child_plan() -> Parent<Planned>
  -> locked.unlock(planned) -> Parent<Selectable> + Received<ChildPlan>
```

The received plan may contain zero runnable children. That is allowed only when
it carries parent-readable rejected surface-attempt evidence.

## Not A Runtime Channel Event

The runtime `Channel` starts later, after child artifacts exist and the parent
enters the C1-C5 child attempt path:

```text
ChildFiles -> C1 -> C2 -> C3 -> C4 -> C5
```

This zero-admission bug happened before `ChildFiles` existed. Therefore:

- do not inspect `ToParent` messages to decide this state;
- do not use `runner-result.json` or result sidecars to drive this state;
- do not use scheduler, branch, or request projections as authority;
- do use the parent-owned `ChildPlan` message as the child-plan authority.

Runtime projections remain useful for later reconstruction and run review, but
they must not substitute for the authority-bearing message transition.

## Child Treatment Evidence Boundary

This zero-admission bug is parent-side and happens before any child runtime
exists. For later child-runtime diagnosis, keep a separate boundary in mind:
treatment evidence leaves the child only in the terminal per-runtime channel
message.

The child execution path is:

```text
run treatment eval closure
-> run treatment protocol closure
-> load treatment closure state
-> build treatment evidence from complete run records
-> require complete treatment metrics
-> validate patch projection
-> optionally attach MBE oracle evidence
-> write attempt runner result projections
-> send ToParent::Result { runner_result, treatment? }
```

`Prototype1TreatmentEvidence` is assembled in memory from treatment
`closure-state.json` and each complete run's `record.json.gz`. It becomes
parent-visible evidence only when carried by `ToParent::Result.treatment`.
`nodes/<node-id>/results/<runtime-id>.json`, `runner-result.json`, and
`Child<ResultWritten>` are reconstruction/projection surfaces; they do not carry
the full treatment payload the parent needs for comparison.

If treatment evidence is incomplete, the child must not emit a success-shaped
terminal result. The current gate requires every treatment instance to have
metrics derived from a complete run record. When that gate fails, the terminal
channel result carries a failed `runner_result` and no `treatment` payload.

One live canary failure exposed a test setup issue at this boundary. The eval
closure did write a run record, but closure-state reconstruction marked the
instance `missing` because the live test used a non-canonical eval-home spelling
containing `crates/ploke-eval/../../target/...`, while the run registration held
the canonical `/home/brasides/code/ploke/target/...` spelling. Registration
lookup compares `storage_roots.runs_dir` by path identity, so live test roots
need to be canonicalized before campaign paths are written. Do not paper over
that with runner-result or scheduler projection reads; the fix belongs in test
setup or path normalization before registration/evidence lookup.

## Source Map

- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`
  - `step`
  - `advance`
  - `advance_child_plan`
  - `active_parent_ready`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `resolve_profile_child_plan`
  - `resolve_child_plan`
  - `publish_broad_harness_child_plan_request`
  - `admit_broad_harness_batch`
  - `publish_broad_harness_child_plan_from_admitted_batch`
  - `persist_rejected_plan`
  - `receive_existing_child_plan`
  - `receive_child_plan`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
  - `prepare_broad_harness_workspace`
  - `admit_submitted_broad_harness_result`
- `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
  - `MaterializeBranch::transition_with_harness`
- `crates/ploke-eval/src/cli/prototype1_state/c2.rs`
  - `BuildChild`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
  - `run_prototype1_resolved_branch_treatment`
  - `require_complete_treatment`
  - `prepare_child_instance_target_cache`
  - `cleanup_prototype1_child_build_products`
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
  - `ToParent::Result`
  - `Channel::send_terminal_result`
- `crates/ploke-eval/src/closure.rs`
  - `build_instance_row`
- `crates/ploke-eval/src/run_registry.rs`
  - `list_registrations_for_instance`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
  - `ChildPlan`
  - `ChildPlanFiles`
  - `Parent<Ready>::awaiting_harness_plan_for_request`
  - `Parent<AwaitingHarnessPlan>::accept_harness_plan`
  - `Parent<Ready>::planned_from_locked_child_plan`

## Regression Coverage

`step_persists_zero_admission_plan` covers the direct control path:

```text
prototype1-step
-> diagnose child_plan
-> advance_child_plan
-> publish broad-harness slots
-> replay historical timed-out headless-TUI diagnostics through a test-only TUI fixture hook
-> expect below-minimum error
-> assert ChildPlanFile exists with zero children and rejected_surface_attempts
-> diagnose again and assert the phase is not child_plan
```

The test-scoped minter writes the same production records the controller reads:
`campaign.json`, admitted `run-profile.toml` plus commitment,
`closure-state.json`, and `.ploke/prototype1/parent_identity.json` committed on
the active parent branch. It does not prewrite the `ChildPlanFile`.

`broad_harness_batch_admits_three_transactions_into_three_children` covers the
positive batch-admission path:

```text
Parent<AwaitingHarnessPlan>
-> accept_harness_plan -> Parent<Ready>
-> write ChildPlanFile during batch_admission
-> receive ChildPlanFile during message_receive
-> produce three runnable ChildFiles
```

`zero_admission_batch_is_persisted` covers the controller contract:

```text
publish broad batch
-> inject historical timed-out headless-TUI summary
-> publish zero-admission admitted batch
-> expect below-minimum error
-> retry through resolve_profile_child_plan
-> observe existing rejected-attempt-only ChildPlan
-> assert no fresh request slots were minted
```

`child_plan_replay_rejects_wrong_parent` covers the receiver-validation branch:
a persisted `ChildPlanFiles` body addressed to another parent can be read as a
locked message, but `message_receive` fails instead of admitting it.

`child_plan_replay_rejects_malformed_file` covers the read boundary: malformed
JSON at the authority path fails during `retry_replay`, before the code can
construct `Parent<Planned>` or attempt message receive.

`broad_tui_prep_failure_is_setup_blocker` covers a related boundary: workspace
preparation failures are setup blockers and must not be silently counted as an
ordinary inadmissible edit.

These tests do not prove a full live successor handoff. They prove the
authority gap that prevented the run from reaching child materialization is now
closed for the historical failure shape, including the `prototype1-step`
controller path up to durable child-plan authority.
