# Worker A Authority Redesign Report

## Changed files

- `crates/ploke-eval/src/cli/prototype1_state/authority.rs`
- `.agents/prototype1-authority-redesign/worker-a-authority.md`

## Semantic constraints encoded

- Distinct private-field witnesses separate active checkout root, child worktree root, shared record root, selected successor, completed child attempt, and verified active root.
- Parent, child, and successor-bootstrap authority are separate move-only tokens rather than cloneable/shared role labels.
- Child worktree roots require parent authority to validate, preventing arbitrary relabeling of workspace paths as candidate worktrees.
- Shared record root validation keeps append-only external state outside the active checkout.
- Child completion consumes child authority and bounds terminal result records to the shared record root.
- Parent selection requires a completed child attempt before producing a selected successor.
- Successor bootstrap can acknowledge handoff, but parent authority is only produced after active-root/binary validation.

## Compile assumptions

- `authority.rs` is additive and not wired into `mod.rs`, per task constraints, so it is not compiled by the crate yet.
- The module assumes `super::event::RuntimeId` remains the runtime identity carrier.
- Path validation uses existing filesystem state and `std::fs::canonicalize`.
- Result records may be validated before the result file exists, but their parent directory must exist for canonical bounded-path validation.

## Integration points

- Wire the module in `prototype1_state/mod.rs` when allowed.
- Convert controller construction of active/shared paths to `ActiveRoot`, `SharedRoot`, and `VerifiedActive`.
- Wrap backend-realized candidate worktrees with `ChildRoot::validate_from_parent`.
- Use `Parent::child` at child invocation spawn and `Child::complete` at runner result recording.
- Use `Parent::select` after observing completed attempts, then pass `Selected` into successor bootstrap.
- Promote successor bootstrap with active-root/binary validation before granting new parent authority.
