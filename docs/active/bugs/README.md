# Active Bugs and Regression-Relevant Reports

Bug reports for issues that are still live, restart-relevant, or needed for
near-term implementation planning. Some entries are retained after mitigation
because they pin regression tests or workflow guardrails; check the individual
file and current code before treating a report as still open.

- [`post-apply-freshness/`](./post-apply-freshness/)
  Cluster index for stale post-apply refresh/re-resolve bugs across semantic
  edits, non-semantic patches, snippet retrieval, and wrong-crate scan
  selection.
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
  Prototype 1 no-progress protocol quota handling; `prototype1-step` now live-verifies the blocking diagnostic, and protocol tool-review fanout is configurable while `prototype1-continue` still needs live verification.
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
- [`2026-05-25-prototype1-child-plan-zero-admission-timeout.md`](./2026-05-25-prototype1-child-plan-zero-admission-timeout.md)
  Prototype 1 child planning can exhaust broad headless-TUI slots with zero admitted children while doctor still reports the campaign as steppable.
- [`2026-05-25-prototype1-overlapping-protocol-step-duplicates-artifacts.md`](./2026-05-25-prototype1-overlapping-protocol-step-duplicates-artifacts.md)
  Prototype 1 does not guard concurrent protocol advances for the same run, allowing duplicate per-call adjudication artifacts while closure still reports protocol complete.
- [`2026-05-25-headless-tui-timeout-submission-admission.md`](./2026-05-25-headless-tui-timeout-submission-admission.md)
  Fixed in source: Prototype 1 broad headless-TUI attempts no longer publish submitted child results after a timed-out turn with failed cargo validation; the live campaign that exposed this still contains tainted r3 evidence.
- [`2026-05-25-prototype1-observe-child-stale-hang.md`](./2026-05-25-prototype1-observe-child-stale-hang.md)
  Fixed in source: `observe_child` no longer waits forever when a child stops producing channel output/result evidence, doctor/replay classify stale pending observe states, and doctor now blocks acknowledged dead-child state before observe starts.
- [`2026-05-25-prototype1-observe-child-success-sidecar-race.md`](./2026-05-25-prototype1-observe-child-success-sidecar-race.md)
  Fixed in source: `observe_child` no longer treats a successful runner-result sidecar as terminal before the treatment-bearing channel result arrives.
- [`2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md`](./2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md)
  Prototype 1 broad headless-TUI child planning retried direct-Google HTTP 401 provider failures as ordinary no-edit/slot failures instead of stopping as an environment blocker.
- [`2026-05-25-prototype1-baseline-eval-nonterminal-registration.md`](./2026-05-25-prototype1-baseline-eval-nonterminal-registration.md)
  Fixed in source: doctor now blocks generation 0 baseline eval when the run registry already contains a nonterminal registered attempt or closure state records partial eval evidence.
- [`2026-05-26-prototype1-treatment-closure-misses-live-child-run.md`](./2026-05-26-prototype1-treatment-closure-misses-live-child-run.md)
  Focused live child runner can produce a valid patch and artifacts while treatment closure rejects the completed run registration due to lexical path mismatch.
- [`2026-05-26-prototype1-foreground-child-recovery-misses-parent-comparison.md`](./2026-05-26-prototype1-foreground-child-recovery-misses-parent-comparison.md)
  Fixed in source: Prototype 1 now re-enters observe, or blocks loudly before selection, when a succeeded child has terminal treatment evidence but lacks the parent comparison artifact needed for successor selection.
- [`2026-06-04-prototype1-headless-timeout-after-apply.md`](./2026-06-04-prototype1-headless-timeout-after-apply.md)
  Fixed in source: Prototype 1 broad headless-TUI slots now preserve typed post-apply timeout/abort/validation terminals instead of collapsing applied candidates into plain `timed_out`; direct-Google missing-validation canary passed, while same-target post-apply timeout validation remains pending.
- [`2026-06-04-prototype1-headless-tui-runtime-actor-leak.md`](./2026-06-04-prototype1-headless-tui-runtime-actor-leak.md)
  Fixed in source: Prototype 1 broad headless-TUI runtimes now retain spawned actor handles in `WorkspaceTuiRuntime` so completed patch attempts do not leave detached TUI actors holding runtime state.
