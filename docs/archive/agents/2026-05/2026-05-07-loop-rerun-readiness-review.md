# Prototype 1 Loop Rerun Readiness Review

Reviewer: reviewer 1
Date: 2026-05-07
Scope: current unstaged successor-selection and handoff path in `/home/brasides/code/ploke`

## 1. Verdict

Conditional for a short loop rerun.

The selector semantics are materially cleaner than the failed overnight path: active `prototype1-state` now defaults to History traversal with current-generation candidates appended, and active handoff is gated through sealed selection material plus Artifact hydration before spawn. The code compiles and the targeted selector/current-generation/handoff tests pass.

It is not safe to rerun from the current dirty checkout as-is. The successor handoff install path explicitly refuses a dirty active checkout before switching the active parent worktree to the selected Artifact. This worktree currently has modified tracked files and an untracked `crates/ploke-eval/src/successor_selection/operator_projection.rs`, so the first successful successor selection is likely to fail at active-checkout install unless the active parent worktree is made clean with all required files tracked/committed.

## 2. Blockers

1. Clean active parent worktree required before live rerun.

   Evidence: `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1051` documents that install moves the stable parent checkout to the selected durable branch, and `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1061` to `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1067` returns `DirtyActiveCheckout` when any dirty path exists. The current status includes modified selector files and untracked `crates/ploke-eval/src/successor_selection/operator_projection.rs`.

2. Track/include `operator_projection.rs` before any git-boundary rerun.

   Evidence: `crates/ploke-eval/src/successor_selection/mod.rs:26` declares `pub mod operator_projection;`, and `crates/ploke-eval/src/successor_selection/operator_projection.rs:17` defines `generation_summary`. The file is untracked in the current worktree. The current checkout compiles because the file exists locally, but a committed parent branch, child worktree, or successor branch that lacks it will not compile.

## 3. Risks

1. Dirty-checkout handoff failure remains the highest-probability runtime failure if rerun immediately.

   The active path builds the successor only after installing the selected Artifact into the active checkout: `crates/ploke-eval/src/cli/prototype1_process.rs:475` to `crates/ploke-eval/src/cli/prototype1_process.rs:483`. Install then calls `install_artifact_in_active_checkout`: `crates/ploke-eval/src/cli/prototype1_process.rs:613` to `crates/ploke-eval/src/cli/prototype1_process.rs:615`. That backend refuses dirty state before `git switch`, so current unstaged work blocks handoff.

2. Default selector is coherent but no longer means "first accepted child wins".

   The CLI default is now `HistoryScoreChildProp`: `crates/ploke-eval/src/cli.rs:476` to `crates/ploke-eval/src/cli.rs:478`. Active mapping sends that to all admitted History plus current-generation candidates with `score_child_prop`: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5410` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5413`. Traversal uses weighted sampling over selectable candidates: `crates/ploke-eval/src/successor_selection/traversal.rs:842` to `crates/ploke-eval/src/successor_selection/traversal.rs:858`. With fixed seed this is replayable, but it can select a historical coordinate or an exploration coordinate, not necessarily the first local keep.

3. Rejected-branch exploration can still continue by design.

   `selectable_decision` converts rejected Stop decisions into `ExploreFrom` candidates: `crates/ploke-eval/src/successor_selection/traversal.rs:649` to `crates/ploke-eval/src/successor_selection/traversal.rs:662`. Handoff disposition records that as `ContinueExploreFromRejected`: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5361` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5373`. This is not selector confusion, but it is a semantic choice operators should expect.

4. Historical candidates are correctly guarded, but missing Artifact/runtime evidence will stop handoff.

   Handoff requires selected payload Artifact material: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:378` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:386`. It then checks node, branch, sealed coordinate, and historical runtime identity before hydrating the selected Artifact: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5758` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5855`. This is correct fail-closed behavior; it can still stop a run if admitted historical records are incomplete.

5. Runtime fanout currently uses the default child budget, not a loaded scheduler policy.

   The live parent path uses `Prototype1SearchPolicy::default().child_budget` when `stop_after == Complete`: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6292` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6295`. The default is `min: 2, max: 6`: `crates/ploke-eval/src/intervention/scheduler.rs:51` to `crates/ploke-eval/src/intervention/scheduler.rs:54`. If setup was configured with different fanout expectations, runtime behavior may be surprising, although this is less likely to be the handoff failure mode.

