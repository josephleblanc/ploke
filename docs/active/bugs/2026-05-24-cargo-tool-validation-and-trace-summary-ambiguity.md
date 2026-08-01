# Cargo Tool Validation And Trace Summary Ambiguity

## Summary

The Gemini 3.5 Flash baseline eval showed that cargo feedback can be genuinely
useful to the model, but current review surfaces make that hard to see and can
overstate the final validation state.

Two issues are coupled:

1. the trace audit reports `content_len = 0` for `cargo` completions even when
   the model-facing `MessageUpdated:tool` events contain rich JSON payloads with
   stdout/stderr tails, failing test names, and exit status;
2. the final `cargo check` in the run resolved to the focused `globset`
   manifest, not to a meaningful whole-workspace or target-crate validation
   surface, and the model never ran `cargo fmt -- --check`, so it did not see
   the final rustfmt failure.

This can make run review and LLM adjudication miss both sides of the story: the
model did use relevant cargo output during repair, but the final validation
claim was still weaker than the assistant's final response implied.

## Affected Surface

- `ploke-eval` agent-turn trace summaries and run-review tooling
- `docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py`
- Prototype 1 LLM adjudication prompts/reports that inspect tool use
- cargo tool working-directory / manifest-resolution reporting

## Concrete Evidence

Campaign:

```text
p1-gemini35-flash-multigen-2g3x3-20260523-223658
```

Run root:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-multigen-2g3x3-20260523-223658/BurntSushi__ripgrep-2209/runs/run-1779602498508-structured-current-policy-ed4a409a
```

Useful cargo feedback did reach the model:

- `cargo` call `5yobijls` failed with `exit_code: 101`. The model-facing tool
  message included `range end index 65 out of range for slice of length 64` at
  `crates/printer/src/util.rs:104:32`. The next assistant message quoted that
  panic and read the relevant `util.rs` lines.
- Later `cargo` failure output included `range end index 321 out of range for
  slice of length 320`. The model quoted that detail and changed its repair
  strategy.
- Later `cargo test -p grep-printer` calls succeeded. The final successful test
  payload included `running 95 tests` and the newly added
  `replacement_lookahead_bug_2208` test.

The trace audit projection hides this:

- `run_trace_audit.py` reports `content_len = 0` for recorded `cargo`
  completions.
- The raw `agent-turn-trace.json` has `MessageUpdated:tool` events with
  thousands of bytes of model-facing cargo result JSON.

The final validation claim was too strong:

- The last `cargo check` completed successfully, but the recorded manifest was
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml`.
- Direct review later found `cargo fmt -- --check` fails because
  `crates/printer/src/util.rs` has an over-indented `pub fn replace_all`.
- The model never saw that formatting failure because no rustfmt check was run
  during the eval.

## Why It Matters

For hill-climbing runs, the adjudicator needs to distinguish:

- a model that ignored validation output,
- a model that used validation output correctly,
- a model that reached only a weak or misleading validation surface, and
- a trace-summary bug that hides model-visible evidence.

This run is a positive example of cargo-driven repair, but the existing summary
surfaces make that evidence too hard to recover. They also let a final
assistant response overclaim "perfect form" after a weak focused check and no
formatting check.

## Expected Behavior

Run review and LLM adjudication should make cargo evidence visible as a linked
chain:

1. cargo command requested;
2. model-facing tool payload, including exit code, failing tests, and short
   stdout/stderr tail;
3. next assistant response that quotes, summarizes, or ignores that payload;
4. follow-up edit/read action;
5. later validation result.

The cargo tool summary should also flag weak validation surfaces, including
focused-manifest checks that resolve outside the changed crate and missing
format checks when patch hygiene matters.

## Suggested Regression/Tooling Work

- Update `run_trace_audit.py` to report model-facing `MessageUpdated:tool`
  payload length and a short cargo result preview, not only the recorded
  `ToolCompleted` content length.
- Add an LLM-adjudication section that asks for concrete examples of
  `tool output -> model interpretation -> follow-up action -> later result`.
- Mark cargo checks against unrelated focused manifests as weak validation when
  the final patch touches different files.
- Add an optional eval gate or report field for `cargo fmt -- --check`, or make
  the absence of a formatting check explicit in candidate summaries.

## 2026-05-25 Recurrence

The same validation weakness recurred in direct-Google campaign
`p1-gemini35-flash-direct-fresh-20260524-190632`.

Run root:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-190632/BurntSushi__ripgrep-2209/runs/run-1779674853833-structured-current-policy-2897a178
```

New evidence:

- The model generated a non-empty patch and `cargo test -p grep-printer`
  passed with `95` unit tests and `2` doc tests.
- Direct `cargo fmt -- --check` failed on an over-indented
  `pub fn replace_all` line in `crates/printer/src/util.rs`.
- The final `cargo check` tool call again reported success against
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml`,
  unrelated to the changed `crates/printer` files.
- `llm-full-responses.jsonl` contains a final assistant response, but
  `agent-turn-summary.json` reports `final_assistant_message = null`.

This reinforces that adjudication should treat target tests, final check scope,
formatting checks, and summary parity as separate fields.

## 2026-06-06 Recurrence

The latest 090815 baseline/protocol run repeats the same validation and review-surface ambiguity, with stronger protocol-correlation evidence.

Campaign:

```text
p1-admissionfix-g35flash-p25flash-20260606-090815
```

Run root:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0
```

Source report:

```text
docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-baseline-protocol-tool-correlation.md
```

New evidence:

- Trace audit: 53 provider responses, 52 provider-emitted tool calls, 52 recorded tool calls, and zero missing call ids.
- Cargo calls `[24]`, `[29]`, and `[47]` were lifecycle `ToolCompleted` but semantically failed (`result.ok=false` / compile or test failure). Protocol/reporting must inspect `ok` and `status_reason`, not only tool lifecycle completion.
- Call `[50]` was a successful `cargo test` but validation audit records `manifest_path=/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml` and `covers_changed_files=false`; it is not broad regression coverage for the changed `crates/printer` files.
- The validation audit's `patch_quality.expected_output_edit_candidates` flags call `function-call-ce5d18bc-f244-434d-81d0-9da71278b162` changing `let expected = "1:x\n2-b\n";` to `let expected = "1:x\n";` in `crates/printer/src/standard.rs` after a failing test.
- Direct submission-patch verification confirmed the exported patch contains `let expected = "1:x\n";`, does not contain the earlier `"1:x\n2-b\n"` expectation, and contains the over-indented `        pub fn replace_all` line.
- `llm-full-responses.jsonl` has the stop/final response, but both `agent-turn-summary.json` and `record.json.gz` have `final_assistant_message = null` for the turn artifact. Final-answer playback still requires a manual join to the full-response sidecar.

This recurrence keeps the bug open as a protocol/read-side issue: the raw run is reviewable, but cargo semantic status, changed-file coverage, expected-output edits, patch hygiene, and final-assistant capture must be first-class fields before protocol completion can be treated as a reliable semantic-success signal.

## Related Reports

- [`2026-05-22-cargo-tool-tail-rendering-and-timeout.md`](./2026-05-22-cargo-tool-tail-rendering-and-timeout.md)
- [`2026-05-24-request-code-context-silent-stale-snippet-skip.md`](./2026-05-24-request-code-context-silent-stale-snippet-skip.md)