- [`2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`](./2026-06-04-prototype1-post-apply-stale-snippet-indexing.md)
  Fixed in source: recorded headless-TUI tape coverage now proves post-apply refresh retracts stale snippet rows before indexing can emit `ContentMismatch` or byte-range warnings after truncating edits; fresh live-log validation remains pending.
- [`2026-06-04-type-context-missing-type-contains-relation.md`](./2026-06-04-type-context-missing-type-contains-relation.md)
  TUI/RAG type-context expansion can query `type_contains` against a database that was imported through a plain/active fixture path and does not contain typed type graph relations.
- [`2026-06-05-prototype1-protected-core-path-stale-after-backend-split.md`](./2026-06-05-prototype1-protected-core-path-stale-after-backend-split.md)
  Fixed in source: broad-harness protected-core metadata and doctor prompt preflight now point at `backend/mod.rs` after the backend module split instead of stale `backend.rs`.
- [`2026-06-06-code-item-lookup-impl-relation-missing-name-field.md`](./2026-06-06-code-item-lookup-impl-relation-missing-name-field.md)
  `code_item_lookup` with `node_kind: impl` issues a Cozo query against a `name` field the stored `impl` relation does not have; protocol marked call `[36]` recoverability-blocked in `p1-admissionfix-g35flash-p25flash-20260606-053302` treatment run.
- [`2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md`](./2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md)
  Fixed in source: broad headless-TUI setup failures now persist typed setup diagnostics and doctor exposes a no-model-call sparse/BM25 setup preflight before spending broad slots.
- [`2026-06-07-prototype1-baseline-embedding-overrides-dropped.md`](./2026-06-07-prototype1-baseline-embedding-overrides-dropped.md)
  Prototype 1 baseline eval dropped setup embedding overrides at campaign/closure/batch boundaries and fell back to the default Codestral embedding route.
- [`2026-06-08-prototype1-successor-handoff-stale-parent-identity.md`](./2026-06-08-prototype1-successor-handoff-stale-parent-identity.md)
  Fixed in source: Prototype 1 successor handoff now commits the selected parent identity into the successor Artifact before measuring and spawning it; the live campaign that exposed this was stopped after stale gen1 identity evidence.
- [`2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md`](./2026-06-08-prototype1-direct-google-g35flash-quota-empty-baseline.md)
  External provider blocker: a fresh direct-Google `google/gemini-3.5-flash` baseline turn aborted with HTTP 429 resource exhaustion and persisted an empty patch, so the campaign is stop-use for loop progress.
- [`2026-06-08-prototype1-child-treatment-google-adc-reauth.md`](./2026-06-08-prototype1-child-treatment-google-adc-reauth.md)
  External auth blocker: child treatment evals reached `direct_google` but both target turns aborted because Google ADC bearer-token resolution required non-interactive reauthentication.
- [`2026-06-09-prototype1-broad-child-google-429-zero-admission.md`](./2026-06-09-prototype1-broad-child-google-429-zero-admission.md)
  External provider-capacity blocker: fresh `prototype1-state` baseline/protocol completed, but broad child generation admitted zero children after direct-Google HTTP 429 `RESOURCE_EXHAUSTED`.
- [`2026-06-09-prototype1-late-child-result-recovery-corrupts-node-state.md`](./2026-06-09-prototype1-late-child-result-recovery-corrupts-node-state.md)
  Open blocker: a gen2 child result arrived after parent `observe_child` timeout, leaving no branch/evaluation record; direct `prototype1-state` re-entry then re-ran child prep and downgraded terminal node states.
- [`2026-06-10-vertex-gemini-35-flash-dsq-shadow-quota-429.md`](./2026-06-10-vertex-gemini-35-flash-dsq-shadow-quota-429.md)
  External provider blocker: Vertex `direct_google` `google/gemini-3.5-flash` hits DSQ/PayGo HTTP 429 without IAM Quotas `base_model` visibility; downgrade to **`google/gemini-2.5-flash-lite`** (not 3.0-flash, which 404s on Vertex).
