Completed:
- Ambiguous agent-turn evidence remains an agent-turn evidence subject with
  loaded-summary and artifact locators.
- Nested `nodes/<node-id>/...` artifacts may receive a scheduler-node locator
  and branch/candidate context when known.
- Agent-turn artifacts are no longer attached to runtime nodes or operation
  nodes through a weak node-id join.
- Root-level agent-turn artifacts remain evidence-only.

Changed files:
- `crates/ploke-tree/src/graph/build.rs`
- `crates/ploke-tree/src/graph/build/passive.rs`
- `crates/ploke-tree/src/graph/build/run_attempts.rs`

Checks:
- `cargo fmt -p ploke-tree` passed.
- `cargo test -p ploke-tree graph -- --nocapture 2>&1 | tail -n 80` passed.
- `cargo check -p ploke-tree 2>&1 | tail -n 80` passed.
- `git diff --check -- ...graph/build.rs ...graph/build/passive.rs ...graph/build/run_attempts.rs` passed.
