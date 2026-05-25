# Prototype 1 Successor Rehydration Review

## Findings

1. **High: post-handoff successor failures are acknowledged as success and then become unobservable.** `execute_prototype1_successor_invocation` writes the ready record before running the rehydrated controller (`crates/ploke-eval/src/cli/prototype1_process.rs:991`-`crates/ploke-eval/src/cli/prototype1_process.rs:1002`), while the parent stops observing as soon as that ready file exists (`crates/ploke-eval/src/cli/prototype1_process.rs:375`-`crates/ploke-eval/src/cli/prototype1_process.rs:384`, `crates/ploke-eval/src/cli/prototype1_process.rs:464`-`crates/ploke-eval/src/cli/prototype1_process.rs:470`). The successor process is detached with stdout/stderr discarded (`crates/ploke-eval/src/cli/prototype1_process.rs:355`-`crates/ploke-eval/src/cli/prototype1_process.rs:361`). If `run_prototype1_successor_controller` fails after the acknowledgement, the parent has already persisted/printed `successor_handoff=acknowledged`, no runner-result or successor-completion artifact is written, and the process error is lost except for any external process status. For a bounded trampoline, the durable state needs to distinguish "handoff acknowledged" from "one bounded next-generation turn completed" and "successor crashed after handoff"; otherwise the chain can silently stop with a successful-looking handoff.

2. **High: successor authority is a mutable singleton decision, not an attempt-scoped handoff.** Validation accepts any invocation whose node branch matches `scheduler.last_continuation_decision.selected_next_branch_id` (`crates/ploke-eval/src/cli/prototype1_process.rs:282`-`crates/ploke-eval/src/cli/prototype1_process.rs:297`). The rehydrated controller is then rebuilt from the current node plus the scheduler policy (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:191`-`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:206`) and does not carry the validated decision, expected generation, or runtime id into `Prototype1LoopControllerInput::from_successor` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:137`-`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:187`). Any controller run that records or clears `last_continuation_decision` between spawn and successor validation can reject the intended successor or authorize the wrong attempt; once the successor starts staging nodes, `register_treatment_evaluation_node` clears the singleton decision (`crates/ploke-eval/src/intervention/scheduler.rs:655`-`crates/ploke-eval/src/intervention/scheduler.rs:669`). This is especially risky for future parallelism because the continuation authority is global per campaign instead of keyed by parent node/runtime.

3. **Medium: generational reports are overwritten instead of being durable per turn.** The shared controller always writes `prototype1-loop-trace.json` under the campaign directory (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:215`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:601`-`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:603`). A successor turn overwrites the parent turn's trace, so the handoff decision, selected branch, and next-generation report are not preserved as a sequence. That weakens auditability and recovery for the bounded trampoline, and it will make later multi-parent work harder because multiple successor turns would race on the same trace path.

4. **Medium: branch registry identity still collapses parent context for future parallel parents.** `record_synthesized_branches` identifies a source node only by `source_state_id` and `target_relpath` (`crates/ploke-eval/src/intervention/branch_registry.rs:204`-`crates/ploke-eval/src/intervention/branch_registry.rs:208`) and then overwrites `parent_branch_id` when a parent is provided (`crates/ploke-eval/src/intervention/branch_registry.rs:226`-`crates/ploke-eval/src/intervention/branch_registry.rs:232`). Current single-successor flow may avoid this in practice by using the parent branch id as the next `source_state_id` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:308`-`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:317`), but the data model is still not parent-attempt scoped. A duplicate or future parallel parent that synthesizes the same source state/target can rewrite ancestry and selected branch state for another parent.

5. **Medium: there are no tests covering successor rehydration semantics.** I found scheduler tests for node registration and continuation decisions, but no tests for `execute_prototype1_successor_invocation`, `run_prototype1_successor_controller`, or the shared controller's successor input path. The untested cases should include: rejecting a non-selected successor, binding the successor to the expected generation/decision, proving exactly one rehydrated controller turn runs without spawning another successor, persisting post-acknowledgement failure/completion, and preserving per-generation trace/history.

## Verification

`cargo check -p ploke-eval --locked` passes with pre-existing warnings.

## Orchestrator Follow-Up

After this review, findings 1 and 3 were addressed in the implementation:

- Successor invocations now write a `successor-completion/<runtime-id>.json`
  record after the rehydrated controller turn, including failed post-ready
  controller attempts.
- Successor controller turns now write attempt-scoped traces under
  `prototype1/loop-traces/` instead of overwriting the parent
  `prototype1-loop-trace.json`.

Findings 2, 4, and 5 remain follow-up work: durable attempt-scoped successor
selection/parent lease records, parent-scoped branch registry identity, and
direct tests for successor rehydration semantics.