- [`2026-06-10-direct-google-malformed-function-call-finish-reason.md`](./2026-06-10-direct-google-malformed-function-call-finish-reason.md)
  Direct Google Gemini can return `finish_reason: malformed_function_call` with Python-style refusal text; Ploke now deserializes and classifies it as model behavior instead of entering `UNKNOWN_TOOL_NAME` repair. Recurs on `2.5-pro`/`non_semantic_patch` (state6); captured-payload + live eval-shape repro tests added; provider-side multi-line-patch malformation under `tool_choice=auto` remains the open item (ranked fixes A patch-arg shaping / B `tool_choice` required/validated).
- [`2026-06-12-prototype1-direct-google-gemini-15-flash-404.md`](./2026-06-12-prototype1-direct-google-gemini-15-flash-404.md)
  External provider/config blocker: fresh Prototype 1 setup admitted `google/gemini-1.5-flash` as the direct-Google protocol model, but doctor live preflight blocked with Vertex HTTP 404 before loop advance.
- [`2026-07-08-prototype1-max-generation-handoff.md`](./2026-07-08-prototype1-max-generation-handoff.md)
  Source repair restored after rollback: a child at `max_generations` now produces a stopped continuation instead of a successor that immediately fails its parent-turn budget.
- [`2026-07-08-prototype1-walk-reconstruct-missing-baseline.md`](./2026-07-08-prototype1-walk-reconstruct-missing-baseline.md)
  Source repair restored after rollback: wholly missing pre-baseline closure evidence reconstructs only through R5 while failed, partial, and inconsistent closure evidence remains strict.
- [`2026-07-13-prototype1-doctor-headless-preflight-rebuild-hang.md`](./2026-07-13-prototype1-doctor-headless-preflight-rebuild-hang.md)
  Resolved shared-library contract: BM25 rebuild admission now rejects full/closed mailboxes explicitly, status deadlines cover enqueue plus response, and the apparent live hang was corrected to a roughly two-minute workspace-ingestion run that passed.
- [`2026-07-13-prototype1-embedding-preflight-after-admission.md`](./2026-07-13-prototype1-embedding-preflight-after-admission.md)
  Repaired readiness gap plus active external blocker: protocol-v10 doctor now proves direct-Google and headless readiness, but all exported OpenRouter credentials fail and the default key's monthly limit still prevents the multi-generation live walk.
- [`2026-07-13-walk-summary-pre-child-plan.md`](./2026-07-13-walk-summary-pre-child-plan.md)
  Open read-model bug: `walk summary` treats a legitimately absent pre-child-plan directory as a malformed run instead of rendering the baseline/pre-plan phase, while later missing authority must still fail closed.
- [`2026-07-13-prototype1-setup-receipt-recovery.md`](./2026-07-13-prototype1-setup-receipt-recovery.md)
  Fixed and live verified: receipt-first setup now resumes partial campaign, closure, database, scheduler, branch, and inherited-identity effects, while completed retries are read-only.
- [`2026-07-13-prototype1-doctor-relative-repo-root.md`](./2026-07-13-prototype1-doctor-relative-repo-root.md)
  Fixed and live verified: shared Prototype 1 control context canonicalizes `--repo-root .` before absolute-path headless indexing and other driver work.
- [`2026-07-14-walk-pre-session-phase-and-start.md`](./2026-07-14-walk-pre-session-phase-and-start.md)
  Fixed and live verified: walk protocol v9 separates strict pre-session reconstruction, cursorless durable sessions, and the committed controller cursor, restores the first guarded Start at R3, and rejects reuse of the old empty guard before persistence.
- [`2026-07-14-ploke-eval-walk-worker-stack-overflow.md`](./2026-07-14-ploke-eval-walk-worker-stack-overflow.md)
  Fixed and live verified: the debug R3-to-R4a walk task exceeded Tokio's default 2 MiB worker stack after acquiring its durable fence, so the production binary now owns an explicit worker-stack budget and a fail-before/fix-after worker-capacity regression.
