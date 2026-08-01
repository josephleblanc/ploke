# Post-Apply Freshness Bug Cluster

These reports track one contract family: after any accepted file mutation, the
next semantic lookup, snippet read, proposal stage, or refresh must observe the
edited file version for the crate that owns the edited path. The stale-anchor
guards are correct; the bug is allowing later work to proceed from stale or
wrong-crate graph state.

## Reports

- [`2026-06-09-prototype1-post-apply-refresh-selects-wrong-crate.md`](./2026-06-09-prototype1-post-apply-refresh-selects-wrong-crate.md)
  Latest live-run recurrence: post-apply refresh ran against `globset` while
  the mutated file belonged to ripgrep's `ignore` crate.
- [`2026-06-09-targeted-post-apply-refresh-implementation.md`](./2026-06-09-targeted-post-apply-refresh-implementation.md)
  Follow-up implementation record for path-scoped post-apply refresh and
  targeted multi-crate regression coverage.
- [`../2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md`](../2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md)
  Semantic edit staging correctly rejects stale same-file anchors, but broader
  same-file composition and refresh ordering remain risky.
- [`../2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md`](../2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md)
  Same-file non-semantic patch attempts can keep using stale file hashes after
  an earlier accepted proposal mutates the file.
- [`../2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](../2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
  Non-semantic apply once claimed a rescan but failed to actually refresh.
- [`../2026-05-24-request-code-context-silent-stale-snippet-skip.md`](../2026-05-24-request-code-context-silent-stale-snippet-skip.md)
  Stale snippet IO can become hidden degraded context unless surfaced as a
  model-visible refresh/re-resolve requirement.
- [`../2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`](../2026-06-04-prototype1-post-apply-stale-snippet-indexing.md)
  Post-apply indexing must retract stale snippet rows before later retrieval.
- [`../2026-05-15-ploke-tui-create-file-focused-root-path-drift.md`](../2026-05-15-ploke-tui-create-file-focused-root-path-drift.md)
  Focused-crate fallback can drift file-tool path interpretation away from the
  workspace root and is related to wrong-crate refresh selection.
