# Prototype 1 Run Review: Clean Gemini 3.5 Flash Direct, ripgrep-2209

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-15g2x3-clean-20260525-120225`

Run: `run-1779710592442-structured-current-policy-8bcdf6e0`

Task: `BurntSushi__ripgrep-2209`

Review status: complete. The run-review Quality Gate passes because the eval
execution path is artifact-backed, the provider/tool/edit/validation trace is
joinable, at least one stale-edit and recovery chain is reconstructable, and the
patch was compared against the issue/gold shape and live checkout. Protocol
artifacts began appearing while this review was in progress; they are labeled as
later protocol output and are not used as baseline eval trace evidence.

## Short Verdict

The eval mechanically completed and exported a non-empty patch. It also
produced real model progress: the model localized the replacement bug to
`Replacer::replace_all`, added an internal bounded replacement helper, observed
failing `grep-printer` tests, repaired the helper, and ended with a cargo test
pass against the root manifest covering the changed files.

The patch is only partially benchmark-useful. Its production change has the
same core shape as the gold fix: stop replacement matches at the original
search end while still allowing the PCRE2 look-ahead buffer. But the submitted
test is weak and was made weaker late in the run: the model changed the new
test from `a(?=\nb)` to plain `a` so it no longer exercises look-around or the
reported issue. The gold test suite adds regression tests for issues 2095 and
2208 under `tests/regression.rs`; this patch adds only a local unit test in
`crates/printer/src/standard.rs`.

The `TOOL_EXECUTION_FAILED` and stale same-file failures seen live should not be
read as terminal failure for this run. Durable artifacts show `terminal_record`
`outcome: completed`, `attempts: 78`, and a final provider `stop` response.
They also show real tool failures: stale same-file `apply_code_edit` rejection
for `util.rs`, `code_item_lookup` and empty `read_file` problems around
`standard.rs`, and six recorded transport failures. The model recovered enough
to finish, but it finished with an overclaiming final answer.

## Evidence Roots

- Campaign state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-clean-20260525-120225`
- Run manifest:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-clean-20260525-120225/BurntSushi__ripgrep-2209/run.json`
- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-clean-20260525-120225/BurntSushi__ripgrep-2209/runs/run-1779710592442-structured-current-policy-8bcdf6e0`
- Run registration:
  `/home/brasides/.ploke-eval/registries/runs/run-1779710592442-structured-current-policy-8bcdf6e0.json`
- Target checkout:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`
- Dataset slice with gold patch/tests:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-clean-20260525-120225/slice.jsonl`
- Later protocol anchor, not baseline eval trace:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-15g2x3-clean-20260525-120225/BurntSushi__ripgrep-2209/runs/run-1779710592442-structured-current-policy-8bcdf6e0/1779711078002_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json`

The bundled trace audit was run read-only:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py /home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-clean-20260525-120225/BurntSushi__ripgrep-2209/runs/run-1779710592442-structured-current-policy-8bcdf6e0 --markdown
```

Audit summary: 78 provider responses, 77 provider-emitted tool calls, 77
recorded tool calls, 0 missing provider call ids, 0 extra recorded call ids, 77
`tool_calls` finishes, and one final `stop`. Recorded classifications were 21
`context_results`, 20 `read_with_content`, 15 `completed`, 11
`empty_completed_read`, 6 `transport_failure`, and 4 `duplicate_request`.

## Execution Path

Artifact-proved path:

```text
single agent eval
-> structured-current-policy treatment
-> run single agent / agent-single-turn
-> runner.rs::run_benchmark_turn
-> agent-turn trace and summary
-> validation audit
-> full response trace
-> final snapshot
-> Multi-SWE submission
-> benchmark patch projection
```

Evidence:

- `run.json` names task `BurntSushi__ripgrep-2209`, repo root
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`, base/head
  `4dc6c73c5a9203c5a8a89ce2161feca542329812`, model
  `google/gemini-3.5-flash`, and expected changed file
  `crates/printer/src/util.rs`.
- `execution-log.json` records `load_manifest`, `checkout_base_sha`,
  `bootstrap_headless_runtime`, `benchmark_turn_completed`,
  `write_validation_audit`, `persist_full_response_trace`,
  `snapshot_completed`, `write_msb_submission`, and
  `write_benchmark_patch_projection`.
- The run registration freezes `run_arm_id: structured-current-policy`,
  `command: run single agent`, `execution: agent-single-turn`, and marks
  execution, patching, packaging, and submission complete.
- `record.json.gz` is present as `run-record.v1` and contains setup,
  agent-turn, packaging, and timing phases. It is useful for persistence
  authority, but the trace review still required manual joins to
  `agent-turn-trace.json`, `llm-full-responses.jsonl`, and validation audit.

## Closure And Protocol State

The baseline eval completed. `closure-state.json` records eval status
`complete`, one expected instance, one complete eval, and no failed evals. At
the closure snapshot time, protocol was still `missing` for all three required
procedures.

While this review was running, the run registration was updated to protocol
`completed` and a protocol anchor appeared, but the protocol directory contained
only `1779711078002_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json`
when checked. That is later protocol output, not part of the baseline eval
execution trace reviewed here.

## Final Message Gap

`agent-turn-summary.json` has `terminal_record.outcome = completed`,
`error_id = null`, `attempts = 78`, and `final_assistant_message = null`.
That does not mean the model failed to produce a final answer. The raw response
ledger has response 77 with finish reason `stop` and 2,410 characters of final
assistant content, and `agent-turn-trace.json` records a final assistant
`MessageUpdated` at event 671 with the same content.

The durable interpretation is narrower: the summary field did not materialize
the final assistant message even though the terminal record and raw trace show
one. Treat this as a projection gap in `agent-turn-summary.json`, not as an
eval failure or proof that the final answer was absent.

## Trace Reconstruction

The main useful chain:

```text
initial issue/RAG snippets include util.rs look-ahead context
-> model reads util.rs and matcher replacement APIs
-> model localizes replacement bug to Replacer::replace_all
-> insert_rust_item adds a bounded helper in util.rs
-> same-file semantic edit fails stale verification
-> model reads live util.rs, switches to non_semantic_patch, and edits call site
-> cargo test exposes helper bug across replacement tests
-> model reads helper, removes erroneous subject[..at] append
-> cargo test passes
-> model adds a regression test, then weakens it after a failed look-ahead test
-> final cargo test passes
-> model emits final answer that overclaims the test as look-ahead coverage
```

Key events:

- Events 440-444: `code_item_lookup` resolves `crate::util::replace_all`, and
  `function-call-0f50750e-ae6a-472e-9c8c-dfa0a19681ad` inserts a helper into
  `crates/printer/src/util.rs`.
- Events 468 and 473-476: `function-call-9aaaaa0a-725f-4d28-9a88-a6744797a65c`
  tries `apply_code_edit` for `crate::util::Replacer::replace_all`; the tool
  rejects it because `util.rs` content changed and asks the model to refresh or
  re-resolve the target. This is the live stale same-file rejection in durable
  trace form.
- Events 478-488: the model obeys the recovery hint enough to read live
  `util.rs` ranges and verify the insertion.
- Events 529-537: a focused `cargo check` first resolves to
  `crates/globset/Cargo.toml` and does not cover the patch; the model then runs
  a package-intended check that resolves to the root `Cargo.toml` and covers
  `grep-printer`.
- Event 545: `cargo test` fails against the root manifest after the first helper
  version. The failure is not only the new test; existing replacement tests
  fail, including `replacement`, `replacement_passthru`,
  `replacement_per_match`, and max-columns replacement cases.
- Events 549-567: the model reads the helper and removes the erroneous
  `dst.extend_from_slice(&subject[..at])`. This is a good recovery chain:
  validation failure led to a targeted helper edit rather than random retry.
- Event 576: `cargo test` passes against
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml` after the
  production helper repair, before the new regression test exists.
