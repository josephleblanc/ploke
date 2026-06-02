# 2026-05-24 p1-gemini35-flash Protocol Follow-up Review

Status: reviewed after the `baseline_protocol` step failure.

Campaign: `p1-gemini35-flash-multigen-2g3x3-20260523-223658`

Instance: `BurntSushi__ripgrep-2209`

Run root:
`/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658/BurntSushi__ripgrep-2209/runs/run-1779602498508-structured-current-policy-ed4a409a`

## Short Verdict

The eval produced a real benchmark submission and a non-empty patch projection,
but the campaign cannot advance because protocol adjudication is blocked before
persisting any protocol artifacts.

The run is therefore mechanically useful as eval evidence and LLM-adjudication
training material, but it is not a complete Prototype 1 candidate. The current
campaign blocker is the filed protocol segmentation parse failure:
`docs/active/bugs/2026-05-24-prototype1-protocol-segmentation-json-trailing-characters.md`.

## Evidence Roots

- Operator report:
  `.orchestrator/reports/loop-operator/2026-05-24T10-48-07Z-prototype1-step.md`
- Campaign manifest:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-multigen-2g3x3-20260523-223658/campaign.json`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-multigen-2g3x3-20260523-223658/closure-state.json`
- Run profile:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-multigen-2g3x3-20260523-223658/prototype1/run-profile.toml`
- Eval run:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658/BurntSushi__ripgrep-2209/runs/run-1779602498508-structured-current-policy-ed4a409a`
- Record:
  `record.json.gz` in the run root
- Provider trace:
  `llm-full-responses.jsonl` in the run root
- Model/tool trace:
  `agent-turn-trace.json` in the run root
- Submission:
  `multi-swe-bench-submission.jsonl` in the run root
- Patch projection:
  `benchmark-patch-projection.json` in the run root

## Closure State

`cargo run -p ploke-eval -- closure status --campaign p1-gemini35-flash-multigen-2g3x3-20260523-223658 --format json`
reported:

- campaign model/provider: `google/gemini-3.5-flash` via
  `google-ai-studio`
- route source: `open_router`
- registry: `complete`, 1 of 1 mapped
- eval: `complete`, 1 complete and 0 failed
- protocol: `missing`, 1 missing and 0 complete
- required procedures still missing:
  `tool-call-intent-segments`, `tool-call-review`,
  `tool-call-segment-review`

The protocol artifact directory is absent at:
`/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658`.

## Eval And Patch Output

The eval produced a submission and patch:

- `multi-swe-bench-submission.jsonl`: 4254 bytes
- `benchmark-patch-projection.json`: 1585 bytes
- patch projection status: `passed`
- projection detail: `fix_patch exported from the recorded checkout cwd`
- diff base: `4dc6c73c5a9203c5a8a89ce2161feca542329812`

The submitted patch changes:

- `crates/printer/src/util.rs`
- `crates/printer/src/standard.rs`

The patch is benchmark-relevant: it changes replacement behavior in
`Replacer::replace_all` and adds
`standard::tests::replacement_lookahead_bug_2208`.

It is not hygiene-clean. Direct review commands found:

- `git diff --check`: passed
- `cargo fmt -- --check`: failed because `pub fn replace_all` is indented one
  level too far in `crates/printer/src/util.rs`

## Oracle And MBE State

The run profile has:

- `[execution.mbe] enabled = false`
- `[selection.oracle] mode = "record-only"`
- `[selection.oracle] require_evidence = true`

So there is no MBE oracle execution evidence for this candidate. That is a
profile choice, not the current blocker.

## LLM And Tool Behavior

The bundled trace audit ran successfully:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py \
  <run-root> --markdown