- [`2026-07-14-walk-until-same-rank-edge-rejection.md`](./2026-07-14-walk-until-same-rank-edge-rejection.md)
  Fixed and live verified: `walk step --until r4c` rejected the advertised R4b-to-R4c edge because the controller's coarse phase rank grouped two phases connected by a real forward edge.
- [`2026-07-14-direct-openai-embedding-overlong-snippet.md`](./2026-07-14-direct-openai-embedding-overlong-snippet.md)
  Open backend-policy gap with config recovery selected: direct OpenAI accepted readiness preflight but rejected a 66,807-byte parsed node at production indexing; the failed run is preserved and the fresh run will use OpenRouter's approved truncation policy.
- [`2026-07-14-prototype1-parent-patcher-role-not-admitted.md`](./2026-07-14-prototype1-parent-patcher-role-not-admitted.md)
  Config-mitigated readiness gap: setup and doctor admitted direct-Google eval/protocol settings without surfacing the separate OpenRouter parent-patcher role that later exhausted all broad child slots with HTTP 402.
- [`2026-07-14-walk-trace-show-tool-arguments-newtype.md`](./2026-07-14-walk-trace-show-tool-arguments-newtype.md)
  Source-repaired typed IPC bug: completed sealed traces containing `ToolArgumentsJson` failed client decoding when buffered inside the internally tagged trace state.
- [`2026-07-14-walk-llm-inspection-controller-lock.md`](./2026-07-14-walk-llm-inspection-controller-lock.md)
  Source-repaired authority-boundary bug: persisted LLM inspector commands waited behind the mutation controller for an entire live typestate edge.
- [`2026-07-15-prototype1-successor-retirement-before-walk-receipt.md`](./2026-07-15-prototype1-successor-retirement-before-walk-receipt.md)
  Fixed and live verified: successor retirement now waits through typed active-job
  drain until the outer R12-to-R13b receipt is terminal; a fresh canary preserved
  the successor across a 12.65-second projection gap and transferred unpinned
  status to its R4c endpoint.
- [`2026-07-15-prototype1-child-runner-processes-not-reaped.md`](./2026-07-15-prototype1-child-runner-processes-not-reaped.md)
  Source repaired, live revalidation pending: C3 now retains explicit child
  process ownership through Ready acknowledgement, reaps normal exits, and
  kills plus waits pre-ack timeout/error paths.
- [`2026-07-15-prototype1-parallel-trace-capture-cross-talk.md`](./2026-07-15-prototype1-parallel-trace-capture-cross-talk.md)
  Fixed and live verified: three parallel broad attempts now carry exact
  session-owned response/debug capture with ordered 46/46, 9/9, and 39/39
  debug-to-tape joins and no cross-lane response-ID overlap.
- [`2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md`](./2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md)
  Source repaired: timeout cancellation preserves the caller's partial
  headless run and drains response capture to disconnection; live failure-path
  persistence is verified, while a post-repair outer-timeout canary remains.
- [`2026-07-15-walk-llm-corrupt-neighbor-hides-sessions.md`](./2026-07-15-walk-llm-corrupt-neighbor-hides-sessions.md)
  Fixed and live verified: protocol-v10 typed LLM inventory reports a zero-byte
  session manifest independently while retaining every healthy lane/session for
  exact CLI and UI inspection.
- [`2026-07-16-prototype1-headless-unpersisted-workspace-mutation.md`](./2026-07-16-prototype1-headless-unpersisted-workspace-mutation.md)
  Open crash-consistency gap: an effectful headless tool can mutate its isolated
  workspace after the last settled debug step, leaving recovery without durable
  provider intent for the mutation.
- [`2026-07-16-prototype1-request-code-context-no-progress-loop.md`](./2026-07-16-prototype1-request-code-context-no-progress-loop.md)
  Source repaired: a broad-only session guard now stops one-tool no-progress
  loops after 15 calls, but only after the threshold batch and terminal trace
  settle; checked-in R2 replay coverage is active in the normal test suite.
- [`2026-07-16-prototype1-r12-policy-stop-routing.md`](./2026-07-16-prototype1-r12-policy-stop-routing.md)
  Source repaired, live validation pending: R12 policy stops now route from the
  continuation disposition instead of treating selected-candidate evidence as
  successor-handoff authority.