- Events 627-633: `read_file` for `standard.rs` lines 3260-3320 returns
  `ok:true`, `exists:true`, and empty `content`. Direct verification with `sed`
  against the checkout shows those lines contain the newly added
  `replacement_multi_line_look_ahead` test, so this was a semantically bad
  successful read.
- Event 640: `code_item_lookup` for `replacement_multi_line_look_ahead` fails
  because the indexed semantic view did not know the newly inserted test.
- Events 643-653: the model weakens the test from `RegexMatcher::new(r"a(?=\nb)")`
  to `RegexMatcher::new(r"a")` and changes expected output from retaining `b`
  lines to just `1:x\n3:x\n`.
- Event 662: final `cargo test` passes with `scope: workspace`,
  manifest `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml`,
  exit code 0, 95 unit tests, and doc tests.
- Event 671 and response 77: the model final answer claims a fully tested
  look-ahead/look-around regression, but the final submitted test no longer
  uses look-around.

The last point where the model had enough information to act usefully was event
545: the failing replacement tests gave concrete validation feedback and the
subsequent helper edit was directionally correct. The later test-quality fork at
events 627-653 had enough information to preserve the issue shape from the
prompt and gold-like expected behavior, but the degraded read and lookup results
made the test-edit decision weaker instead of more faithful.

## Validation And Patch Output

`validation-audit.json` records seven cargo calls. The important distinction is
that the final validation is meaningful for compilation/regression sanity but
not sufficient to prove benchmark usefulness:

| Event | Call id | Command | Resolved scope | Manifest | Ok | Covers changed files |
| --- | --- | --- | --- | --- | --- | --- |
| 232 | `function-call-617dee64-ad4f-4439-af83-74d856d73d28` | `check` | workspace | root `Cargo.toml` | true | true |
| 529 | `function-call-d4983bd7-f9e3-4919-a2ad-c594c98b9edf` | `check` | focused | `crates/globset/Cargo.toml` | true | false |
| 537 | `function-call-30f56b08-8031-4346-84b8-eba866568d90` | `check` | workspace | root `Cargo.toml` | true | true |
| 545 | `function-call-84181572-24e0-443a-a6ae-54d759fb6f67` | `test` | workspace | root `Cargo.toml` | false | true |
| 576 | `function-call-c8574994-86c6-4b1b-8ad9-1687424794b0` | `test` | workspace | root `Cargo.toml` | true | true |
| 615 | `function-call-316db1de-977d-416b-9655-659a30bac56c` | `test` | workspace | root `Cargo.toml` | false | true |
| 662 | `function-call-fa3c7d42-fe1f-49c8-bd1c-8bf97bfde981` | `test` | workspace | root `Cargo.toml` | true | true |

The final cargo pass is therefore real: it runs against
`/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml`, compiles
`grep-printer`, and covers the changed files. It is not oracle-equivalent. The
validation audit also flags an expected-output/assertion edit candidate at event
644, and no formatting check was observed.

`benchmark-patch-projection.json` marks patch export `passed`, with a non-empty
5,129-byte submission. The final checkout diff changes two files:

- `crates/printer/src/util.rs`: production helper and call-site changes.
- `crates/printer/src/standard.rs`: local unit test addition.

## Patch Usefulness Against Gold

The production patch is close enough to be an adjudication candidate. Gold adds
`replace_with_captures_in_context`, passes `range.clone()`, stops captures when
`m.start() >= range.end`, and appends only up to `min(bytes.len(), range.end)`.
The generated patch adds `replace_with_captures_at`, computes `search_end` as
`range.end` for multiline and trimmed `m.end()` for single-line, stops when
`m.start() >= end`, and appends only through `end`.

That is the right algorithmic family. The main residual risks are:

- The generated helper manually reimplements the replacement loop with
  `captures_at` rather than using `captures_iter_at`, so its zero-width match
  behavior should be reviewed carefully against the matcher trait contract.
