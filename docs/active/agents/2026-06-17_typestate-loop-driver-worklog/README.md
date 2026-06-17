# 2026-06-17 Typestate Loop Driver Worklog

Short description: running implementation notes for the Prototype 1 typestate loop driver migration.

Status: active worklog. This records slice intent, decisions, test failures, recovery actions, and commit checkpoints while implementing [`../2026-06-17_typestate-loop-driver-plan.md`](../2026-06-17_typestate-loop-driver-plan.md).

Related:

- [`../2026-06-17_typestate-loop-driver-plan.md`](../2026-06-17_typestate-loop-driver-plan.md)
- [`../2026-06-16_walk-server.md`](../2026-06-16_walk-server.md)
- [`../2026-06-17_walk-command-guide.md`](../2026-06-17_walk-command-guide.md)
- `crates/ploke-eval/src/cli/prototype1_state/typestate/`
- `crates/ploke-eval/src/cli/prototype1_state/walk/`

## Operating rhythm

- Keep changes small enough to commit after each recoverable milestone.
- Prefer existing live `prototype1-state` semantics unless a typed invariant forces a correction.
- Do not push from this agent session.
- Use `git status` in addition to GitNexus checks because generated/untracked/string-only changes can be missed.
- Use the walk commands for smoke/use-testing when a slice touches operator behavior.
- Preserve a note here whenever a test fails, a workaround is needed, or an invariant is intentionally kept strict.

## Slice log

### Slice 0 — plan and inventory checkpoint

Started after user confirmed autonomous implementation. Initial focus:

1. Inventory current direct live edges and walk-supported phases.
2. Identify the first durable reconstruction seam for R0-R5.
3. Add only enough driver scaffolding to let `walk show` prefer durable reconstruction where the evidence exists.

Open notes:

- User notes say ADR 007/genesis History admission should come after durable R0-R5 and coarse driver work.
- Provider calls, child spawns, and History sealing are allowed by default for the walker once the corresponding typed edge admits them.
- Checkout installs should require an explicit `--allow git-changes` style admission before being made available from `walk`.
- Parent/successor handoff remains the main high-consequence invariant: only one active runtime on a lineage should have permissions to append History, spawn successor parent processes, and mutate checkout/worktree state.

### Slice 1 — early durable reconstruction for `walk`

Implemented first driver seam:

- added `crates/ploke-eval/src/cli/prototype1_state/driver/`;
- added side-effect-free `driver::reconstruct::reconstruct_early` for R1/R3/R4a/R4b/R4c/R5;
- reconstruction builds R1 without calling live `r0_to_r1`, because that edge can update monitor/closure state;
- R4c -> R5 reconstruction checks for matching `ParentStarted` journal evidence instead of calling `r4c_to_r5`, because `r4c_to_r5` appends journal/resource records;
- `walk show` refreshes from durable evidence when the server has no in-memory state;
- `walk step` also refreshes from durable evidence before choosing the next step;
- `walk show` and `walk step` now auto-spawn the local server like `walk start`, using the default idle TTL.

Deliberate limitations:

- no ADR 007/genesis History migration yet;
- no persisted R0 command context yet, so restart reconstruction uses parent identity/campaign/profile plus default command settings;
- no predecessor handoff-invocation reconstruction yet, because the invocation path is not discoverable from durable context in this slice;
- R4a checkout mismatch remains a hard blocker and is displayed in reconstruction notes, but the richer typed blocker UX is left to Slice 2.

Validation:

GitNexus `detect-changes` risk: HIGH, because this slice intentionally changes `walk/client.rs::run` and `walk/controller.rs` dispatch behavior, affecting multiple walk flows. No non-walk execution process is expected to change. Focused walk/typestate/prototype1 tests and isolated socket smokes passed.

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo test -p ploke-eval prototype1_state --all-targets
cargo build -p ploke-eval
```

Smoke:

```text
PLOKE_EVAL_WALK_SOCKET_DIR=<tmp> ./target/debug/ploke-eval loop walk show --repo-root /home/brasides/code/ploke
```

Result: auto-spawned server, reconstructed to R4a, and preserved a hard checkout blocker because the development checkout is dirty/mismatched.

```text
PLOKE_EVAL_WALK_SOCKET_DIR=<tmp> ./target/debug/ploke-eval loop walk show --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
```

Result: auto-spawned server and reconstructed to R5 from parent-start journal evidence.

```text
PLOKE_EVAL_WALK_SOCKET_DIR=<tmp> ./target/debug/ploke-eval loop walk step --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
```

Result: auto-spawned server, reconstructed R5, and reported no next admitted step in the current server slice.
