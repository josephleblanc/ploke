# Active Bugs

Current bug reports for issues that are still live, restart-relevant, or needed
for near-term implementation planning.

- [`2026-03-21-indexworkspace-relative-target-regression.md`](./2026-03-21-indexworkspace-relative-target-regression.md)
  `IndexWorkspace` relative target re-resolution regression in `ploke-tui`.
- [`2026-04-10-qwen-reasoning-content-deserialization-failure.md`](./2026-04-10-qwen-reasoning-content-deserialization-failure.md)
  Provider response-deserialization failure for reasoning-content payloads.
- [`2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md`](./2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)
  Fixed Rust 2015 bare-trait-object parse blocker in `syn_parser`.
- [`2026-04-15-observability-test-todo-panic.md`](./2026-04-15-observability-test-todo-panic.md)
  Test-only observability `todo!()` panic tracking note.
- [`2026-04-15-protocol-segment-review-index-failure.md`](./2026-04-15-protocol-segment-review-index-failure.md)
  `tool-call-segment-review` rejects valid persisted segment indices.
- [`2026-04-17-generic-lifetime-transform-failure.md`](./2026-04-17-generic-lifetime-transform-failure.md)
  Parsed-workspace transform fails on `generic_lifetime` relation writes for current `nushell` and `serde` runs.
- [`2026-04-17-nushell-duplicate-commands-module-path.md`](./2026-04-17-nushell-duplicate-commands-module-path.md)
  `nu-cli` indexing fails with duplicate `crate::commands` module-path collisions.
- [`2026-04-17-nushell-indexing-completed-timeout.md`](./2026-04-17-nushell-indexing-completed-timeout.md)
  Current `nushell` runs timing out at `indexing_completed` after 300 seconds.
- [`2026-04-18-eval-patch-artifact-collision-and-empty-diff.md`](./2026-04-18-eval-patch-artifact-collision-and-empty-diff.md)
  Eval runs can mix arms or report successful patch activity without a trustworthy final diff.
- [`2026-04-18-openrouter-codestral-embed-404-fallback.md`](./2026-04-18-openrouter-codestral-embed-404-fallback.md)
  Live eval RAG requests currently fall back to conversation-only mode after OpenRouter embeddings return `404` for Codestral.
- [`2026-04-18-semantic-edit-applied-zero-writes.md`](./2026-04-18-semantic-edit-applied-zero-writes.md)
  Semantic edit approval could present zero-write proposals as `Applied`, polluting patch summaries.
- [`2026-04-18-multi-edit-apply-result-accounting.md`](./2026-04-18-multi-edit-apply-result-accounting.md)
  Same-file multi-edit apply and result accounting lose per-edit semantics and can under-report failures.
- [`2026-04-18-arm-agnostic-latest-run-selection.md`](./2026-04-18-arm-agnostic-latest-run-selection.md)
  Read-side run selection still picks the newest run dir without respecting control vs treatment arms.
- [`2026-04-21-provider-tool-call-argument-malformation-without-repair.md`](./2026-04-21-provider-tool-call-argument-malformation-without-repair.md)
  Provider-emitted malformed or schema-invalid tool-call arguments are accepted without a repair/retry path.
- [`2026-05-09-prototype1-history-traversal-membership-mismatch.md`](./2026-05-09-prototype1-history-traversal-membership-mismatch.md)
  Prototype 1 History traversal can select a membership that is absent from the final sealed considered set.
- [`2026-05-10-prototype1-historical-successor-surface-root-mismatch.md`](./2026-05-10-prototype1-historical-successor-surface-root-mismatch.md)
  Prototype 1 can seal a historical successor Artifact with the previous parent's mutated surface root.
- [`2026-05-10-prototype1-successor-hydration-surface-mismatch.md`](./2026-05-10-prototype1-successor-hydration-surface-mismatch.md)
  Prototype 1 compares selected-child Artifact surface evidence against the hydrated successor Parent checkout after parent identity is committed.
- [`2026-05-11-prototype1-mbe-shared-instance-patch-provenance.md`](./2026-05-11-prototype1-mbe-shared-instance-patch-provenance.md)
  Prototype 1 child self-validation can export MBE `fix_patch` evidence from a shared benchmark checkout instead of a candidate-owned instance target state.
- [`2026-05-11-prototype1-workspace-except-eval-selects-archive-targets.md`](./2026-05-11-prototype1-workspace-except-eval-selects-archive-targets.md)
  Prototype 1 live edit-surface generation still uses a deterministic mock target picker; archive/core target selection was mitigated by `5f92eb6e`, and MBE validation needs rerun.
- [`2026-05-12-prototype1-broad-harness-request-plan-erasure.md`](./2026-05-12-prototype1-broad-harness-request-plan-erasure.md)
  Prototype 1 `BroadHarness` erases pending request state and request-bound child-plan provenance into flat generator/validation paths.
- [`2026-05-15-ploke-tui-create-file-focused-root-path-drift.md`](./2026-05-15-ploke-tui-create-file-focused-root-path-drift.md)
  `ploke-tui` can resolve workspace-relative file-tool paths against the focused crate after reindex, producing doubled member paths in broad harness slots.
- [`2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md`](./2026-05-15-ns-patch-apply-missing-rescan-causes-stale-index-retries.md)
  Non-semantic patch apply claims a rescan was scheduled but does not trigger one, leaving broad-harness sessions on stale indexed file state and causing repeated `NsContentMismatch` retries.
- [`2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md`](./2026-05-17-headless-tui-same-file-ns-patch-stale-anchor-retries.md)
  Headless TUI attempts can keep applying or retrying same-file `ns_patch` proposals after an earlier accepted proposal has already invalidated their staged file hashes.
- [`2026-05-17-headless-tui-staged-proposal-tool-result-lifecycle.md`](./2026-05-17-headless-tui-staged-proposal-tool-result-lifecycle.md)
  Headless TUI `ns_patch` staging is replayed to the model as a completed tool result before proposal admission/apply decides whether the workspace changed.
- [`2026-05-19-rf-05-edit-composition-same-file-repair.md`](./2026-05-19-rf-05-edit-composition-same-file-repair.md)
  RF-05 headless TUI repeated same-file repair attempts can materialize malformed intermediate Rust unless edits are composed, invalidated, or rejected before candidate artifact submission.
- [`2026-05-19-rf-08-headless-tui-evidence-read-roots.md`](./2026-05-19-rf-08-headless-tui-evidence-read-roots.md)
  RF-08 broad-harness prompts advertise campaign evidence that the headless TUI read-root policy only partially admits, causing avoidable outside-root and missing-artifact navigation failures.