- It has no doc comment tying it to `find_iter_at_in_context` or the PCRE2
  look-ahead kludge, so the intent is less durable than gold.
- The test coverage is not benchmark-faithful. Gold adds `regression::r2095`
  and `regression::r2208` under `tests/regression.rs`; the generated test does
  not exercise `--replace`, `-U`, issue 2208's named capture, or real
  look-around.

Classification: promising production fix, weak benchmark proof. This should be
sent to a stronger adjudicator or oracle run, not counted as solved solely from
the final cargo pass.

## Tool Failures And Bad Successes

The stale same-file `util.rs` rejection is model-visible and recoverable. It
did not prevent the patch; the model fell back to live reads and
`non_semantic_patch`.

The `standard.rs` read/lookup failures are more concerning because they happened
during test repair:

- `request_code_context` for `pcre2 file:crates/printer/src/standard.rs`
  returned degraded context: 3 snippets returned, 7 stale snippets skipped.
- `read_file` for lines 3260-3320 returned `ok:true` with empty content even
  though direct `sed` showed live content in that range.
- `code_item_lookup` could not resolve the newly added test.

These bad successes and failures helped steer the model toward weakening the
test. They did not block final completion, but they reduced benchmark usefulness.

The exact live stdout string `TOOL_EXECUTION_FAILED` was not found as a durable
string in the checked run files. Durable equivalents are present as
`ToolFailed` events and `ok:false` tool messages, while terminal accounting
still reports completed.

## Positive Examples And Adjudication Candidates

- Good localization signal: initial snippets and `MAX_LOOK_AHEAD` search led to
  `util.rs::Replacer::replace_all`, not an unrelated printer surface.
- Good recovery signal: failed `cargo test` at event 545 led to a targeted
  helper edit at events 549-567 and a passing package test at event 576.
- Good final validation signal: the final cargo test is a real root-manifest
  run covering changed files.
- Adjudication candidate: production diff shape is close to gold despite weak
  tests. A future reviewer should evaluate the manual `captures_at` loop against
  `Matcher::replace_with_captures_at` semantics, especially empty matches.
- Negative adjudication signal: final answer claimed look-ahead/look-around
  regression coverage after replacing the look-ahead regex with plain `a`.

## What Is Working

- The eval path persisted enough raw evidence to reconstruct provider calls,
  tool lifecycle, edits, validation, patch export, and checkout diff.
- The model used validation feedback productively for the production helper.
- The final validation audit correctly distinguishes the focused `globset`
  cargo check from root-manifest cargo checks/tests and flags the test
  expected-output edit.
- Stale same-file edit failure was visible to the model, and the model recovered
  through direct reads and patch tools.

## What Is Not Working Yet

- `agent-turn-summary.json` loses the final assistant message even when the raw
  response and trace contain it.
- `read_file` can return `ok:true` with empty content for an existing non-empty
  range, which makes successful reads unsafe without spot verification.
- Semantic lookup/index state did not include newly inserted test code during
  the same turn.
- Protocol status can drift after eval closure; review artifacts must continue
  separating baseline eval from later protocol projections.
- Final response quality is not audited against the actual final patch: it
  claimed formatting-free, fully tested look-around coverage without a formatting
  check and after weakening the test.

## Action Items

- Add an artifact-level check for `final_assistant_message = null` when
  `llm-full-responses.jsonl` contains a final `stop` response, so summaries do
  not imply absent final content.
- Treat `read_file ok:true content:""` for an existing in-range file slice as a
  tool-quality failure or at least emit a warning field that protocol can score.
- Add an adjudication field for "test was weakened after validation failure" and
  connect it to validation-audit expected-output edit candidates.
- Add a protocol/review field that compares final-answer claims against observed
  tool evidence: cargo command, manifest, formatting check, and whether the test
  still exercises the issue mechanism.
- For this candidate patch, run an oracle or stronger adjudicator focused on
  `replace_with_captures_at` empty-match semantics and the missing gold
  regression tests before counting it as benchmark solved.
