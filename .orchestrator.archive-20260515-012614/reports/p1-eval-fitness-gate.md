# p1-eval-fitness-gate

Changed files:
- crates/ploke-eval/src/branch_evaluation.rs

Summary:
- Added branch evaluation comparisons for `tool_calls_failed`, `aborted`, and `nonempty_valid_patch`.
- `tool_calls_failed` is lower-is-better.
- `aborted` prefers false.
- `nonempty_valid_patch` prefers true.
- Added focused tests covering each new gate.

Verification:
- `cargo test -p ploke-eval branch_evaluation 2>&1 | tail -n 80`
- Passed: 8 passed, 0 failed.

Blockers:
- None.
