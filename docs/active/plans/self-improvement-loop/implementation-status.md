# Implementation Status

Updated: 2026-05-10

## Current Read

The core records/playback path appears implemented through typed records, sealed-History playback projections, browser-model enrichment, CLI export/playback, and an egui shell.

The current open proof is not a known `ploke-records` or `ploke-tree` schema patch. It is browser bundling/serve verification, plus the need to keep live-status evaluation away from stale `scheduler.json` authority.

## Implemented Evidence

- `crates/ploke-records/src/playback.rs`
  Defines `RunPlayback`, `RunPlaybackRef`, `Coarse`, `Fine`, `EvidenceStrength`, iterator support, and fine step vocabulary.
- `crates/ploke-tree/src/playback/coarse.rs`
  Builds a sealed-History coarse spine from `SealedBlockRecord` values.
- `crates/ploke-tree/src/playback/fine.rs`
  Builds the first fine sealed-History stream: candidate considered, successor selected, History entry admitted, and History block sealed.
- `crates/ploke-tree/src/lib.rs`
  Loads sealed history blocks from `prototype1/history/blocks/segment-*.jsonl` and transition journal entries from `transition-journal.jsonl`.
- `crates/ploke-tree-browser/src/lib.rs`
  Exposes renderer-neutral `PlaybackBrowserModel`, run summaries, evidence-bearing steps, evaluation snapshots, surface snapshots, and protocol snapshots.
- `crates/ploke-eval/src/cli/prototype1_state/history_playback.rs`
  Projects `history playback --granularity coarse|fine --format table|json` from sealed History blocks.
- `crates/ploke-eval/src/cli/prototype1_state/browser_export.rs`
  Exports enriched browser models, but currently counts journal entries by reading the journal file as text.
- `crates/ploke-tree-egui/src/main.rs`
  Provides the native/WASM egui shell over the browser model.

## Open Gaps

- WASM/browser bundle and serve verification is not closed. A prior `trunk build` attempt reportedly failed with `invalid value '1' for '--no-color'`.
- Live-status and stop-reason logic still has a `scheduler.json` branch in `terminal_state`; this is not authoritative for this track.
- Fine playback is sealed-History-derived only. Runtime, journal, invocation, and benchmark phases still need a loader/index join before they become first-class playback steps.
- Benchmark improvement is represented in evaluation snapshots, but aggregate improvement trajectory and attribution are not yet first-class frontend concepts.
- `browser_export` should eventually count and summarize journal entries through typed loading rather than raw text counting.

## Focused Verification

Verified in this checkout on 2026-05-10:

```bash
cargo test -p ploke-records playback 2>&1 | tail -n 80
cargo test -p ploke-tree coarse_history 2>&1 | tail -n 80
cargo test -p ploke-tree-browser 2>&1 | tail -n 80
cargo check -p ploke-tree-egui --target wasm32-unknown-unknown 2>&1 | tail -n 80
```

All four passed.

Additional bounded checks to run before changing record/playback code:

```bash
cargo test -p ploke-records sealed_block_record_roundtrips_as_passive_data 2>&1 | tail -n 80
cargo test -p ploke-records traversal_evidence 2>&1 | tail -n 80
cargo test -p ploke-eval history_playback_command_parses 2>&1 | tail -n 80
cargo test -p ploke-eval fs_block_store_history_segment_deserializes_as_passive_record 2>&1 | tail -n 80
```

For real-run coarse playback:

```bash
PLOKE_TREE_RUN_ROOT=/home/brasides/.ploke-eval/campaigns/p1-edit-surface-history-long-20260508-1/prototype1 cargo test -p ploke-tree coarse_playback_real_run -- --ignored --nocapture 2>&1 | tail -n 80
```
