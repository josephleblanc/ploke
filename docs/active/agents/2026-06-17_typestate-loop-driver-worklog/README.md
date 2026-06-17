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

### Slice 2 — R4a checkout blocker UX

Implemented typed/operator-facing R4a blocker display without relaxing validation:

- added read-only `GitWorktreeBackend::active_branch` and `GitWorktreeBackend::dirty_paths` accessors;
- added `driver::reconstruct::format_r4a_blocker`;
- reconstruction blockers now show:
  - blocked edge;
  - original strict reason;
  - repo root;
  - active branch;
  - expected parent identity artifact branch;
  - dirty paths when present;
  - recovery commands;
- live `walk step` failures from R4a return the same typed blocker detail.

Validation:

```text
cargo check -p ploke-eval --all-targets
cargo build -p ploke-eval
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval prototype1_state --all-targets
```

Smoke:

```text
PLOKE_EVAL_WALK_SOCKET_DIR=<tmp> ./target/debug/ploke-eval loop walk show --repo-root /home/brasides/code/ploke
PLOKE_EVAL_WALK_SOCKET_DIR=<tmp> ./target/debug/ploke-eval loop walk step --repo-root /home/brasides/code/ploke
```

Result: both commands preserved the hard R4a stop and printed active branch, expected artifact branch, dirty files, and recovery commands.

### Slice 3 — admit R6 parent baseline in `walk`

Implemented the next coarse driver edge:

- converted the R6 alias to `runtime_alias!` and exported `R6_SHAPE`;
- added read-only `load_parent_baseline_for_id(...)` beside the live baseline-establishment helpers;
- reconstruction now promotes R5 to R6 when matching parent-start journal evidence and durable parent baseline evidence exist;
- live `walk step` now admits the canonical async `r5_to_r6` direct edge;
- walk output/deltas now include R6 and the R5 -> R6 side-effect note.

Deliberate limitations:

- R5 -> R6 may still run the existing live baseline establishment path when durable baseline evidence is missing; this preserves current `prototype1-state` behavior rather than inventing a weaker fallback;
- R7+ remain blocked by the debug server slice until policy/budget reconstruction is added;
- `walk start --until r6` advances through R5 -> R6 but does not create a last-step delta; single-step `walk step` from R5 does.

Validation:

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
```

Smoke with isolated socket and clean parent worktree:

```text
./target/debug/ploke-eval loop walk start --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock --until r6
./target/debug/ploke-eval loop walk show --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk start --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock --until r5
./target/debug/ploke-eval loop walk step --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk show --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock delta --no-color
```

Result: R6 typestate displayed as `Evidence<..., evidence::baseline::Ready<CompleteBaseline>, ...>`, and the single-step delta showed `r5_to_r6`.

### Slice 4 — admit R7 policy/budget readiness in `walk`

Implemented the next read-only driver edge:

- converted the R7 alias to `runtime_alias!` and exported `R7_SHAPE`;
- added shared `resolve_parent_policy_budget(...)` for the existing R6 -> R7 logic;
- refactored canonical `r6_to_r7` to call the shared helper;
- reconstruction now promotes R6 to R7 when policy and child-planning budget inputs can be derived;
- `walk step` now admits canonical `r6_to_r7`;
- walk output/deltas now include R7 as the current server boundary.

Deliberate limitations:

- if policy/budget validation fails, reconstruction stops at R6 and reports a blocked `r6 -> r7` edge rather than weakening budget or max-generation checks;
- R8 remains blocked because child-plan authority can publish/receive messages and needs the next explicit live-admission slice.

Validation:

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo test -p ploke-eval prototype1_state --all-targets
```

Smoke with isolated socket and clean parent worktree:

```text
./target/debug/ploke-eval loop walk start --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock --until r7
./target/debug/ploke-eval loop walk show --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk reset --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk start --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock --until r6
./target/debug/ploke-eval loop walk step --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk show --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock delta --no-color
```

Result: R7 typestate displayed `evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>`, and the single-step delta showed `r6_to_r7`.

### Slice 5 — reconstruct R8 child-plan authority only

Implemented the next durable/replay boundary without admitting the long live child-plan edge yet:

- converted the R8 alias to `runtime_alias!` and exported `R8_SHAPE`;
- added read-only helpers to locate, validate, and load an existing child-plan message for a ready parent;
- widened `ChildPlanFiles::validate_receiver` to `pub(crate)` so the same receiver invariant can be reused before consuming the parent typestate value;
- reconstruction now promotes R7 to R8 when an existing child-plan message is present and valid;
- `walk show` can display R8 typestate and R8 deltas from shape metadata;
- live `walk step` still stops at R7 until async progress/watch semantics are implemented for `r7_to_r8`.

