# Prototype 1 Control Commands Review

Short review of the config-driven `prototype1-doctor` / `prototype1-continue` / `prototype1-step` implementation added under `crates/ploke-eval/src/cli/prototype1_state/run/`.

Review source: delegated `gpt-5.5` (`xhigh`) code review, with local verification of the cited code paths before recording this report.

Review target:

- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/run/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`
- `crates/ploke-eval/src/cli/prototype1_state/successor.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`

## Findings

1. **High: resume blocks terminal failed children that never produce `runner-result.json`.**

   `load_child_snapshots` treats any terminal node without `runner-result.json` as contradictory and pushes a blocker at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:738`. That is too strong for failures produced before runner execution completes. The extracted resume path already treats build rejection as a non-advanced outcome at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:1049`, and existing typed child transitions can persist `Failed` without a runner result. As written, `prototype1-continue` can refuse exactly the resumed failure states it is supposed to diagnose.

2. **High: handoff recomputes selection instead of consuming the sealed selection already written to History/journal.**

   `latest_successor_marker` reduces a persisted successor record to a phase-only marker and discards the actual selection payload at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:837`. `advance_handoff` then calls `select_successor_for_profile` again at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:1369`. That lets the handoff phase diverge from the previously recorded selection if policy inputs, evidence, or replayed state changed between `select` and `handoff`. In the degenerate case, recomputation can return `None`, causing handoff to no-op while `prototype1-continue` keeps retrying until its 256-iteration guard trips.

3. **High: successful children with a missing evaluation report are silently downgraded to reject on resume.**

   `reconstruct_terminal_outcomes` maps `Succeeded` plus missing `evaluation_report` to `completed:Reject` at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:1287`, and leaves `selection_input` absent at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:1316`. If the parent crashes after the child succeeds but before `compare_observed_child_treatment` persists the branch evaluation report, resume will not recover that compare boundary; it can convert a keep-worthy child into rejection and eventually conclude `selection resolved to none`.

4. **Medium: `run-control.toml` mode is loaded and reported but does not affect `prototype1-continue`.**

   The control surface parses `mode` and exposes it in status, but `resume` always advances with `ExecuteMode::Continuous` at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:222` and `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:244`. The only current proof for `mode = "step"` is the TOML load test. If operator-local control is supposed to own a default execution mode, this implementation does not yet honor it.

5. **Medium: setup can produce campaigns that the new control commands refuse to operate on.**

   `prototype1-setup` still allows `admitted_profile` to be absent at `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:217`, using CLI-derived search policy only to write the new default run control. But the new control path unconditionally requires an admitted run profile at `crates/ploke-eval/src/cli/prototype1_state/run/core.rs:348`. That creates a setup/control mismatch: a campaign can be created successfully and still be undiscoverable by `prototype1-doctor`, `prototype1-continue`, and `prototype1-step`.

6. **Low: newly unused successor helper violates the repo dead-code rule.**

   `State::allows_successor_handoff` is now unused at `crates/ploke-eval/src/cli/prototype1_state/successor.rs:68`. The repo rule requires intentionally unused code to carry a `#[allow(dead_code, reason = "task-stack:...")]` marker tied to a live task-stack item, or else to be removed.

## Residual Risk

- The new commands were verified by parser tests, focused run-control tests, and `cargo check`, not by live resume/doctor/step execution against persisted Prototype 1 campaigns.
- The planned diagnosis fixture matrix is still missing, so contradiction handling, phase recovery, and select/handoff continuity remain lightly tested.
