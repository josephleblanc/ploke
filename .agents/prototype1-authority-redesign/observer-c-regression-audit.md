# Observer C Regression Audit: Prototype 1 Authority Redesign

Focus: risks to watch while making Prototype 1 type design enforce operational authority.

Intended lifecycle: active parent root -> child worktrees -> child self-eval records -> policy selection -> active checkout update -> successor binary built from selected artifact -> handoff -> old parent exits.

## Main Regression Risks

1. Successor not built from selected artifact.
   - Current hot spot: `crates/ploke-eval/src/cli/prototype1_process.rs:360-418`.
   - Watch especially `std::env::current_exe()` around `:369`; that launches the old parent binary, not a successor built from the selected artifact.
   - Check: `rg -n "current_exe\\(|successor_binary|spawn_and_handoff_prototype1_successor|SuccessorInvocation" crates/ploke-eval/src`.
   - Expected after redesign: successor launch consumes a typed selected-artifact/checkout-updated/successor-built witness.

2. Policy selection reduced to per-node `keep`.
   - Current hot spots: `crates/ploke-eval/src/intervention/scheduler.rs:44-53`, `:140-155`; `crates/ploke-eval/src/cli/prototype1_process.rs:240-260`.
   - Risk: `decide_node_successor_continuation(..., Some("keep"))` is not proof that the active parent selected this node from all child self-evals.
   - Check: `rg -n "last_continuation_decision|record_continuation_decision|decide_node_successor_continuation|selected_next_branch_id|Some\\(\"keep\"\\)" crates/ploke-eval/src`.
   - Expected after redesign: successor authority requires a durable parent-owned selection record with selected node/artifact, policy id, evaluated result ids, and parent epoch/lease.

3. Legacy runner path bypasses capability layer.
   - Current hot spot: `crates/ploke-eval/src/cli.rs:1942-2040`.
   - Risk: `prototype1-runner --campaign --node-id --execute` can execute without an invocation capability, while invocation role dispatch exists separately at `:1952-1998`.
   - Check: `rg -n "prototype1-runner|execute_prototype1_runner_node|--campaign|--node-id|load_executable|InvocationAuthority" crates/ploke-eval/src`.
   - Expected after redesign: executable child/successor paths require role-specific invocation/capability files; raw campaign/node ids are inspect-only or removed.

4. Child can still mutate parent-owned state.
   - Current hot spots: `crates/ploke-eval/src/cli/prototype1_process.rs:763-900`; exported scheduler mutation functions in `crates/ploke-eval/src/intervention/mod.rs:38-50`.
   - Risk: child authority should be self-evaluate + record bounded result, not update scheduler/frontier/selection/branch registry.
   - Check: `rg -n "update_node_status|record_continuation_decision|update_scheduler_policy|mark_treatment_branch_applied|restore_treatment_branch|select_treatment_branch|register_treatment" crates/ploke-eval/src`.
   - Expected after redesign: child code receives only an attempt-scoped result sink; parent-only mutations require an active-parent authority type.

5. Path-shaped records can forge authority.
   - Current hot spots: `crates/ploke-eval/src/intervention/scheduler.rs:74-138`; `crates/ploke-eval/src/cli/prototype1_state/backend.rs:47-76`; `crates/ploke-eval/src/cli/prototype1_state/c2.rs:269-285`.
   - Risk: persisted `workspace_root`, `binary_path`, `runner_result_path`, or `target_relpath` can point outside the node/worktree unless revalidated at use.
   - Check: `rg -n "workspace_root|binary_path|runner_request_path|runner_result_path|node_dir|target_relpath" crates/ploke-eval/src/cli/prototype1_state crates/ploke-eval/src/intervention`.
   - Expected after redesign: paths are validated against campaign root, node dir, runtime id, and backend metadata; `target_relpath` cannot be absolute or escape via `..`.

6. Journal/replay does not cover the full authority lifecycle.
   - Current hot spots: `crates/ploke-eval/src/cli/prototype1_state/journal.rs:163-172`; replay starts around `:223`; known persistence gaps in `crates/ploke-eval/src/cli/prototype1_state/mod.rs:232-244`.
   - Risk: materialize/build/spawn/ready/observe are recorded, but selection, checkout update, successor build, handoff, parent exit, and lease/epoch may remain convention-only.
   - Check: `rg -n "JournalEntry|replay_|Successor|Selected|Checkout|Handoff|Exit|epoch|lease" crates/ploke-eval/src/cli/prototype1_state crates/ploke-eval/src/intervention`.
   - Expected after redesign: every authority transition has a journal variant and replay classifier.

7. Handoff acknowledgement mistaken for authority transfer.
   - Current hot spots: `crates/ploke-eval/src/cli/prototype1_process.rs:918-949`; standby sleep at `:948`; successor-ready schema in `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:307-319`.
   - Risk: successor-ready plus sleep is not proof that the successor owns the campaign or that the old parent exited.
   - Check: `rg -n "SUCCESSOR_STANDBY_TIMEOUT|SuccessorReadyRecord|successor-ready|execute_prototype1_successor_invocation|sleep\\(" crates/ploke-eval/src`.
   - Expected after redesign: successor-ready is bound to parent epoch and selected artifact, followed by durable authority transfer and old-parent exit record.

## Compile/Type Checks

- Run `cargo check -p ploke-eval --tests`.
- Run `cargo test -p ploke-eval --tests`.
- Run `cargo test --all-targets --locked` before merging broad type-state changes.
- Search for accidental weak capability types: `rg -n "derive\\(.*Default|pub .*Authority|struct .*Authority|Capability|Lease|Epoch|allow\\(dead_code\\)" crates/ploke-eval/src/cli/prototype1_state crates/ploke-eval/src/cli/prototype1_process.rs`.
- Audit role-specific loaders around `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:254-289`; `load_executable` currently returns either role, so dispatch must not become generic.

## Privacy/API Checks

- Build/treatment failure excerpts persist stdout/stderr: `crates/ploke-eval/src/cli/prototype1_process.rs:118-138`, `:441-442`; `crates/ploke-eval/src/cli/prototype1_state/c2.rs:45-72`. Confirm redaction before durable write.
- OpenRouter embedding sends snippet batches in `crates/ingest/ploke-embed/src/providers/openrouter.rs:207-227`. Child/successor authority should not imply permission for remote API calls unless explicitly granted.
- Debug logs include absolute paths, binary paths, journal paths, ids, and PIDs around `prototype1_process.rs:173-180`, `:392-400`, `:1043-1052`, and `cli.rs:2125-2131`. Confirm these are acceptable in `~/.ploke-eval/logs`.

## Lifecycle Questions To Recheck

- Can a child reach any scheduler, branch registry, checkout, selection, or parent lease mutation?
- Can a successor launch before durable policy selection and active checkout update?
- Is the successor binary built from the selected artifact rather than the old parent executable?
- Are child results attempt-scoped by runtime id, not just latest node-level result files?
- Can two parents believe they own the same campaign?
- Can an invocation be replayed after the parent epoch changes?
- Does recovery distinguish spawned-not-ready, ready-not-terminal, selected-not-checked-out, checked-out-not-built, successor-ready, and old-parent-exited?
