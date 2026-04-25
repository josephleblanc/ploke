# Prototype 1 Typestate Hardening Review

The previous high-severity timeout/terminality issue appears addressed in the current diff:

- spawn timeout no longer records a fake terminal observation in [c3.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs:588)
- completion timeout is again a rejected path in [c4.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c4.rs:372)
- the CLI state surface again reports completion timeout as rejected in [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:2200)

## Findings

### 1. Medium: timeout decisions are now nonterminal, but they are not journaled, so restart/recovery loses the fact that a timeout policy already fired

- In [c3.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs:297), `ReadyTimedOut` deliberately maps to no `SpawnObservation`, and [c3.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs:588) only appends an observed spawn entry for `ExitedBeforeReady`.
- In [c4.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c4.rs:372), result timeout returns `Outcome::Rejected` without any `ObserveChild` after-record.
- Replay therefore collapses both cases back to generic pending states: [replay_spawn_child](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/journal.rs:350) can only report `SpawnedUnacknowledged` or `AcknowledgedUnobserved`, and [replay_observe_child](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/journal.rs:409) can only report `ResultPending` or `TerminalResultWrittenUnobserved`.
- That means a restarted controller cannot distinguish “still waiting” from “we already timed out and deliberately abandoned this wait,” so timeout policy is not restart-stable even though the journal is supposed to be the durable transition substrate.

### 2. Medium: spawn replay still accepts contradictory histories where a child is recorded as terminated-before-ready and ready anyway

- [wait_for_ready](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c3.rs:646) checks the journal and then polls process exit each loop. A child can append `ChildReady` and exit between those two observations, causing the parent to record `ExitedBeforeReady`.
- If that happens, the journal can legitimately contain `Spawned`, then `Observed(TerminatedBeforeAcknowledged)`, and later a `ChildReady`.
- [replay_spawn_child](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/journal.rs:340) currently treats any `spawned + observed + optional ready` triple as a committed spawn without validating that `ready` is incompatible with `TerminatedBeforeAcknowledged`.
- This leaves replay willing to normalize a lifecycle contradiction instead of surfacing it as inconsistent state.

## Residual Risks And Testing Gaps

- [c4.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/c4.rs:89) still equates “runner result observed” with `ChildRuntimeLifecycle::Terminated`. The child writes its runner result before process return in [prototype1_process.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_process.rs:690), so the lifecycle name is still slightly stronger than what the parent has actually observed.
- [invocation.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/invocation.rs:59) still exposes `Invocation` fields publicly. The constructor/persistence path is much tighter now, but the wire type itself is not yet a private/trusted carrier.
- I did not find tests covering:
  `load_executable_child` rejecting a persisted `Successor`, timeout restart behavior, or contradictory `Spawned + Observed(TerminatedBeforeAcknowledged) + ChildReady` replay.

## Recommended Next Steps

1. Persist timeout as an explicit nonterminal observation instead of dropping it on the floor.
   That keeps the fix to false terminality while making timeout policy restart-safe.
2. Teach spawn replay to reject contradictory histories.
   In particular, `ready` should not coexist with `Observed(TerminatedBeforeAcknowledged)` without being flagged inconsistent.
3. Decide whether `ChildRuntimeLifecycle::Terminated` means “process exit observed” or “terminal result observed,” then rename or tighten the carrier accordingly.
4. Add tests for successor rejection, timeout recovery, and contradictory spawn history replay.
