# Explorer B: Live Prototype 1 Authority Path

Scope: inspected the current live controller/process path only. No Rust source was edited.

## Summary

Prototype 1 already has useful typed scaffolding, but the live path still protects most authority boundaries by convention: raw `PathBuf`, `campaign_id`, `node_id`, `branch_id`, and JSON files stand in for operational authority. The smallest redesign should wire capability types at existing seams instead of expanding the controller.

Target lifecycle:

`active parent root -> child worktrees -> child self-eval records -> policy selection -> active checkout update -> successor binary built from selected artifact -> handoff -> old parent exits`

## Main Integration Points

1. Active parent root authority

Risk: the live loop resolves `intervention_repo_root` from `current_dir()` and then uses it for active checkout mutations. See [cli.rs:1322](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1322), source branch materialization at [cli.rs:1336](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1336), candidate apply at [cli.rs:1439](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1439), branch restore at [cli.rs:1614](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1614), and selected branch materialization at [cli.rs:1639](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1639).

Smallest wiring: introduce `ActiveParentRoot` immediately after `current_dir()` resolution and require it for calls that mutate the active checkout.

2. Child worktree authority

Risk: `GitWorktreeBackend::realize` is a good typed backend seam, but the live path discards its witness and persists only `workspace_root`. See realization at [prototype1_process.rs:529](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:529), status update at [prototype1_process.rs:542](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:542), and raw root persistence at [prototype1_process.rs:548](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:548).

Smallest wiring: return/pass a `ChildWorktree` or `StagedNode<ChildWorktree>` from staging into build/spawn, or reload and validate the same witness at build entry.

3. Attempt-scoped child result authority

Risk: child result paths are attempt-scoped, but writes are not capability-scoped. `record_attempt_runner_result` writes both `results/<runtime>.json` and latest `runner-result.json` at [prototype1_process.rs:500](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:500). Direct `--campaign/--node-id --execute` can still write results without a persisted invocation; runtime id falls back to a fresh value at [prototype1_process.rs:768](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:768), and the compatibility entrypoint is at [cli.rs:2037](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2037).

Smallest wiring: require an `AttemptCapability` produced from `ChildInvocation` for `record_attempt_runner_result` and `write_runner_result_at`.

4. Latest runner result overwrite authority

Risk: scheduler result functions accept node identity, not authority. `write_runner_result_at` writes an arbitrary path at [scheduler.rs:315](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs:315). `record_runner_result` writes latest node result and updates status at [scheduler.rs:470](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs:470). `clear_runner_result` deletes latest result by node id at [scheduler.rs:485](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs:485).

Smallest wiring: split these into parent-only clear/promote operations and child-only publish operation keyed by `AttemptCapability`.

5. Scheduler decision as successor capability

Risk: policy selection currently yields `Option<String>` branch id in the live loop at [cli.rs:1624](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1624). The branch id is selected/materialized directly at [cli.rs:1626](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1626) and [cli.rs:1639](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1639). `decide_continuation` accepts loose strings at [scheduler.rs:497](/home/brasides/code/ploke/crates/ploke-eval/src/intervention/scheduler.rs:497), and successor validation hard-codes `Some("keep")` at [prototype1_process.rs:246](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:246).

Smallest wiring: have policy produce `SelectedSuccessor` containing branch id, node id, generation, evaluation artifact/ref, and decision id. Active checkout update and successor handoff should consume that type.

6. Successor binary authority

Risk: successor handoff launches `current_exe()` rather than a binary built from the selected artifact. See [prototype1_process.rs:369](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:369). It only changes process current dir to the node worktree at [prototype1_process.rs:403](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:403).

Smallest wiring: make `spawn_and_handoff_prototype1_successor` take `SelectedSuccessor` plus `SuccessorBinary`, and remove its internal `current_exe()` lookup.

7. Handoff acknowledgement authority

Risk: parent accepts ready-file existence as handoff success. It removes stale ready file at [prototype1_process.rs:385](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:385), polls for existence at [prototype1_process.rs:325](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:325), and accepts `Some(_)` at [prototype1_process.rs:410](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:410). The ready record lacks selected-decision or binary digest fields at [prototype1_process.rs:227](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:227).

Smallest wiring: extend `SuccessorReadyRecord` with decision id, parent runtime id, successor binary digest, and active checkout head; validate these before returning `SuccessorHandoff`.

8. Old parent exit

Risk: successor invocation acknowledges then sleeps at [prototype1_process.rs:948](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:948). The typed command records handoff only in report fields after `Keep` at [cli.rs:2278](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2278). There is no `SuccessorHandoff -> ParentExit` transition.

Smallest wiring: add a tiny parent-only terminal transition that consumes `SuccessorHandoff` and `ParentControllerCapability`, records exit intent/status, and returns from the old parent command.

9. Cleanup authority

Risk: backend cleanup is safer than raw deletion, but it is not live-integrated. `GitWorktreeBackend::remove` verifies managed worktree identity at [backend.rs:465](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/backend.rs:465). The live loop restores active content at [cli.rs:1614](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:1614), but child worktree cleanup is not tied to observed child completion.

Smallest wiring: expose cleanup only as `cleanup_child_worktree(ParentControllerCapability, ObservedChild, CleanupPolicy)`, never as cleanup by path.

## Existing Types To Reuse

- `InvocationAuthority::{Child, Successor}` already separates executable roles at [invocation.rs:351](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/invocation.rs:351).
- `GitWorktreeBackend::realize` already verifies worktree reuse before mutation at [backend.rs:376](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/backend.rs:376).
- C1-C4 already model materialize/build/spawn/observe as move-only transitions, but this path is mainly wired through `Prototype1StateCommand`, not the main loop. See [cli.rs:2105](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2105).

## Recommended Order

1. Add `ActiveParentRoot` to the main loop and require it for active checkout mutations.
2. Carry/reload `ChildWorktree` authority between staging, build, and spawn.
3. Require `AttemptCapability` for child result writes.
4. Make scheduler selection return `SelectedSuccessor`.
5. Build and hand off a `SuccessorBinary` from the selected artifact.
6. Add the final `SuccessorHandoff -> ParentExit` record.
