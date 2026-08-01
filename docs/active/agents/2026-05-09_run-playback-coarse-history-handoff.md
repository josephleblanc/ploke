# 2026-05-09 Run Playback Coarse History Handoff

Restart handoff for the `prototype1-run-playback-coarse-history-spine`
thread. This thread follows
[`2026-05-09_run-playback-typed-observability-plan.md`](2026-05-09_run-playback-typed-observability-plan.md).

## Current Focus

Build the first typed, iterable Prototype 1 playback slice:

- passive playback vocabulary in `ploke-records`;
- coarse sealed-History spine projection in `ploke-tree`;
- bounded synthetic validation first;
- compact real-run validation against the last live run;
- no scheduler/projection records as ordering authority.

Task-stack focus item added:

- `prototype1-run-playback-coarse-history-spine`

## User Direction

The main thread should act as orchestrator and preserve context:

- do not directly run broad `rg`, `jq`, or raw record inspection in the main
  thread;
- use `gpt-5.4-mini` or similarly cheap sub-agents for bounded discovery and
  schema triage;
- main thread may read only exact small spans reported by agents or stop and
  report the blocker;
- do not dump raw live-run JSON/log payloads;
- if sub-agents fail to isolate a hard mismatch, widen the delegated question
  or report the blocker instead of doing broad local probing.

The last turn violated this policy: after a real-run parse failure, the main
thread started direct `jq`/`rg`/`sed` probing. On restart, treat that as a
process bug and return to orchestrator mode.

## Implemented So Far

### `ploke-records`

Files:

- `crates/ploke-records/src/playback.rs`
- `crates/ploke-records/src/lib.rs`

Added passive playback vocabulary:

- `EvidenceStrength`
  - `Projection`
  - `TypedRecord`
  - `AdmittedHistory`
  - `SealedHistory`
- `PlaybackGranularity`
- `Coarse` typestate
- `CoarseStep`
- `CoarseStepRef<'a>`
- `RunPlayback<G = Coarse>`
- `RunPlaybackRef<'a, G = Coarse>`
- `iter()` and `IntoIterator` support for owned and borrowed playback forms

Verification reported and later repeated successfully:

```bash
cargo check -p ploke-records --lib 2>&1 | tail -n 80
cargo test -p ploke-records playback 2>&1 | tail -n 80
```

### `ploke-tree`

Files:

- `crates/ploke-tree/src/playback.rs`
- `crates/ploke-tree/src/lib.rs`

Added coarse sealed-History projection module:

- `CoarseHistoryStep`
- `CoarseHistorySpine`
- `CoarseHistoryWarning`
  - `ParentHashLinkMismatch`
  - `MissingSelectionDecisionPayload`
- `build_coarse_history_spine(&[SealedBlockRecord]) -> CoarseHistorySpine`
- `project_coarse_history_spine(&[SealedBlockRecord]) -> Vec<CoarseHistoryStep>`
- `coarse_run_playback_from_sealed_history(&[SealedBlockRecord]) -> RunPlayback<Coarse>`

The projection:

- orders by sealed History `block_height`, then `block_hash`;
- does not use scheduler/node/request/result records for ordering;
- extracts selected candidate and considered-candidate count from typed
  `SelectionDecision` payloads when present;
- emits warnings without reordering playback.

Synthetic test:

- `coarse_history_spine_orders_by_height_preserves_selection_and_emits_warnings`

Verification passed:

```bash
cargo check -p ploke-tree --lib 2>&1 | tail -n 80
cargo test -p ploke-tree coarse_history 2>&1 | tail -n 80
```

Extraction note:

- Playback projection was first added directly to `ploke-tree/src/lib.rs`.
- This tripped the plan's file-growth/module-pressure rule.
- It was then extracted to `crates/ploke-tree/src/playback.rs`.

## Real-Run Validation State

Added a real-run ignored test in `ploke-tree`:

- `coarse_playback_real_run`

It uses:

```bash
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1 \
cargo test -p ploke-tree coarse_playback_real_run -- --ignored --nocapture 2>&1 | tail -n 80
```

