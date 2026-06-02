# Parent Audit: Prototype 1 Authority Redesign

## Integrated

- Wired `prototype1_state::authority` into the typed state module.
- Added capability/root carriers for active checkout, shared record root, child worktree root, completed attempt, selected successor, successor bootstrap, verified active root, cleanup, and parent exit.
- Tightened the worker draft so:
  - child completion requires child acknowledgement
  - parent selection accepts only accepted child attempts
  - successor bootstrap must acknowledge before it can become parent authority
- Updated `workspace.rs` so the workspace sketch carries authority/root witnesses instead of raw branch/root fields for select, update, build, cleanup, active checkout, child worktree, and shared paths.
- Added unit tests for the three main authority gates.

## Verified

- `cargo check -p ploke-eval --tests`
- `cargo test -p ploke-eval authority`

Both passed with existing unrelated warnings.

## Deferred

- Main live loop still uses raw active checkout paths and loose branch ids.
- Successor handoff still needs to stop using `current_exe()` and instead consume a successor binary built from the selected artifact.
- Child result writes still need an invocation/attempt capability in the live path.
- Scheduler selection still needs a durable selected-successor record with decision provenance.
- Full lifecycle journal/replay still lacks selection, active-checkout update, successor build, handoff transfer, parent exit, lease, and epoch events.
