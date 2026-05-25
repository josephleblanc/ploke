# Active Bugs and Regression-Relevant Reports

Bug reports for issues that are still live, restart-relevant, or needed for
near-term implementation planning. Some entries are retained after mitigation
because they pin regression tests or workflow guardrails; check the individual
file and current code before treating a report as still open.

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
  Prototype 1 child self-validation can export MBE `fix_patch` evidence from a shared benchmark checkout instead of a candidate-owned instance target state; source checkout now rejects non-empty submissions without same-run patch evidence, asserts child repo-cache override roots, and prevents starting-DB cache reuse across checkout roots.
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
- [`2026-05-22-prototype1-continue-protocol-quota-no-progress-loop.md`](./2026-05-22-prototype1-continue-protocol-quota-no-progress-loop.md)
  Prototype 1 `prototype1-continue` retries `baseline_protocol` after Google 429/no-progress protocol failures until the 256-advance guard fires.
- [`2026-05-22-cargo-tool-tail-rendering-and-timeout.md`](./2026-05-22-cargo-tool-tail-rendering-and-timeout.md)
  Cargo tool UI details showed oldest retained output and successful compile progress instead of the final test-result tail; outer tool-call timeout layering can still hide cargo output from the model.
- [`2026-05-22-prototype1-history-metrics-branch-registry-parse.md`](./2026-05-22-prototype1-history-metrics-branch-registry-parse.md)
  Prototype 1 read-only `history metrics` can fail hard on `prototype1-branch-record.v1` JSONL branch registry evidence, blocking progress inspection for an important live campaign.
- [`2026-05-22-prototype1-google-post-apply-indexing-timeout.md`](./2026-05-22-prototype1-google-post-apply-indexing-timeout.md)
  Google broad-harness attempts can apply candidate edits but time out before dense indexing emits submitted Prototype 1 result evidence.
- [`2026-05-22-prototype1-protocol-segmentation-truncated-json.md`](./2026-05-22-prototype1-protocol-segmentation-truncated-json.md)
  Prototype 1 baseline protocol can block when Direct Google returns truncated intent-segmentation JSON before a protocol artifact is persisted.
- [`2026-05-22-prototype1-successor-history-sealed-block-verification.md`](./2026-05-22-prototype1-successor-history-sealed-block-verification.md)
  `p1-google-live-run-20260521-4` produced and evaluated applied children, then failed successor startup because the sealed History block did not verify before storage.
- [`2026-05-24-request-code-context-silent-stale-snippet-skip.md`](./2026-05-24-request-code-context-silent-stale-snippet-skip.md)
  `request_code_context` can silently omit stale snippets under non-strict RAG IO instead of surfacing a tool-level stale-index failure.
- [`2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md`](./2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md)
  Cargo output can be model-visible and repair-relevant while trace summaries hide it, and final cargo checks can resolve to weak focused manifests.
- [`2026-05-24-prototype1-protocol-reasoning-config-blocker.md`](./2026-05-24-prototype1-protocol-reasoning-config-blocker.md)
  Fixed and live-verified: Prototype 1 protocol adjudication carries admitted reasoning policy, and missing policy now resolves through route-aware `auto` defaults so direct-Google protocol calls disable hidden reasoning unless explicitly overridden.
- [`2026-05-24-prototype1-live-preflight-reasoning-budget-false-negative.md`](./2026-05-24-prototype1-live-preflight-reasoning-budget-false-negative.md)
  `prototype1-doctor --live-protocol-preflight` can falsely block reasoning-mandatory models because its 64-token canary budget is consumed by hidden reasoning before sentinel JSON is returned.
- [`2026-05-24-prototype1-protocol-segmentation-json-trailing-characters.md`](./2026-05-24-prototype1-protocol-segmentation-json-trailing-characters.md)
  Prototype 1 baseline protocol blocks when Gemini returns a complete intent-segmentation JSON object followed by an extra top-level closing brace.
- [`2026-05-24-prototype1-protocol-review-malformed-json-retry.md`](./2026-05-24-prototype1-protocol-review-malformed-json-retry.md)
  Prototype 1 tool-call review can block when Gemini returns malformed JSON for a local-analysis adjudication branch; source now retries parse failures instead of salvaging semantic stray text.
- [`2026-05-24-prototype1-protocol-segmentation-anchor-skipped.md`](./2026-05-24-prototype1-protocol-segmentation-anchor-skipped.md)
  Prototype 1 protocol status can see a stored segmentation artifact while aggregate planning skips the anchor and retries live segmentation.
- [`2026-05-24-prototype1-eval-complete-after-aborted-turn.md`](./2026-05-24-prototype1-eval-complete-after-aborted-turn.md)
  Prototype 1 baseline eval can export a patch and mark closure complete even when the terminal agent turn aborted without a final assistant message.
- [`2026-05-24-prototype1-protocol-misses-hidden-apply-failure.md`](./2026-05-24-prototype1-protocol-misses-hidden-apply-failure.md)
  Prototype 1 protocol can mark an edit segment successful from staged tool summaries while a later hidden apply failure leaves the final patch behaviorally incomplete.
- [`2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md`](./2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md)
  Source now rejects already-stale canonical semantic edit anchors before staging; broader same-file proposal composition remains a Prototype 1 risk.
- [`2026-05-25-prototype1-step-env-cwd-preflight-reporting.md`](./2026-05-25-prototype1-step-env-cwd-preflight-reporting.md)
  Prototype 1 baseline eval can fail during embedding preflight when launched from a credential-empty worktree, while `prototype1-step` reports a clean doctor-shaped phase summary.
- [`2026-05-25-parent-patcher-direct-google-provider-preference.md`](./2026-05-25-parent-patcher-direct-google-provider-preference.md)
  Fixed in source: broad headless-TUI parent patching now ignores stale OpenRouter provider preferences for direct-Google registry rows while explicit provider pins still validate.
- [`2026-05-25-prototype1-child-plan-publishes-unmaterialized-slots.md`](./2026-05-25-prototype1-child-plan-publishes-unmaterialized-slots.md)
  Prototype 1 child planning can publish prompt files for broad-harness slots whose candidate workspaces were never materialized, causing doctor prompt preflight to block further progress.
- [`2026-05-25-prototype1-overlapping-protocol-step-duplicates-artifacts.md`](./2026-05-25-prototype1-overlapping-protocol-step-duplicates-artifacts.md)
  Prototype 1 does not guard concurrent protocol advances for the same run, allowing duplicate per-call adjudication artifacts while closure still reports protocol complete.
- [`2026-05-25-headless-tui-timeout-submission-admission.md`](./2026-05-25-headless-tui-timeout-submission-admission.md)
  Fixed in source: Prototype 1 broad headless-TUI attempts no longer publish submitted child results after a timed-out turn with failed cargo validation; the live campaign that exposed this still contains tainted r3 evidence.
- [`2026-05-25-prototype1-observe-child-stale-hang.md`](./2026-05-25-prototype1-observe-child-stale-hang.md)
  Fixed in source: `observe_child` no longer waits forever when a child stops producing channel output/result evidence, and doctor/replay classify stale pending observe states.
