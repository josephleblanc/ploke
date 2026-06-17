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

### Slice 8 — admit R10 selection strategy in `walk`

Admitted the next pure strategy-construction edge:

- converted the R10 alias to `runtime_alias!` and exported `R10_SHAPE`;
- added `WalkState::R10` and `WalkPhase::R10`;
- wired canonical `r9_to_r10` into `WalkController`;
- `walk show` at R9 now advertises `r9_to_r10 -> r10`;
- `walk step` from R9 advances to `r10 - selection strategy ready` without `--watch` because the edge is pure in-process strategy construction;
- R10 has no admitted next edge in this slice, so R11+ fanout/rejected-only phases remain unavailable;
- bumped the walk transition graph version to `walk-r0-r10-v1` so stale pre-R10 servers fail closed.

Deliberate limitations:

- no R11 rejected-only branch or child fanout admission yet;
- no child spawning, checkout install, History sealing, successor decision, handoff, or final report admission yet;
- R10 is not yet reconstructed directly from durable evidence; after restart, current behavior reconstructs R8 from child-plan evidence, then steps through R9/R10.

Validation:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo build -p ploke-eval
```

Focused test addition:

- `walk::phase::tests::r9_advertises_r10_strategy_step`
- `walk::phase::tests::r10_shape_records_selection_strategy_delta`

Smoke with isolated socket and clean parent worktree that already has matching child-plan evidence:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260604-221908
sockdir=$(mktemp -d /tmp/ploke-walk-r10.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" --with-version
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" delta --verbose --no-color
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: `show` reconstructed R8, first `step` advanced `r8_to_r9`, second `step` advanced `r9_to_r10`, and verbose delta showed `evidence::selection::Plan<context::ChildPlanFacts>` changing to `evidence::selection::Strategy` with no admitted next step.

Explicit live API smoke after user clarified live calls should be used:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
sockdir=$(mktemp -d /tmp/ploke-walk-live-r8.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" --with-version
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: the live R7 -> R8 child-planning path ran and produced child-plan/harness artifacts, but the request returned a strict error because broad harness admitted 4 child transactions, below required minimum 5:

```text
batch selection is invalid: broad harness admitted 4 child transaction(s), fewer than required minimum 5
```

Follow-up `walk show` from a fresh socket reconstructed R8 from the newly written child-plan message, and `walk step --until r10` advanced through `r8_to_r9` and `r9_to_r10`. The below-min admission invariant was not weakened.

GitNexus impact checks before edits were LOW for `R10`, `r9_to_r10`, `WalkPhase`, `WalkController`, and `TRANSITION_GRAPH_VERSION` (0 indexed direct callers/processes reported for each checked target). Pre-commit `npx gitnexus detect-changes --repo ploke` reported LOW risk, 9 changed files, 31 changed symbols, and 0 affected processes.

### Slice 9 — admit watch-gated R11 rejected-only/fanout branch in `walk`

Admitted the first post-strategy branch only behind an explicit operator gate:

- converted `R11aRejectedOnly` and `R11FanoutComplete` to `runtime_alias!` and exported `R11A_SHAPE` / `R11_SHAPE`;
- added `WalkState::R11a`, `WalkState::R11`, `WalkPhase::R11a`, and `WalkPhase::R11`;
- wired canonical async `r10_to_r11` into `WalkController`, but only when `walk step --watch` is present;
- default `walk step` at R10 remains safe/no-op and does not project rejected-only evidence or run child fanout;
- `walk show` / `walk step` at R10 advertise both possible `r10_to_r11 --watch` branches;
- R11a and R11 are current stops; R12+ remains unavailable;
- bumped the walk transition graph version to `walk-r0-r11-v1` so stale pre-R11 servers fail closed.

Deliberate limitations:

- no R12 report/outcome projection yet;
- no successor decision, handoff, checkout install, History sealing, or final report admission yet;
- live fanout keeps strict child terminal/evaluation requirements; missing branch evaluation reports block instead of being tolerated;
- R11 states are not reconstructed directly from durable evidence yet. Restart recovery still reconstructs to R8 where child-plan evidence exists, then steps through R9/R10 before a fresh `--watch` R11 attempt.

Validation:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo build -p ploke-eval
```

