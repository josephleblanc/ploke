# Prototype 1 Eval Review: `p1-gemini35-flash-direct-fresh-20260524-190632`

## Verdict

This fresh direct-Google baseline eval and protocol pass produced useful
evidence. The loop got past the earlier route/reasoning and worktree-env
blockers, generated a non-empty patch, recorded all provider tool calls,
exported a Multi-SWE-bench submission, and completed all required protocol
procedures after retrying malformed direct-Google adjudication responses.

The patch is not clean enough to call a benchmark-quality success yet:
`cargo test -p grep-printer` passes, but `cargo fmt -- --check` fails on
`crates/printer/src/util.rs`. The final model response overclaims success
because the run never executed a formatting check and its final `cargo check`
again resolved to an unrelated focused `globset` manifest.

## Evidence Roots

- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260524-190632`
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260524-190632`
- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-190632/BurntSushi__ripgrep-2209/runs/run-1779674853833-structured-current-policy-2897a178`
- Target checkout:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`

## Closure State

Post-eval closure:

- `eval.status = complete`
- `eval.complete_total = 1`
- doctor phase moved to `baseline_protocol`

After two bounded protocol steps:

- `protocol.status = complete`
- `tool-call-intent-segments = complete`
- `tool-call-review = complete`
- `tool-call-segment-review = complete`
- protocol status reports `artifact_count = 61`
- `call_review_count = 56`
- `segment_review_count = 4`
- `missing_call_indices = []`
- `missing_segment_indices = []`
- doctor phase moved to `child_plan`

The campaign manifest records the intended model route:

- `model_id = google/gemini-3.5-flash`
- `route_source = direct_google`

Before the step, live protocol preflight passed with:

- `provider = google`
- `route_source = direct_google`
- `reasoning = omit`

## Eval And Patch Output

Patch projection:

- `benchmark-patch-projection.check.status = passed`
- submission byte length: `3715`
- submission line count: `104`
- changed files:
  - `crates/printer/src/util.rs`
  - `crates/printer/src/standard.rs`

The patch changes `Replacer::replace_all` to use `captures_iter_at`, stop when
the match starts at or after `range.end`, and append only up to
`min(range.end, subject.len())`. It also adds
`standard::tests::replacement_multi_line_duplicate`.

Direct verification after the run:

```text
cargo test -p grep-printer
```

passed: `95` unit tests and `2` doc tests.

Direct formatting check after the run:

```text
cargo fmt -- --check
```

failed with:

```text
Diff in crates/printer/src/util.rs:47
-        pub fn replace_all<'a>(
+    pub fn replace_all<'a>(
```

## LLM And Tool Behavior

Trace audit summary:

- provider responses: `57`
- provider-emitted tool calls: `56`
- recorded tool calls: `56`
- missing recorded provider call ids: `0`
- finish reasons: `56` `tool_calls`, `1` `stop`

The model did localize the relevant code. It read `crates/printer/src/util.rs`,
searched for replacement-related code, inspected matcher APIs, and eventually
edited `crate::util::Replacer::replace_all`.

The model also attempted to add a regression test semantically:

- `insert_rust_item` failed with `No inline module container found for
  crate::standard::tests`.
- The model recovered by using `non_semantic_patch` to insert the test into
  `crates/printer/src/standard.rs`.

Validation was mixed:

- The model ran `cargo test -p grep-printer` after the edits and saw success.
- It did not run `cargo fmt -- --check`.
- Its final `cargo check` resolved to
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml`,
  which is not a meaningful check for changes in `crates/printer`.

`agent-turn-summary.json` has `terminal_record.outcome = completed`, but
`final_assistant_message = null`. The final response is present in
`llm-full-responses.jsonl` at response index `56`, so this is a summary-surface
gap rather than a missing provider final response.

## Positive Examples And Adjudication Candidates

Positive examples:

- The initial retrieval included the relevant existing
  `replacement_multi_line` test and `find_iter_at_in_context` boundary logic.
- A failed semantic test insertion produced model-visible feedback, and the
  model switched to another edit tool instead of stopping.
- The model ran the focused target tests after editing, and the new test was
  included in the successful `running 95 tests` result.
- Protocol did not corrupt state when direct Google returned malformed JSON
  with a valid object followed by stray sentence fragments. The retry wrapper
  retried individual `tool_call_review` and `tool_call_segment_review` items,
  and the second bounded protocol step completed with all `56` calls and all
  `4` segments reviewed.

Adjudication candidates:

- Mark `cargo test -p grep-printer` as a positive validation signal.
- Mark missing `cargo fmt -- --check` as a patch-hygiene gap.
- Mark final `cargo check` as weak when the resolved manifest is unrelated to
  the changed files.
- Track semantic edit fallback chains, especially
  `insert_rust_item failed -> non_semantic_patch applied`.
- Track summary parity for final assistant messages:
  `llm-full-responses.jsonl` contains the final answer while
  `agent-turn-summary.json` reports `final_assistant_message = null`.
- Track adjudication retry pressure separately from final protocol status:
  this run completed, but several review calls needed retry because direct
  Google appended non-JSON text after an otherwise complete JSON object.

## What Is Working

- Direct Google route setup and live protocol canary are passing in the fresh
  run when commands are launched from the env-bearing source checkout.
- Baseline eval can now run through to a recorded, non-empty patch.
- Provider and recorded tool-call ledgers agree on call identity.
- The model found the relevant subsystem and produced a plausible patch.
- Protocol segmentation, call review, and segment review can complete against
  the recorded eval run, including retry recovery from malformed adjudication
  JSON.

## What Is Not Working Yet

- Eval completion does not imply format-clean or benchmark-clean patch quality.
- Cargo validation remains misleading when `cargo check` resolves to an
  unrelated focused manifest.
- Trace summaries still require deeper inspection for model-visible cargo
  payloads and final assistant response parity.

## Action Items

- Continue from doctor phase `child_plan` with one bounded step.
- Update the cargo validation bug with this second direct-Google recurrence.
- Keep this run as positive evidence for route/env/protocol progress, but do
  not treat it as benchmark-clean because formatting still fails.
