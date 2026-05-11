# Prototype 1 Selection Impact Audit: Control Flow

Date: 2026-05-05

## Frame

Surface Request: audit how selecting a child node affects downstream code.

Causal Chain: child eval/evidence -> successor selection -> continuation decision -> next parent/runtime/worktree -> loop termination.

Concern: selection may be acting as more authority than intended or may stop/continue the loop for the wrong reason.

Preservation Check: selection evidence must not be conflated with Crown/oracle authority. A selected child is a candidate Artifact/Runtime coordinate; successor admission still belongs to Parent/Runtime/Artifact/Crown/History transitions.

## Intended Model

`crates/ploke-eval/src/cli/prototype1_state/mod.rs:166-178` defines the intended single-successor flow: Parent creates child checkout, evaluates child runtime, selects the child Artifact, updates the stable active checkout, hydrates the successor runtime from that checkout, hands off parent authority, and cleans up temporary child workspaces.

`crates/ploke-eval/src/cli/prototype1_state/mod.rs:221-228` says the sealed History block should carry the selected Artifact, successor identity, policy-bearing surface digest, and evidence references; late evidence belongs to ingress, not a rewritten sealed block.

`crates/ploke-eval/src/cli/prototype1_state/mod.rs:409-421` explicitly warns that scheduler reports are not authority and that continuation/successor handoff should be attempt-scoped rather than derived from the latest mutable scheduler field.

`crates/ploke-eval/src/cli/prototype1_state/history.rs:360-372` narrows the current implementation claim: live successor validation still uses mutable scheduler/invocation state for transport identity, while History admission checks the current checkout tree key and surface commitment against the sealed head.

## Control-Flow Path

### Typed state command path

1. Child completion is observed in `crates/ploke-eval/src/cli/prototype1_state/c4.rs`.
   - Failed child runner result becomes `C5 { report: Report::reject(), observed: ObservedChild::Failed(..) }` and returns without successor selection: `c4.rs:341-371`.
   - Successful child runner result loads the branch evaluation artifact and carries `report.overall_disposition` into `C5`: `c4.rs:374-428`.

2. Only `ObservedChild::Succeeded` enters successor selection in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5736-5796`.
   - `selection_input_from_child_report` maps the node plus evaluation report into `SelectionInput`, preserving node id, branch id, generation, branch disposition, evaluation artifact path, and parent/child metric comparisons: `cli_facing.rs:6820-6845`.
   - `successor_selection::decide` is called at `cli_facing.rs:5748-5754`.
   - Its result is converted into scheduler continuation by passing only `selected_branch_id` and `selected_branch_disposition`: `cli_facing.rs:5755-5763`.
   - Both the selection decision and continuation decision are appended as `SuccessorRecord::Selected`: `cli_facing.rs:5777-5791`.
   - The continuation decision is also written to scheduler singleton state: `cli_facing.rs:5792-5796`.
   - Only `ContinueReady` triggers successor handoff: `cli_facing.rs:5797-5806`; non-continue decisions render `successor_handoff=skipped:<disposition>`: `cli_facing.rs:5830-5842`.

3. Successor selection itself is operational-only today.
   - Registry defaults to the operational domain only: `crates/ploke-eval/src/successor_selection/registry.rs:19-34`.
   - Operational verdicts are derived from branch disposition plus comparable metric deltas: `crates/ploke-eval/src/successor_selection/domains/operational.rs:39-51`.
   - Verdict mapping is `Better -> Select`, `Mixed -> ContinueWithRisk`, `Worse/Inconclusive/missing -> Stop`: `crates/ploke-eval/src/successor_selection/decision.rs:26-31`.
   - `Select` and `ContinueWithRisk` both populate `selected_branch_id`; `Stop` clears it: `decision.rs:33-38`.

4. Scheduler continuation is the stop/continue gate.
   - `Prototype1ContinuationDecision` stores disposition, selected branch id/disposition, next generation, and total node count: `crates/ploke-eval/src/intervention/scheduler.rs:45-54`.
   - `decide_continuation` stops for no selection, rejected branch when `require_keep_for_continuation` is true, first-keep policy, generation limit, or total-node limit; otherwise it returns `ContinueReady`: `scheduler.rs:627-661`.
   - Because the keep requirement is policy-controlled, a `ContinueWithRisk` decision with selected disposition `reject` can continue if `require_keep_for_continuation` is false: `scheduler.rs:636-651` and `successor_selection/decision.rs:33-38`.

5. `ContinueReady` hands off to the next runtime.
   - `spawn_and_handoff_prototype1_successor` reloads the selected node and calls `prepare_prototype1_active_successor_runtime`: `crates/ploke-eval/src/cli/prototype1_process.rs:1123-1137`.
   - Preparation revalidates the scheduler singleton, marks the treatment branch selected, installs the selected Artifact into the stable active checkout, and builds the successor binary from that checkout: `prototype1_process.rs:492-504`.
   - Scheduler validation accepts only `last_continuation_decision.disposition == ContinueReady` and matching `selected_next_branch_id == node.branch_id`: `prototype1_process.rs:415-448`.
   - Installing the Artifact verifies the target, records checkout before/after, updates the active checkout, validates parent identity, and appends `ActiveCheckoutAdvancedEntry`: `prototype1_process.rs:589-693`.
   - The Parent then seals a History block through `Parent<Selectable>::seal_block_with_artifact`, retiring the Parent and admitting the selected Artifact claim under a ruling Crown: `prototype1_process.rs:1123-1200`; the block is appended at `prototype1_process.rs:1201-1220`.
   - The successor invocation can only be written through a `Parent<Retired>` carrier: `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:242-264` and `invocation.rs:389-400`.
   - The successor process is spawned from the active checkout, and the predecessor waits for ready/timeout/early exit: `prototype1_process.rs:1332-1425`.

6. The successor rehydrates as Parent only after separate startup checks.
   - The successor command loads the invocation, checks campaign, node identity, active root, scheduler continuation, and predecessor History startup, then records successor ready before entering `Parent<Ready>`: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5271-5345`.
   - On successful bounded turn, it records successor completion: `cli_facing.rs:5941-5949`.