Deliberate limitations:

- no provider call, broad harness publication, or child-plan message creation is performed by `walk` in this slice;
- malformed existing child-plan evidence blocks at R7 instead of being tolerated;
- R9+ remain blocked.

Validation:

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo test -p ploke-eval prototype1_state --all-targets
```

Smoke with isolated socket and clean parent worktree without child-plan evidence:

```text
./target/debug/ploke-eval loop walk start --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock --until r7
./target/debug/ploke-eval loop walk step --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk show --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
```

Result: `walk` stayed at R7 and did not call the live R8 child-plan authority edge.

### Slice 6 — add explicit `walk step --watch` for live R7 -> R8

Added an explicit operator gate for the first long child-plan authority edge:

- added `walk step --watch` to the public command;
- extended the walk protocol `Step` request with `watch` and bumped protocol version to 3;
- default `walk step` at R7 remains a no-op safe boundary and does not call providers or publish child-plan work;
- `walk step --watch` admits canonical `r7_to_r8` and waits for it to complete;
- `walk show` advertises `r7_to_r8 --watch -> r8` and `walk show delta` knows the R7 -> R8 side-effect text.

Deliberate limitations:

- this is a blocking watch, not yet a background progress task with `walk show progress`;
- default return-after-~1s progress semantics are still pending for unattended live edges; the safer current behavior is to require `--watch` before starting R7 -> R8;
- live `--watch` was not smoke-run in this slice to avoid invoking provider/harness work unexpectedly from the clean local parent worktree.

Validation:

```text
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
```

Smoke with isolated socket and clean parent worktree:

```text
./target/debug/ploke-eval loop walk start --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock --until r7
./target/debug/ploke-eval loop walk show --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk step --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249 --socket <tmp>/walk.sock
./target/debug/ploke-eval loop walk step --help | rg -- '--watch|--until'
```

Result: show advertised `r7_to_r8 --watch`, default step stayed at R7, and help listed `--watch`.

GitNexus `detect-changes` risk: HIGH, because the slice intentionally changes walk client `run` dispatch, the walk protocol, and controller stepping. Affected flows are walk socket/context flows; no non-walk execution process was reported. The change was kept as a recoverable milestone and validated with focused tests plus isolated socket smokes.

### Slice 7 — admit R9 schedule shaping in `walk`

Admitted the pure schedule-shaping edge after child-plan authority:

- converted the R9 alias to `runtime_alias!` and exported `R9_SHAPE`;
- added `WalkState::R9` and `WalkPhase::R9`;
- wired canonical `r8_to_r9` into `WalkController`;
- `walk show` at R8 now advertises `r8_to_r9 -> r9`;
- `walk step` from reconstructed/in-memory R8 advances to R9 without `--watch` because the edge is pure schedule shaping;
- R9 has no admitted next edge in this slice, so R10+ child fanout/selection remains unavailable;
- bumped the walk transition graph version to `walk-r0-r9-v1` so stale pre-R9 servers fail closed.

Deliberate limitations:

- no R10 selection-strategy construction yet;
- no child fanout, rejected-only projection, successor decision, handoff, or final report admission yet;
- no live `--watch` smoke was run for R7 -> R8 in this slice; the R9 smoke used existing child-plan evidence and did not invoke provider/harness work.

Validation:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
```

Focused test addition:

- `walk::phase::tests::r8_advertises_r9_schedule_step`
- `walk::phase::tests::r9_shape_records_schedule_ready_delta`

Smoke with isolated socket and clean parent worktree that already has matching child-plan evidence:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260604-221908
sockdir=$(mktemp -d /tmp/ploke-walk-r9.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" --with-version
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" delta --verbose --no-color
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: `show` reconstructed R8 from existing child-plan message evidence, advertised `r8_to_r9 -> r9`, `step` advanced to `r9 - child schedule ready`, and verbose delta showed `Plan<..., plan::schedule::None>` changing to `Plan<..., plan::schedule::Ready<Prototype1ChildBudget, Prototype1ChildScheduleMode>>`.

GitNexus impact checks before edits were LOW for `WalkPhase`, `r8_to_r9`, `R9`, and `TRANSITION_GRAPH_VERSION` (0 indexed direct callers/processes reported for each checked target). Pre-commit `npx gitnexus detect-changes --repo ploke` reported LOW risk, 9 changed files, 31 changed symbols, and 0 affected processes.