Focused test additions:

- `walk::phase::tests::r10_advertises_watch_gated_r11_steps`
- `walk::phase::tests::r11_shapes_record_branch_deltas`

Safe/default smoke with isolated socket and a clean parent worktree with child-plan evidence:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
sockdir=$(mktemp -d /tmp/ploke-walk-r11-safe.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r10
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: the first command reached `r10 - selection strategy ready` and advertised `r10_to_r11 --watch`; the default step stayed at R10 with `transition: already at requested phase` and did not start fanout.

Explicit live R10 -> R11a smoke:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
sockdir=$(mktemp -d /tmp/ploke-walk-live-r11.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r10
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch
./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" delta --verbose --no-color
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: `walk step --watch` advanced `r10_to_r11 --watch` to `r11a - rejected-only selection evidence ready`. The delta showed `Children<... Planned<ChildFiles> ...>` changing to `Children<children::set::Rejected, ...>` and `evidence::selection::Strategy` changing to `evidence::selection::Evidence<SelectionSealMaterial>`.

Explicit live R10 -> R11 fanout attempt on a clean worktree with children:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-2g1x3-20260525-073410
sockdir=$(mktemp -d /tmp/ploke-walk-live-r11fanout.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r10
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: the fanout attempt reached R10 and then failed strictly with:

```text
terminal child 'node-11fa531aab94448c' is missing branch evaluation report; run observe recovery before direct prototype1-state re-entry
```

This preserved the existing child terminal/evaluation invariant; no permissive fallback was added.

GitNexus impact checks before edits were LOW for `R11aRejectedOnly`, `R11FanoutComplete`, `R10FanoutBranch`, `r10_to_r11`, `WalkPhase`, `WalkController`, and `TRANSITION_GRAPH_VERSION`. Pre-commit `npx gitnexus detect-changes --repo ploke` reported LOW risk, 9 changed files, 41 changed symbols, and 0 affected processes.

### Slice 10 — admit R12 report-facts projection in `walk`

Admitted the pure report-facts projection after R11 branches:

- converted `R12` to `runtime_alias!` and exported `R12_SHAPE`;
- added `WalkState::R12` and `WalkPhase::R12`;
- wired canonical `r11_to_r12` into `WalkController` from both in-memory R11 branches by re-wrapping the typed `R10FanoutBranch` carrier;
- `walk show` at R11a/R11 advertises `r11_to_r12 -> r12`;
- default `walk step` from R11a/R11 advances to `r12 - report facts ready` because the edge only projects report facts and does not emit the final report;
- R12 is the current stop; R13+ continuation/handoff remains unavailable;
- bumped the walk transition graph version to `walk-r0-r12-v1` so stale pre-R12 servers fail closed.

Deliberate limitations:

- no R13 stopped/successor continuation yet;
- no History head read, History sealing, checkout install, successor handoff, or final report emission yet;
- R12 is not reconstructed directly from durable evidence yet. Restart recovery still reconstructs to R8 where child-plan evidence exists, then steps through R9/R10 and requires fresh in-memory R11 before R12;
- missing report inputs still fail through the canonical `r11_to_r12` edge; no fake child outcomes are invented.

Validation so far:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo build -p ploke-eval
```

Focused test additions:

- `walk::phase::tests::r11_branches_advertise_r12_report_projection`
- `walk::phase::tests::r12_shape_records_report_facts_delta`

Smoke with isolated socket and rejected-only branch evidence:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
sockdir=$(mktemp -d /tmp/ploke-walk-r12.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r10
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" delta --verbose --no-color
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: R10 reached R11a with explicit `--watch`, then default `walk step` advanced `r11_to_r12` to `r12 - report facts ready`. The delta showed rejected children becoming report children, continuation becoming `Maybe<SuccessorDecision>`, and `Report<report::None>` becoming `Report<report::Facts>`. No final report was emitted.

GitNexus impact checks before edits were LOW for `R12`, `r11_to_r12`, `WalkPhase`, `WalkController`, and `TRANSITION_GRAPH_VERSION` (0 indexed direct callers/processes reported for each checked target). Pre-commit `npx gitnexus detect-changes --repo ploke` reported LOW risk, 9 changed files, 31 changed symbols, and 0 affected processes.

### Slice 11 — admit no-selection R13a stopped continuation in `walk`

Admitted only the no-selection stopped continuation path after R12:

- converted `R13aStopped` to `runtime_alias!` and exported `R13A_SHAPE`;
- added a narrow non-consuming `Collected::has_successor_selection()` accessor and `R12::has_successor_selection()` wrapper;
- added `WalkState::R13a` and `WalkPhase::R13a`;
- wired canonical `r12_to_r13` into `WalkController` only after checking that R12 has no selected-successor evidence;
- if selected-successor evidence is present, `walk` restores R12 and blocks with a hard message before the handoff-capable canonical edge is called;
- R13a is the current stop; R13b handoff and R14+ final report emission remain unavailable;
- bumped the walk transition graph version to `walk-r0-r13a-v1` so stale pre-R13a servers fail closed.

Deliberate limitations:

- selected-successor stopped decisions and R13b handoff are not admitted in this slice, because the existing canonical `r12_to_r13` can spawn successor handoff when selection allows it;
- no checkout install, History sealing, successor process spawn, or final report emission is admitted;
- R13a is not reconstructed directly from durable evidence yet. Restart recovery still reconstructs to R8 where child-plan evidence exists, then steps through R9/R10 and requires fresh in-memory R11/R12 before R13a.

Validation so far:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo build -p ploke-eval
```

Focused test additions:

- `walk::phase::tests::r12_advertises_stopped_continuation_only`
- `walk::phase::tests::r13a_shape_records_stopped_continuation_delta`

Smoke with isolated socket and rejected-only/no-selection path:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
sockdir=$(mktemp -d /tmp/ploke-walk-r13a.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r10
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" delta --verbose --no-color
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: R10 reached R11a with explicit `--watch`, R11a projected R12 report facts, and default R12 step advanced `r12_to_r13` to `r13a - stopped continuation ready`. The delta showed History head `FromStartup -> Read` and continuation decision `None -> Stopped<Prototype1ContinuationDecision>`. No successor handoff was started.

GitNexus impact checks before edits were LOW for `R13aStopped`, `R12ContinuationBranch`, `r12_to_r13`, `WalkPhase`, `WalkController`, `TRANSITION_GRAPH_VERSION`, and the added `Collected` accessor targets (0 indexed direct callers/processes reported for each checked target). Pre-commit `npx gitnexus detect-changes --repo ploke` reported LOW risk, 10 changed files, 29 changed symbols, and 0 affected processes.

### Slice 12 — admit stopped/no-selection R14a final report in `walk`

Admitted the stopped final-report edge after no-selection R13a:

- converted `R14aFinalStopped` to `runtime_alias!` and exported `R14A_SHAPE`;
- added `WalkState::R14a` and `WalkPhase::R14a`;
- wired canonical `r13_to_r14` into `WalkController` only by wrapping the in-memory R13a state as `R12ContinuationBranch::Stopped`;
- R14b handoff-final remains unreachable because selected-successor handoff is blocked earlier at R12 and the controller never constructs the handoff branch;
- R14a is the current stop;
- bumped the walk transition graph version to `walk-r0-r14a-v1` so stale pre-R14a servers fail closed.

Deliberate limitations:

- no R13b successor handoff or R14b handoff-final report admission yet;
- no checkout install, History sealing, successor process spawn, or server transfer is admitted;
- R14a is not reconstructed directly from durable evidence yet. Restart recovery still reconstructs to R8 where child-plan evidence exists, then steps through R9/R10 and requires fresh in-memory R11/R12/R13a before R14a.

Validation so far:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval shape --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo build -p ploke-eval
```

Focused test additions:

- `walk::phase::tests::r13a_advertises_r14a_final_report`
- `walk::phase::tests::r14a_shape_records_final_report_delta`

Smoke with isolated socket and rejected-only/no-selection path:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g31pro-p25flash-20260604-191249
sockdir=$(mktemp -d /tmp/ploke-walk-r14a.XXXXXX)
chmod 700 "$sockdir"
sock="$sockdir/walk.sock"

./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r10
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock"
./target/debug/ploke-eval loop walk show --repo-root "$ROOT" --socket "$sock" delta --verbose --no-color
./target/debug/ploke-eval loop walk stop --repo-root "$ROOT" --socket "$sock"
rm -rf "$sockdir"
```

Result: R10 reached R11a with explicit `--watch`, R11a projected R12 report facts, R12 reached no-selection R13a, and default R13a step advanced `r13_to_r14` to `r14a - final stopped report emitted`. The delta showed History head `Read -> Unchanged`, completion evidence `None -> Recorded`, and `Report<report::Facts> -> Report<report::Emitted<Prototype1StateReport>>`. No successor handoff or R14b path was started.

GitNexus impact checks before edits were LOW for `R14aFinalStopped`, `R14FinalBranch`, `r13_to_r14`, `emit_final_report_from_parts`, `WalkPhase`, `WalkController`, and `TRANSITION_GRAPH_VERSION` (0 indexed process impacts; `emit_final_report_from_parts` had one direct caller, `r13_to_r14`). Final pre-commit validation also ran `git diff --check` and `npx gitnexus detect-changes --repo ploke`; detect reported LOW risk, 9 changed files, 27 changed symbols, and 0 affected processes.

### Slice 13 — recover missing parent comparison from terminal child channel

Implemented the first channel-authority recovery slice for terminal child results:

- `run_planned_child` now gives stored terminal-child reconstruction enough parent baseline and branch-log context to repair a missing parent comparison report;
- `stored_child_outcome` keeps the existing hard block for nonterminal node projections with terminal runner results;
- succeeded terminal children with no branch evaluation report may recover only from a child invocation whose channel carries a validated terminal `ToParent::Result`;
- recovery validates channel envelope/body hash through `Channel::recv_from_child`, checks child invocation/node identity, requires matching latest and attempt-scoped runner-result files, and requires inline treatment evidence for successful children;
- the parent then replays the canonical `ObserveChild` transition and runs `compare_observed_child_treatment`, so selection input is derived only from the channel-sourced treatment evidence and the parent-owned comparison projection;
- no child C1-C3 work, provider calls, or fake selection evidence are started by this recovery path.

Deliberate limitations:

- recovery repairs parent observe/comparison projections for the current stored terminal child path only; it does not yet reconstruct R11/R12 directly from durable evidence after server restart;
- path-only `ResultWritten` remains diagnostic evidence and is not selectable without the terminal `Result` payload;
- selected-successor handoff remains blocked by the current walk slice.

Focused test additions:

- `succeeded_child_without_evaluation_recovers_from_terminal_channel`

Validation so far:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval succeeded_child_without_evaluation --all-targets -- --nocapture
cargo test -p ploke-eval loop_walk_ --all-targets
cargo test -p ploke-eval typestate --all-targets
cargo build -p ploke-eval
```

Historical smoke with isolated `0700` socket dirs:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-2g1x3-20260525-073410
./target/debug/ploke-eval loop walk start --repo-root "$ROOT" --socket "$sock" --until r7 --with-version
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch --with-version
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r10 --with-version
./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --watch --with-version
```

Result: the previously blocked fanout advanced `r10_to_r11 --watch -> r11 - child fanout complete`. Recovery wrote the missing parent comparison report at `prototype1/evaluations/branch-ac56d600dd90b3ab.json`; the recovered branch disposition was `reject` over one compared instance. A follow-up isolated smoke stepped the same campaign through `r11_to_r12` and `r12_to_r13a`, preserving the current no-handoff guard.

GitNexus impact checks before edits were LOW for `stored_child_outcome`, `run_planned_child`, `ObserveChild`, and `compare_observed_child_treatment` (0 indexed process impacts reported for each checked target).

### Slice 14 — reconstruct R11/R12 from terminal child channel evidence

Implemented direct durable reconstruction past child fanout without rerunning children:

- added `reconstruct_child_outcomes_from_store(...)` for read-only reconstruction of `PlannedChildOutcome` values from stored child records;
- each reconstructed terminal child now requires a stored terminal node, matching latest `runner-result.json`, a matching attempt-scoped result file, a valid child invocation authority, and a validated terminal child `Channel::recv_from_child` `ToParent::Result` payload;
- successful children additionally require a branch evaluation report whose baseline campaign, branch id, and treatment campaign agree with the parent campaign, node record, and runner result;
- `driver::reconstruct_early` now replays pure `R8 -> R9 -> R10`, reconstructs R11 from channel-derived outcomes or R11a from rejected attempts, then calls canonical `r11_to_r12` to rebuild report facts;
- reconstruction stops at R10 with an explicit blocker when durable child terminal/comparison evidence is incomplete rather than spawning children or fabricating outcomes;
- `walk start --until ...` now prefers durable reconstruction for existing matching parent checkouts instead of replaying live setup edges, avoiding duplicate parent-start/resource journal writes. Plain `walk start`/R0 and parent identity initialization remain live command paths.

Deliberate limitations:

- R13b/R14b selected-successor handoff is still blocked; reconstructed selected-successor R12 stops before R13a;
- no provider, harness fanout, child C1-C3, or live child planning work is started by this reconstruction path;
- parent-owned `runner-result.json` and evaluation files remain projections/caches unless tied back to the admitted child channel terminal result and matching report identity.

Focused test update:

- `succeeded_child_without_evaluation_recovers_from_terminal_channel` now also asserts read-only `reconstruct_child_outcomes_from_store(...)` after terminal-channel recovery writes the parent comparison report.

Validation so far:

```text
cargo fmt --all
cargo check -p ploke-eval --all-targets
cargo test -p ploke-eval succeeded_child_without_evaluation_recovers_from_terminal_channel --all-targets -- --nocapture
cargo build -p ploke-eval
```

Historical smokes with isolated `0700` socket dirs:

```text
ROOT=/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-2g1x3-20260525-073410
CID=p1-gemini35-flash-direct-2g1x3-20260525-073410

./target/debug/ploke-eval loop walk step --repo-root "$ROOT" --socket "$sock" --until r12 --format json
./target/debug/ploke-eval loop walk start --repo-root "$ROOT" --socket "$sock" --until r12 --format json
./target/debug/ploke-eval loop walk start --repo-root "$ROOT" --campaign "$CID" --socket "$sock" --until r12 --format json
```

Results:

- all three commands reconstructed directly to `r12 - report facts ready` with `steps: 0` and notes through `reconstructed R11 from 2 channel-derived child outcomes`;
- transition journal line count stayed unchanged at 39 for the read-only smokes;
- `walk step --until r13a` from the reconstructed selected-successor R12 still failed with the intended guard: `walk reached R12 with selected-successor evidence; R13b handoff is not admitted by this debug server slice`.

Incident/recovery note:

- An initial smoke used `walk start --until r12` before the start-command guard was fixed; that path replayed live setup edges and appended two duplicate parent-start/resource journal entries to the historical campaign. Those two accidental tail entries were identified by timestamp/PID and removed immediately, restoring the journal line count from 41 to 39 before the read-only smokes above.

GitNexus impact checks before edits were LOW for `reconstruct_early`, the disambiguated `WalkState`, `stored_child_outcome`, `ParentSelection`, and `WalkController.start` (exact indexed UID `Function:crates/ploke-eval/src/cli/prototype1_state/walk/controller.rs:WalkController.start#2`).
