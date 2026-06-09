# 2026-06-09 Prototype 1 Post-Apply Refresh Selects Wrong Crate

## Summary

Prototype 1 can apply edits to one workspace member and then refresh a different
loaded crate before the next semantic lookup or patch attempt. In the live
ripgrep run that exposed this, the edited file was
`crates/ignore/src/dir.rs`, but the post-apply refresh path repeatedly scanned
the `globset` crate. The stale-anchor hash guard then correctly rejected later
semantic edits against `dir.rs` as `Content changed`.

This is the same contract family as the existing same-file stale-anchor reports,
but the concrete recurrence is wrong-crate refresh selection, not a need to
weaken `apply_code_edit` hash verification.

## Current Status

Open. This report groups the recurrence with the older post-apply freshness
bugs and adds syn_parser regression coverage for the target crate/item. The
production refresh path still needs to route post-apply scans from touched file
paths to their owning loaded crate roots.

Regression coverage added in this report:

```text
PLOKE_RIPGREP_IGNORE_CRATE=/path/to/ripgrep/crates/ignore \
  cargo test -p syn_parser --test ripgrep_ignore_method_lookup \
    ripgrep_ignore_crate_parses_and_locates_ignore_matched_ignore -- --nocapture
```

The test is environment-gated because ripgrep is an external target checkout,
not a checked-in syn_parser fixture. It parses the target `ignore` crate and
locates `Ignore::matched_ignore` through typed graph nodes and
`ImplAssociatedItem` relations.

## Broken Contract

After a mutation is accepted, every later tool step that depends on indexed
content for a touched file must either:

- refresh the crate that owns the touched path and wait for that refreshed graph
  to be visible;
- compose or reject later same-file proposals against the new file version
  before staging; or
- return a model-visible stale-index error that asks the model to re-resolve the
  target.

It must not fall back to a primary or focused crate that is unrelated to the
touched file.

## Evidence

Live log:

```text
/home/brasides/.ploke-eval/logs/ploke_eval_20260608_234321_1555505.log
```

Observed sequence:

- The target checkout was `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`.
- The edited file was `crates/ignore/src/dir.rs`.
- The model first tried `apply_code_edit` for
  `crate::dir::matched_ignore`; that failed because method canonical paths
  require the owning type.
- The model then used `crate::dir::Ignore::matched_ignore`; that semantic edit
  staged and applied.
- `insert_rust_item` then inserted `strip_overlap` in the same file.
- A later semantic edit for
  `crate::dir::tests::git_info_exclude_in_linked_worktree` failed with
  `Content changed for .../crates/ignore/src/dir.rs`.
- Subsequent non-semantic patch attempts did run post-apply refresh, but the log
  showed `scan_for_change in crate_name: globset` followed by
  `No changed files detected`.

The stale hash rejection is expected behavior. The wrong behavior is selecting
`globset` for refresh after mutating `ignore/src/dir.rs`.

## Source Trace

Relevant source path:

- `crates/ploke-tui/src/rag/editing.rs`: the apply paths know the mutated
  `file_paths`, but `rescan_for_changes` does not take them.
- `crates/ploke-tui/src/app_state/database.rs`: `scan_for_change` selects
  `primary_scan_target`.
- `crates/ploke-tui/src/app_state/core.rs`: focused-crate accessors can fall
  back to the first loaded crate.

That shape makes the refresh safe only when the primary/focused crate is also
the owner of every edited path. The live run disproves that assumption for
ripgrep's multi-crate workspace.

## Missing Validation

The missing regression is not a string-trace assertion. The needed production
test should mutate a file in a non-primary loaded crate and assert that the
post-apply refresh targets the owning crate path, or fails loudly when no loaded
crate owns the edited path.

The syn_parser regression added with this report covers the target-item side of
the failure: the parser can parse ripgrep's `ignore` crate and locate
`Ignore::matched_ignore`, so a later TUI/RAG fix can assert refresh and lookup
against typed graph state instead of log strings.

## Fix Direction

- Keep `read_full_verified` and semantic stale-anchor rejection strict.
- Change post-apply refresh to accept touched file paths.
- Resolve each touched path to the loaded crate with the longest matching crate
  root prefix.
- Refresh those owning crates only; do not fall back to primary/focused crate
  selection for path-scoped post-apply work.
- If no loaded crate owns a touched path, return a model-visible/tool-visible
  error instead of silently scanning an unrelated crate.
- Add focused TUI/RAG coverage for a multi-crate workspace where the edited
  member is not the primary loaded crate.

## Related Bugs

- [`../2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md`](../2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md)
- [`../2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md`](../2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md)
- [`../2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](../2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
- [`../2026-05-24-request-code-context-silent-stale-snippet-skip.md`](../2026-05-24-request-code-context-silent-stale-snippet-skip.md)
- [`../2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`](../2026-06-04-prototype1-post-apply-stale-snippet-indexing.md)
- [`../2026-05-15-ploke-tui-create-file-focused-root-path-drift.md`](../2026-05-15-ploke-tui-create-file-focused-root-path-drift.md)