```

Audit summary:

- provider responses: 74
- provider-emitted tool calls: 73
- recorded tool calls: 73
- missing recorded provider call ids: 0
- extra recorded call ids: 0
- finish reasons: 73 `tool_calls`, 1 `stop`
- recorded classifications:
  - `completed`: 12
  - `context_results`: 20
  - `read_with_content`: 21
  - `empty_completed_read`: 6
  - `duplicate_request`: 13
  - `transport_failure`: 1

Do not read the audit counts as semantic success. The high-signal chain is in
`agent-turn-trace.json`, where tool payloads were visible in `MessageUpdated`
events and the next assistant messages reacted to them.

## Cargo Feedback Visibility

Cargo output was model-visible and was used.

Positive examples:

1. Event 362/365: `cargo test -p grep-printer` returned `exit_code: 101`.
   Event 368 shows the model quoting the failing test
   `standard::tests::replacement_per_match`, the panic location
   `crates/printer/src/util.rs:104:32`, and the out-of-range slice error. The
   next action was a targeted read of `util.rs`.
2. Event 548/553: a later `cargo test -p grep-printer` still failed. Event 554
   shows the model using the new panic from
   `replacement_max_columns_preview2` and reasoning about why the previous
   clamp was insufficient.
3. Event 587/591: after a `non_semantic_patch`, cargo returned `ok: true` and
   `exit_code: 0` for 94 tests. Event 592 shows the model recognizing that
   the focused tests passed, then deciding to add a regression test.
4. Event 618/623: final `cargo test -p grep-printer` returned `ok: true` with
   `running 95 tests`. Event 624 shows the model recognized that the new
   `replacement_lookahead_bug_2208` test loaded and passed.

These are good adjudication candidates: the chain is not merely
`ToolCompleted`; it is failed command output, model interpretation, targeted
repair, and later passing validation.

## Trace Reconstruction

The initial RAG context was useful. It surfaced the replacement tests in
`standard.rs` and the `find_iter_at_in_context` helper in `util.rs`, which are
near the issue.

The model then spent a large number of turns on mixed-quality navigation:

- useful reads of `crates/printer/src/util.rs`
- useful reads of `crates/printer/src/standard.rs`
- repeated and sometimes stale `request_code_context` calls
- several empty completed reads for high line ranges
- an early `cargo check` that resolved to the focused `globset` manifest,
  which was not a meaningful validation for the target patch

The main productive turning point was the first failing `grep-printer` test
after an edit. From there the model used concrete panic details to repair the
implementation. It also recovered from `insert_rust_item` failing by using
`non_semantic_patch` to add the regression test.

The final validation was partially strong and partially weak:

- strong: `cargo test -p grep-printer` passed and included the new test
- weak: the final `cargo check` again resolved to `crates/globset/Cargo.toml`
- missing: the model did not run or see `cargo fmt -- --check`

## Protocol Review And Blind Spots

Protocol did not complete, so there is no protocol judgment to compare against
the trace.

The operator report and existing bug identify the live blocker: the
`tool_call_intent_segmentation` response contained a complete JSON object plus
an extra top-level closing brace. The parser reported:

```text
failed to parse json response: trailing characters at line 62 column 1
```

No segmentations, call reviews, or segment reviews were persisted.

The trace audit also exposes a blind spot for future protocol review: cargo
tool rows can show `content_len = 0` even when `MessageUpdated:tool` carries
the model-visible JSON with exit code, stdout/stderr tails, and test names.
Protocol review should key off model-visible tool payloads and subsequent
assistant behavior, not only the compact audit table.

## What Is Working

- The eval completed and packaged a non-empty Multi-SWE-Bench submission.
- The model used tool feedback to recover from test failures.
- The model added a relevant regression test and reached a passing focused
  test state.
- Tool lifecycle capture matched provider tool call ids: 73 emitted and 73
  recorded.
- The existing trace audit script is useful for inventory and suspicious-call
  triage.

## What Is Not Working Yet

- Protocol adjudication is blocked before artifacts are persisted.
- The submitted patch is not rustfmt-clean.
- The run is token-heavy: 74 provider responses for one eval.
- Empty/truncated reads and duplicate context requests were mechanically
  successful but not always information-successful.
- Final validation included a misleading `cargo check` against `globset`.
- The final packaging says patch projection passed, but that only proves a
  submission was exported from the recorded checkout. It does not prove
  formatting, oracle success, or protocol success.

## Action Items

Blocker:

1. Repair or retry-route the protocol segmentation trailing-character parse
   failure tracked in
   `docs/active/bugs/2026-05-24-prototype1-protocol-segmentation-json-trailing-characters.md`.
   Do not advance this campaign with another `prototype1-step` until that is
   handled.

Non-blockers for loop advancement after the protocol blocker is fixed:

1. Add an adjudication field for
   `failed cargo output -> model quotes/uses failure -> targeted repair ->
   later passing validation`.
2. Add an adjudication field for final validation strength, distinguishing
   focused package tests from misleading focused-manifest checks.
3. Flag `cargo fmt -- --check` absence or failure as patch-hygiene evidence.
4. Treat empty completed reads as suspicious information failures, especially
   when `truncated = true`.
5. Improve trace summaries so cargo payloads with model-visible JSON are not
   summarized as empty-content successes.
6. Consider surfacing the `insert_rust_item` failure followed by successful
   `non_semantic_patch` fallback as a positive recovery example.

## Verification Commands

- `python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py <run-root> --markdown`
  - succeeded; produced the 74-response / 73-tool-call audit above
- `cargo run -p ploke-eval -- closure status --campaign p1-gemini35-flash-multigen-2g3x3-20260523-223658 --format json`
  - succeeded; eval complete and protocol missing
- `jq '.' <run-root>/benchmark-patch-projection.json`
  - succeeded; projection status `passed`
- `wc -c <run-root>/multi-swe-bench-submission.jsonl <run-root>/benchmark-patch-projection.json`
  - succeeded; submission 4254 bytes, projection 1585 bytes
- `test -d /home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658 && find ... || printf 'absent\n'`
  - reported `absent`
- `git -C /home/brasides/.ploke-eval/repos/BurntSushi/ripgrep diff --check`
  - passed
- `cargo fmt -- --check`
  - failed in the target checkout on `crates/printer/src/util.rs` indentation