## 4. Evidence

- CLI default: `crates/ploke-eval/src/cli.rs:427` to `crates/ploke-eval/src/cli.rs:433` keeps `GenerationLocal`, `HistoryFrontierMax`, and `HistoryScoreChildProp`; `crates/ploke-eval/src/cli.rs:476` to `crates/ploke-eval/src/cli.rs:478` makes `HistoryScoreChildProp` the default.
- Active strategy mapping: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5399` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5415` maps `GenerationLocal` to current-generation traversal and the two History strategies to all admitted History.
- Current-generation projection: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5621` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5680` builds sealed payloads from typed child outcomes and records missing selection input as projection failure.
- Active selection call: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6311` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6318` routes complete parent turns through `ParentSelection::select_successor`.
- Current candidates appended to History: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5688` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5709`.
- Traversal eligibility: `crates/ploke-eval/src/successor_selection/traversal.rs:581` to `crates/ploke-eval/src/successor_selection/traversal.rs:646` excludes candidates missing selection input, candidate-set proof, valid binding, or decision-grade identity.
- Traversal selected-source tracking: `crates/ploke-eval/src/successor_selection/traversal.rs:280` to `crates/ploke-eval/src/successor_selection/traversal.rs:286`.
- Handoff gating: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6355` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6362` only creates handoff material when the decision has a selected branch.
- Spawn path: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6395` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6413` calls `spawn_and_handoff_prototype1_successor` and reports acknowledged/timed-out.
- Successor install/build/spawn: `crates/ploke-eval/src/cli/prototype1_process.rs:475` to `crates/ploke-eval/src/cli/prototype1_process.rs:483`, `crates/ploke-eval/src/cli/prototype1_process.rs:613` to `crates/ploke-eval/src/cli/prototype1_process.rs:615`, and `crates/ploke-eval/src/cli/prototype1_process.rs:1265` to `crates/ploke-eval/src/cli/prototype1_process.rs:1282`.
- Successor acknowledgement validates campaign, node, active parent root, and History continuation: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5946` to `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6020`.

## 5. Recommended Sanity Commands

Run before a live rerun, from the intended active parent worktree:

```bash
git status --short --untracked-files=all
git ls-files --error-unmatch crates/ploke-eval/src/successor_selection/operator_projection.rs 2>&1 | tail -n 5
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo test -p ploke-eval successor_selection --lib -- --nocapture 2>&1 | tail -n 60
cargo test -p ploke-eval current_generation --lib -- --nocapture 2>&1 | tail -n 60
cargo test -p ploke-eval history_handoff --lib -- --nocapture 2>&1 | tail -n 60
cargo build -p ploke-eval --bin ploke-eval 2>&1 | tail -n 80
```

Checks run during this review:

- `cargo check -p ploke-eval 2>&1 | tail -n 80`: passed with warnings.
- `cargo test -p ploke-eval successor_selection --lib -- --nocapture 2>&1 | tail -n 60`: passed, 20 tests.
- `cargo test -p ploke-eval history_handoff --lib -- --nocapture 2>&1 | tail -n 60`: passed, 3 tests.
- `cargo test -p ploke-eval current_generation --lib -- --nocapture 2>&1 | tail -n 60`: passed, 2 tests.

Do not treat these as live-loop proof. They validate compile-time fallout and selector/handoff guard behavior only.

## 6. Recommendation

Do not start another long run yet.

A short 2-3 generation rerun is safe enough only after the active parent worktree is clean and includes the new `operator_projection.rs` at the git boundary used by the runtime. Use the active parent worktree's own freshly built `./target/debug/ploke-eval` for `prototype1-state`, not a binary from another checkout.

After that, a short rerun is reasonable before any overnight run. Watch for the first successful successor selection crossing all the way through active checkout install, successor build, spawn, and ready acknowledgement. If it fails, the most likely remaining issues are dirty-checkout install refusal, missing tracked selector module in the successor checkout, incomplete historical Artifact/runtime evidence, or an intentional `ExploreFrom` selection being mistaken for a keep-only continuation.