- [`2026-07-16-walk-until-target-second-claim.md`](./2026-07-16-walk-until-target-second-claim.md)
  Source repaired, live validation pending: a bounded walk now returns as soon
  as its committed edge reaches the requested target instead of claiming the
  transferred successor session a second time.
- [`2026-07-16-prototype1-post-edit-indexer-stall.md`](./2026-07-16-prototype1-post-edit-indexer-stall.md)
  Source repaired, live validation pending: an unbounded post-edit scan barrier
  left one broad lane and the R7-to-R8 walk edge nonterminal while the server
  remained CPU- and memory-hot; v14 is preserved as abandoned evidence.
- [`2026-07-17-prototype1-handoff-replaced-executable-epoch.md`](./2026-07-17-prototype1-handoff-replaced-executable-epoch.md)
  Source repaired, fresh strict live validation pending: a later-generation
  rebuild replaced the live predecessor binary pathname, so epoch capture now
  retains the running inode while keeping replacement clients incompatible.
- [`2026-07-17-prototype1-operational-keep-without-oracle-proof.md`](./2026-07-17-prototype1-operational-keep-without-oracle-proof.md)
  Source repaired, fresh strict live validation pending: v15 reached R12 with
  an operationally kept child but no oracle evaluation, so successor admission
  now has a separate fail-closed `all-resolved` gate bound to exact profile
  targets without weakening handoff or digest verification.
- [`2026-07-17-prototype1-post-edit-refresh-panic-hang.md`](./2026-07-17-prototype1-post-edit-refresh-panic-hang.md)
  Source repaired, fresh strict live validation pending: v20 wrote a duplicate
  semantic item, then a correct parser invariant panic escaped the post-edit
  task without a terminal tool event; exact historical replay now proves
  durable `PartiallyApplied` failure evidence instead of a hang.
- [`2026-07-17-prototype1-late-child-observer-timeout.md`](./2026-07-17-prototype1-late-child-observer-timeout.md)
  Config-mitigated, source recovery open: v21 produced a strict Keep but its
  third healthy child completed after the 1,200-second parent fence, leaving
  R10 indeterminate without a safe late-result reconciliation path.
- [`2026-07-17-prototype1-selection-receipt-float-roundtrip.md`](./2026-07-17-prototype1-selection-receipt-float-roundtrip.md)
  Source repaired, fresh strict live validation pending: v22 reached R11 but
  default typed JSON decoding moved one selection-formula float by one ULP;
  exact float replay plus a write-side rehash guard preserves the strict loader.
- [`2026-07-17-prototype1-selection-without-candidate-safety-proof.md`](./2026-07-17-prototype1-selection-without-candidate-safety-proof.md)
  Source repaired, fresh strict handoff proof pending: v25 reached a valid R12
  but selected an unsafe candidate; v26 proved the artifact-bound gate and then
  failed closed on a truncated third review at R10. Exact historical replay,
  bounded fresh-adjudication retry, and fail-closed DB/audit bindings now guard
  selection without modifying either preserved run.
- [`2026-07-18-prototype1-reconstruct-node-cap-after-plan.md`](./2026-07-18-prototype1-reconstruct-node-cap-after-plan.md)
  V28 replay blocker: reconstruction reapplied the live pre-planning node-cap
  check after a legal child plan filled the final node slots. Exact historical
  replay now passes without weakening live cap admission; fresh strict terminal
  validation remains.
- [`2026-07-20-walk-terminal-read-only-startup.md`](./2026-07-20-walk-terminal-read-only-startup.md)
  Fixed and live verified: completed V29 now starts a protocol-v11 read-only
  inspection endpoint while every controller mutation remains rejected with a
  typed terminal blocker and its run-history hashes remain unchanged.
- [`2026-07-20-walk-use-invalid-repo-root.md`](./2026-07-20-walk-use-invalid-repo-root.md)
  Fixed and live verified: `walk use` rejects missing and non-directory roots
  before replacing saved context, while an absolute V29 checkout works for
  subsequent short-form commands.
