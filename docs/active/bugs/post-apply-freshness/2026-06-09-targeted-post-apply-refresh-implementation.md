# 2026-06-09 Targeted Post-Apply Refresh Implementation

## Status

Implemented in the current working tree, pending review and commit.

This is a follow-up record for
[`2026-06-09-prototype1-post-apply-refresh-selects-wrong-crate.md`](./2026-06-09-prototype1-post-apply-refresh-selects-wrong-crate.md).
It records what was found during implementation and what changed. It does not
replace or rewrite the earlier live-run bug report.

## Broken Contract

After an accepted file mutation, post-apply refresh must scan the loaded crate
that owns the mutated path. It must not use the primary or focused crate as an
implicit fallback for path-scoped edit refresh.

## Evidence

Implementation confirmed two unscoped refresh paths:

- `crates/ploke-tui/src/rag/editing.rs`: semantic and non-semantic approval
  paths already had the proposal `file_paths`, but `rescan_for_changes` called
  the generic `scan_for_change`.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`:
  the broad headless harness `wait_for_refresh` also sent generic
  `StateCommand::ScanForChange`.

The database layer already had the lower-level owner-specific primitive:

- `crates/ploke-tui/src/app_state/database.rs`: `scan_for_change_target` accepts
  a `LoadedCrateScanTarget`.

The missing boundary was resolving touched paths to loaded crate targets before
running the refresh barrier.

## Source Trace

The implementation adds a path-scoped scan command and routes post-apply edit
refresh through it:

- `crates/ploke-tui/src/app_state/commands.rs`: added
  `StateCommand::ScanPathsForChange { paths, scan_tx }`.
- `crates/ploke-tui/src/app_state/dispatcher.rs` and
  `crates/ploke-tui/src/app_state/handlers/db.rs`: dispatch and handler wrapper
  for the targeted command.
- `crates/ploke-tui/src/app_state/database.rs`: added target resolution from
  absolute changed paths to the loaded crate with the longest matching root
  prefix, then scans those owner crates.
- `crates/ploke-tui/src/rag/editing.rs`: semantic, non-semantic, and create
  approval paths now pass applied paths into targeted post-apply refresh.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/tui_bridge.rs`:
  broad headless harness refresh now requires changed paths and sends
  `ScanPathsForChange`.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter/harness/tui.rs`:
  lower-level harness settle path passes accumulated changed paths into the
  refresh barrier.

The generic `ScanForChange` path remains for explicit current-focus/manual scan
behavior. Path-scoped post-apply work no longer relies on that fallback.

## Current Repro Coverage

Added production TUI coverage:

```text
cargo test -p ploke-tui targeted_scan_refreshes_changed_member_independent_of_focus -- --nocapture
```

This regression indexes the checked-in multi-crate workspace fixture, focuses
the runtime on one member, mutates a file in another member, runs the targeted
scan with the changed absolute path, and asserts the database retracts the old
function row and indexes the renamed function in the touched member.

The test asserts database state and returned changed paths. It does not use
string-based trace output as evidence.

Focused validation run in this working tree:

```text
cargo test -p ploke-tui targeted_scan_refreshes_changed_member_independent_of_focus -- --nocapture
cargo test -p ploke-tui approval_refreshes_file_hash_before_returning -- --nocapture
cargo test -p ploke-tui approve_emits_rescan_sysinfo -- --nocapture
cargo test -p ploke-eval sparse_post_apply_refresh_returns_on_bm25_without_dense_index_completion -- --nocapture
cargo check -p ploke-eval
```

All listed commands passed. `cargo check -p ploke-eval` still emits existing
warning-class output unrelated to the targeted refresh change.

## Missing Validation

The new TUI regression proves owner-crate selection for a checked-in workspace
fixture. Remaining validation before closing the broader bug cluster:

- rerun a broad headless TUI replay/live attempt that mutates a non-focused
  member crate and verify later semantic lookup/edit no longer sees stale graph
  state;
- decide whether create-file refresh should also grow explicit new-file
  indexing coverage, because the current scan primitive starts from files
  already known in the database.

## Fix Direction

Keep the current direction:

- strict `read_full_verified` stale-anchor rejection remains correct;
- path-scoped post-apply refresh should fail loudly when no loaded crate owns a
  touched path;
- generic focused-crate scan should remain separate from edit lifecycle refresh.

## Related Records

- [`2026-06-09-prototype1-post-apply-refresh-selects-wrong-crate.md`](./2026-06-09-prototype1-post-apply-refresh-selects-wrong-crate.md)
- [`../2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md`](../2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md)
- [`../2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](../2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