### Older loop command path

`run_prototype1_loop_controller` has a separate continuation projection.

- It evaluates all branch summaries, chooses `selected_next_branch_id = select_most_promising_branch(&branch_evaluations)`, and immediately calls `select_treatment_branch` if any branch was chosen: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:953-1010`.
- `select_most_promising_branch` always returns the max-scoring row when any evaluation exists; `Keep` is only one tie-breaker, not a hard filter: `cli_facing.rs:6302-6319`.
- Only after mutating branch selection does the loop derive `selected_branch_disposition`, call `decide_continuation`, and write the scheduler decision: `cli_facing.rs:1024-1041`.
- `select_treatment_branch` mutates `source_node.selected_branch_id` and `active_targets.active_branch_id` in the branch registry: `crates/ploke-eval/src/intervention/branch_registry.rs:452-544`.

This path does not spawn the successor directly, but it can leave a rejected or otherwise stop-causing branch recorded as the active selected treatment branch before the continuation policy says stop.

## Risks

1. Failed child observations stop implicitly without a persisted continuation decision in the typed state path. `ObservedChild::Failed` produces a reject report (`c4.rs:341-371`), but `cli_facing.rs:5736-5842` only calls successor selection and `record_continuation_decision` for `ObservedChild::Succeeded`. The terminal reason is therefore an output string rather than the same scheduler/SuccessorRecord path used for selected/rejected/inconclusive successful evaluations.

2. Scheduler continuation authority is still a mutable campaign singleton. `record_continuation_decision` overwrites `last_continuation_decision`: `crates/ploke-eval/src/intervention/scheduler.rs:676-685`; successor validation later reads the latest singleton and branch id only: `prototype1_process.rs:415-448`. This preserves current single-parent behavior but is not attempt-scoped and can authorize/reject the wrong runtime if another controller turn changes the scheduler between selection and successor validation.

3. Selection can carry more control-flow authority than intended through `ContinueWithRisk`. `SuccessorDecision` gives `ContinueWithRisk` a selected branch id (`successor_selection/decision.rs:33-38`). Scheduler policy then decides whether a non-keep selected branch is fatal (`scheduler.rs:636-651`). With `require_keep_for_continuation = false`, a mixed/rejected operational decision can become `ContinueReady`; that may be intentional policy, but it should be explicit because it lets weak evidence proceed to Artifact installation and Crown/History handoff.

4. The older loop path mutates branch registry selection before continuation says continue. `cli_facing.rs:1006-1010` calls `select_treatment_branch` immediately after ranking summaries, while `cli_facing.rs:1024-1041` computes the stop/continue decision afterward. A rejected branch can be projected as selected/active in `branches.json` even when the scheduler decision stops the loop.

5. Successor timeout occurs after active checkout advancement and Crown retirement. `spawn_and_handoff_prototype1_successor` installs the Artifact and seals/appends History before spawn wait (`prototype1_process.rs:1123-1220`), then timeout returns `Ok((retired_parent, None))`: `prototype1_process.rs:1404-1411`. That is structurally honest about retired predecessor state, but operators should read timeout as "handoff failed after authority boundary work began", not as a harmless no-op stop.

## Commands That Verify These Claims

Small targeted checks run during this audit:

```sh
cargo test -p ploke-eval successor_selection --locked
cargo test -p ploke-eval continuation --locked
```

Both passed on 2026-05-05. They cover the current successor-selection outcome mapping and scheduler continuation policy, but they do not cover the full typed state path from failed child observation to persisted terminal reason, nor the race/staleness risk around `last_continuation_decision`.

Useful read-only inspection commands:

```sh
rg -n "successor_selection::decide|record_continuation_decision|spawn_and_handoff_prototype1_successor|validate_prototype1_successor_continuation|select_most_promising_branch" crates/ploke-eval/src
rg -n "last_continuation_decision|ContinueWithRisk|StopNoSelectedBranch|StopSelectedBranchRejected" crates/ploke-eval/src
```

## Bottom Line

The typed state path mostly preserves the authority boundary: selection evidence is recorded first, scheduler continuation gates handoff, and Crown/History admission happens only after the active checkout is installed and the Parent crosses to `Retired`. The weak points are around projection and scoping: failed child terminal reasons are not persisted through the same continuation record, `last_continuation_decision` is not attempt-scoped, the older loop path records branch selection before continuation, and `ContinueWithRisk` can become real successor admission under permissive policy.