Observed failure:

```text
load sealed history blocks: Json {
  path: "/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1/history/blocks/segment-000000.jsonl",
  source: Error("invalid type: map, expected a string", line: 1, column: 253304)
}
```

Interpretation:

- The synthetic playback model is working.
- The real-run stop condition is not met.
- The passive `ploke_records::history::SealedBlockRecord` mirror does not yet
  deserialize the known live run's sealed History record shape.
- This is now the blocking issue before claiming live-run compatibility.

## Failed Triage Attempts

Sub-agent triage attempts:

1. First schema-mismatch triage agent hung and was closed.
2. Second agent guessed the mismatch was around
   `state.header.common.opening_authority`, but did not prove an exact field.
3. Third agent found the serde error occurs before the top-level `state` key,
   likely inside `entries[0]`, but still did not isolate an exact field.

The main thread then began direct probing and should not continue doing that
after restart.

Bounded fact worth preserving:

- The parse error column appears before top-level `state`, so the mismatch is
  likely in `entries[0]`, not in `state.header.common`.

Do not treat the guessed `opening_authority` path as proven.

## Next Task

Recover the exact schema mismatch without broad main-thread exploration.

Recommended next move:

1. Delegate a bounded worker or explorer to add a path-aware diagnostic test or
   one-off diagnostic helper using `serde_path_to_error` or an equivalent local
   path-aware deserialization approach.
2. The diagnostic should target only the first line of the known fixture file
   and print only:
   - serde path;
   - expected/actual shape summary if available;
   - Rust DTO field/type candidate.
3. Patch only the exact passive DTO field once the path is known.
4. Re-run:

```bash
cargo test -p ploke-records sealed_block_record_roundtrips_as_passive_data 2>&1 | tail -n 80
cargo test -p ploke-tree coarse_history 2>&1 | tail -n 80
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1 cargo test -p ploke-tree coarse_playback_real_run -- --ignored --nocapture 2>&1 | tail -n 80
```

If adding `serde_path_to_error` as a dev-dependency is needed, keep it scoped to
diagnostics/tests if possible. Do not introduce broad compatibility wrappers or
opaque JSON fields in public records.

## Stop Conditions For Current Slice

These are now met for the coarse sealed-History playback spine:

- `RunPlaybackRef`/`RunPlayback` vocabulary compiles;
- synthetic coarse History test passes;
- real live run loads as typed `SealedBlockRecord`;
- `coarse_playback_real_run` confirms the known 12-block fixture shape;
- warnings are visible and do not reorder playback;
- no scheduler records are used for ordering.

## 2026-05-09 Progress After Restart

Real-run schema blocker:

- Exact serde path was `entries[*].core.payload.traversal.strategy`.
- Root cause: writer-side `TraversalEvidence.strategy` serializes as tagged
  `StrategyKind`, while passive `ploke-records` expected `String`.
- Fixed in `crates/ploke-records/src/history/payload.rs` with passive
  `TraversalMetricInputsRecord` and `TraversalStrategyRecord`.
- Added `serde_path_to_error` as a `ploke-records` dev-dependency.

Coarse playback validation:

- `coarse_playback_real_run` now passes against the known live fixture:
  `blocks=12`, `steps=12`, `warnings=0`.
- The prior test assertion requiring nonempty warnings was corrected. This
  fixture is clean for currently implemented warning classes.
- Synthetic warning coverage remains in `coarse_history_spine_orders_by_height_preserves_selection_and_emits_warnings`.

Borrowed path:

- `ploke-tree` now exposes
  `coarse_run_playback_ref_steps_from_sealed_history`.
- Owned and borrowed coarse playback step IDs both use sealed `block_hash`.

Passive History fidelity:

- `ploke-eval` test
  `fs_block_store_history_segment_deserializes_as_passive_record` now writes a
  real selection decision with traversal evidence, deserializes it through
  `ploke-records`, and asserts nested selection/traversal fields.

Debug projection:

