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
