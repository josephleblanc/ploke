# 2026-05-07 Prototype 1 Edit Surface Implementation Handoff

## Task

Carry forward the bounded `ploke-tui` edit-surface integration for Prototype 1 so a loop run can generate candidates over the `ploke-tui` tool area, validate them through an explicit surface boundary, persist selection evidence into History, and run with the newer History-backed selector.

Related files:

- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-model.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-handoff.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/archive/agents/2026-05/2026-05-07-tui-edit-surface-producer-review.md`

## Current State

Committed baseline:

- `d5a6b0d2` planned the bounded edit-surface implementation.
- `87d2eea3` added the Prototype 1 edit-surface boundary.
- `be430169` scaffolded the bounded TUI edit-surface adapter.
- `e5357423` sealed bounded edit adapter apply evidence.
- `18e1bee5` added fail-closed edit-surface generator knobs.
- `fccd6cf5` added the single-file edit-surface backend bridge.
- `4f23a214` extracted the stage-free TUI edit resolver.
- `7652f51a` added the deterministic edit-surface producer.

Dirty workspace at handoff:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`

Those dirty files are the Phase 4 patch from subagent `019e051a-e878-74f0-9f48-626cf1e75020`, which reported completion. The patch persists checked edit-surface evidence into the sealed candidate artifact/payload path and exposes evidence presence in `selection-show`.

## What Went Right

- The edit boundary is no longer raw patch text or UI proposal storage. It flows through explicit surface/grant/check/apply carriers.
- The backend owns after-Artifact validation for the current single-file bridge: path containment, stale base, proposed hash, patch id, and surface bounds are checked before child creation.
- The TUI resolver extraction avoids staging UI state as authority. The existing `apply_code_edit_tool` path still stages proposals, but eval-side code can resolve writes without using UI/chat/proposal side effects.
- The deterministic producer gives us a controlled short-run path over the `ploke-tui` tool surface while the richer LLM/code-graph producer is still being designed.
- Phase 4 reportedly adds History-carried edit-surface evidence for future replay/scoring instead of dropping the grant/check/delta facts when converting to `ChildFiles` and `CandidateArtifact`.

## What Went Wrong Or Remains Risky

- The live producer is deterministic direct-splice over `crates/ploke-tui/src/tools/code_edit.rs`; it is not yet the final semantic edit engine using the code graph and TUI resolver as the proposal source.
- The Phase 4 evidence patch is uncommitted and should be reviewed before landing. It reportedly passes `cargo fmt`, `cargo check -p ploke-eval`, `prototype1_state`, and `successor_selection`, but the main thread has not independently checked the diff yet.
- Surface evidence is persisted inside sealed candidate artifact/payload evidence, not as its own first-class History transition. That matches the current implementation slice, but deeper grant/check/delta History admission is still future work.
- Runtime child budget is still default-policy driven in the active parent path. This is probably not a blocker for the next short run, but it is still ambient policy and should become explicit runtime configuration later.
- The deterministic candidate edits are intended to be compile-safe comments. A live parent worktree build/run is still the real validation gate.

## Recommended Next Steps

1. Inspect the Phase 4 dirty diff narrowly, starting with the new `SurfaceEvidence` carrier in `history.rs`, the optional `ChildFiles` field in `parent.rs`, the `CheckedSurfaceEdit` projection in `cli_facing.rs`, and the `selection-show` preview in `history_preview.rs`.
2. Run bounded verification:
   - `cargo fmt --all`
   - `cargo check -p ploke-eval 2>&1 | tail -n 80`
   - `cargo test -p ploke-eval prototype1_state --lib 2>&1 | tail -n 20`
   - `cargo test -p ploke-eval successor_selection --lib 2>&1 | tail -n 20`
3. Start an independent reviewer subagent for the Phase 4 patch before committing. The review should check History replay adequacy, serde compatibility, whether surface evidence is tied to the selected Artifact rather than a projection, and whether naming/visibility preserves structure.
4. Commit Phase 4 only after review and checks pass.
5. Use the `prototype1-loop-runtime` workflow to set up/build a short live run from a new parent worktree. The user should run the actual live loop command from the active parent binary.
6. For the short run, use the new path:
   - `--candidate-generator tui-edit-surface`
   - `--edit-surface ploke-tui-tools`
   - `--successor-selection history-score-child-prop`
   - `--successor-selection-metrics operational-and-protocol`
7. Observe the run with campaign-tail/preview helpers, not full tracing walls. Confirm child plan creation, child fanout, sealed candidate evidence, History-backed selection, successor handoff, and surface evidence visibility.
8. After the short run succeeds, replace the deterministic direct-splice producer with the real bounded semantic edit proposal path over code graph view plus TUI edit harness.

## Do Not Lose

The core invariant is still:

- child candidates may observe bounded surfaces;
- only the parent may create writable candidate Artifacts;
- proposed Artifact deltas must be checked against the bounded surface before they become candidates;
- History must carry enough sealed evidence for later selection/scoring without re-reading mutable projection files.