- Added `ploke-eval history playback --format table|json`.
- CLI dispatch is thin; semantics come from `ploke-tree` sealed History
  projection, not scheduler/projection records.
- New module: `crates/ploke-eval/src/cli/prototype1_state/history_playback.rs`.

Front-facing shell:

- Added new renderer-neutral crate `crates/ploke-tree-browser`.
- It depends on `ploke-tree` and `ploke-records`, not `ploke-eval`.
- It exposes `PlaybackBrowserModel` and
  `coarse_history_browser_model(_from_blocks)` as the first UI/WebAssembly
  boundary without choosing egui/eframe/wasm tooling yet.

Fine playback:

- User chose `Fine` as the next granularity.
- Added `Fine`, `FineStepKind`, `FineOrder`, `FineStep`, and `FineStepRef` in
  `crates/ploke-records/src/playback.rs`.
- Split `crates/ploke-tree/src/playback.rs` into:
  - `crates/ploke-tree/src/playback/mod.rs`
  - `crates/ploke-tree/src/playback/coarse.rs`
  - `crates/ploke-tree/src/playback/fine.rs`
- Added `fine_run_playback_from_sealed_history` and
  `fine_run_playback_ref_steps_from_sealed_history`.
- First Fine projection is sealed-History-derived only:
  `CandidateConsidered`, `SuccessorSelected`, `HistoryEntryAdmitted`, and
  `HistoryBlockSealed`.
- Scheduler-only `ChildPlanned` remains excluded from the authoritative fine
  stream. Journal/runtime phases are typed but still need a loader/index pass
  before they can be joined.
- Live fixture now reports:
  `fine_history steps=306 candidates=270 selected=12 admitted=12 sealed=12`.
- `ploke-eval history playback` now accepts `--granularity coarse|fine` and
  defaults to `fine`.
- `ploke-tree-browser` now exposes `fine_history_browser_model_from_blocks`.

Latest bounded checks:

```bash
cargo test -p ploke-records playback 2>&1 | tail -n 80
cargo test -p ploke-records sealed_block_record_roundtrips_as_passive_data 2>&1 | tail -n 80
cargo test -p ploke-records traversal_evidence 2>&1 | tail -n 80
cargo test -p ploke-eval fs_block_store_history_segment_deserializes_as_passive_record 2>&1 | tail -n 80
cargo test -p ploke-eval history_playback_command_parses 2>&1 | tail -n 80
cargo test -p ploke-eval table_render_handles_empty_spine 2>&1 | tail -n 80
cargo test -p ploke-eval table_render_handles_empty_fine_playback 2>&1 | tail -n 80
cargo test -p ploke-tree coarse_history 2>&1 | tail -n 80
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1 cargo test -p ploke-tree coarse_playback_real_run -- --ignored --nocapture 2>&1 | tail -n 80
cargo test -p ploke-tree-browser 2>&1 | tail -n 80
cargo check -p ploke-eval 2>&1 | tail -n 80
```

All passed.

## Dirty Files To Be Aware Of

Playback-thread files currently touched:

- `.codex/task-stack.jsonl`
- `AGENTS.md`
- `docs/active/agents/readme.md`
- `docs/active/agents/2026-05-09_run-playback-typed-observability-plan.md`
- `docs/active/agents/2026-05-09_run-playback-coarse-history-handoff.md`
- `Cargo.lock`
- `Cargo.toml`
- `crates/ploke-eval/Cargo.toml`
- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_playback.rs`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-records/src/lib.rs`
- `crates/ploke-records/src/history/payload.rs`
- `crates/ploke-records/src/playback.rs`
- `crates/ploke-tree/src/lib.rs`
- `crates/ploke-tree/src/playback.rs`
- `crates/ploke-tree/src/playback/mod.rs`
- `crates/ploke-tree/src/playback/coarse.rs`
- `crates/ploke-tree/src/playback/fine.rs`
- `crates/ploke-tree-browser/Cargo.toml`
- `crates/ploke-tree-browser/src/lib.rs`

Other dirty files existed from adjacent threads and should not be casually
rewritten or reverted.
