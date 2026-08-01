# Prototype 1 Baseline Eval + Protocol Review: p1-gemini35-flash-direct-15g2x3-20260525-140904

Date: 2026-05-25
Campaign: `p1-gemini35-flash-direct-15g2x3-20260525-140904`
Task: `BurntSushi__ripgrep-2209`
Run: `run-1779743381178-structured-current-policy-2c1daae7`
Parent node after protocol: `node-18f71c7f3b1718b8`
Model route: `google/gemini-3.5-flash` through direct Google

## Verdict

The baseline eval produced a non-empty patch and a Multi-SWE-bench submission.
The model found the relevant `grep-printer` replacement path, made a production
edit in `crates/printer/src/util.rs`, added one regression-style test, and saw
successful cargo outputs before its final answer. The final answer still
overstated the validation evidence: the last recorded `cargo check` resolved to
the `globset` crate and did not cover the changed file.

The baseline protocol completed mechanically. Closure reports one completed
segmentation procedure, 66 completed tool-call reviews, and 12 completed segment
reviews. That should not be read as strong behavioral validation. The protocol
missed the final cargo-scope mismatch because the reviewed packet previews did
not preserve the manifest/scope detail that `validation-audit.json` recorded.

After protocol, the parent advanced into child-plan handling and published a
broad harness request, but no durable child result was present in the checked
records. The fresher node record says `running`; `scheduler.json` still lists
the frontier as `planned`.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-140904`
- Eval run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-20260525-140904/BurntSushi__ripgrep-2209/runs/run-1779743381178-structured-current-policy-2c1daae7`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-15g2x3-20260525-140904/BurntSushi__ripgrep-2209/runs/run-1779743381178-structured-current-policy-2c1daae7`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-140904/closure-state.json`
- Transition evidence:
  `/home/brasides/.ploke-eval/logs/ploke_eval_20260525_142753_2.log`

Checked eval artifacts included `record.json.gz`, `agent-turn-trace.json`,
`agent-turn-summary.json`, `llm-full-responses.jsonl`,
`validation-audit.json`, `benchmark-patch-projection.json`, and
`multi-swe-bench-submission.jsonl`. Checked protocol artifacts included the
segmentation artifact, all tool-call review artifacts, and all segment-review
artifacts. Checked transition/channel surfaces found edit-harness request files
for `node-18f71c7f3b1718b8`, but no `runner-result.json`, no
`edit-harness-result` directory, and no transition/channel JSONL record for the
child-plan phase.

## Closure And Trace Chain

`closure-state.json` reports eval status `complete` with one expected instance,
one complete instance, and no failed, missing, partial, or in-progress eval
instances. It also reports protocol status `complete` with one expected
protocol instance and one full protocol instance.

The run trace audit matched provider and persisted tool ledgers:

- LLM responses: 67.
- Provider-emitted tool calls: 66.
- Recorded tool calls: 66.
- Missing provider call IDs: 0.
- Extra recorded call IDs: 0.
- Finish reasons: 66 `tool_calls`, 1 `stop`.
- Tool classifications: 26 `context_results`, 14 `completed`,
  10 `read_with_content`, 10 `empty_completed_read`, 4 `duplicate_request`,
  and 2 `transport_failure`.

The eval chain was a single long turn. The model searched and read through
`grep-printer` replacement code, ran baseline cargo, edited `util.rs`, validated
the production edit, failed an attempted test insertion into `standard.rs`,
recovered by adding a test module to `util.rs`, ran `cargo test --package
grep-printer`, then ran a final bare `cargo check`.

## Patch And Submission

The eval produced a patch and a submission.

`agent-turn-summary.json` reports two applied edit proposals, both targeting
`/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs`.
The patch artifact reports `applied: true`, `all_proposals_applied: true`,
expected changed path `crates/printer/src/util.rs`, before hash
`9e637...`, after hash `8c794...`, and `changed: true`.

`benchmark-patch-projection.json` reports:

- `check.status`: `passed`.
- `fix_patch` exported from the recorded checkout cwd.
- Submission path: `multi-swe-bench-submission.jsonl`.
- Submission SHA-256:
  `078598193d8ef780cf8cdaf299125ba17805fcb99b8da11ad6e4ca7dd3c93fbf`.
- Submission byte length: 4144.
- Submission line count: 123.
- Diff base:
  `4dc6c73c5a9203c5a8a89ce2161feca542329812`.

The exported diff is one file: `crates/printer/src/util.rs`. It replaces the
`Matcher::replace_with_captures_at` path in `Replacer::replace_all` with a
manual loop over `captures_at`, stops when a match starts at or after
`range.end`, appends the suffix from `range.end`, and adds
`util::tests::replacement_multi_line_look_around_duplication`.

The patch is directionally relevant, but the proof is weak for benchmark
adjudication. The added test uses a newline regex over `"a\nb\nc\n"` and expects
`"1:aRbRcR\n"`. It does not directly encode the issue's PCRE look-around or
capture-shape behavior.

## Tool Output Visibility

Cargo and tool outputs did reach the model. The trace records tool messages
being added after cargo completions, including the final `cargo test --package
grep-printer` payload showing `running 95 tests` and the new util test passing.
The final assistant message was generated after the final bare `cargo check`
tool message.

The model nevertheless compressed the validation story incorrectly. Its final
answer cited `cargo test --package grep-printer` and claimed all 95 tests
passed, which is true for the package test. It did not acknowledge that the
last recorded `cargo check` was not the same validation surface.

## Validation Evidence

`validation-audit.json` is the decisive cargo record:

- Changed path: `crates/printer/src/util.rs`.
- Production changed path count: 1.
- Test changed path count: 0.
- Edit request count: 3.
- Test edit request count: 2.
- Production edit request count: 1.
- `successful_cargo_covering_changed_files`: true.
- `final_cargo_covers_changed_files`: false.
- `fmt_check_observed`: false.

Recorded cargo calls:

| Event | Command intent | Resolved scope | Result | Covers changed file |
| --- | --- | --- | --- | --- |
| 72 | `cargo check`, package `grep-printer` | workspace root | ok | yes |
| 336 | `cargo test`, package `grep-printer` | workspace root | ok | yes |
| 360 | `cargo check`, package `grep-printer` | workspace root | ok | yes |
| 368 | `cargo test`, package `grep-printer` | workspace root | ok | yes |
| 544 | `cargo test`, package `grep-printer` | workspace root | ok, 95 tests | yes |
| 553 | bare `cargo check` | focused `crates/globset/Cargo.toml` | ok | no |

The audit warnings are adjudication-relevant:

- Final cargo check does not cover the changed path.
- No formatting check evidence was recorded.
- Edit requests were mostly test-scoped, 2 of 3, even though the patch contains
  a production edit.

An independent `git diff --check` against the eval checkout produced no
whitespace-error output, but that is not a substitute for recorded rustfmt or
changed-path cargo coverage.

## Protocol Review

Protocol closure reports all required stages complete:

- `tool-call-intent-segments`: 1 complete artifact.
- `tool-call-review`: 66 complete artifacts.
- `tool-call-segment-review`: 12 complete artifacts.

The segmentation artifact covers 66 of 66 calls:

- Total calls: 66.
- Labeled calls: 66.
- Uncovered calls: 0.
- Ambiguous calls: 0.
- Segments: 12.
- Search calls: 27.
- Read calls: 20.
- Browse calls: 3.
- Edit calls: 1.
- Failed calls: 2.

Protocol review verdict counts across the 78 review artifacts:

- Usefulness: 60 `key_progress`, 14 `helpful_but_non_essential`,
  2 `low_value`, 2 `no_value`.
- Redundancy: 69 `distinct`, 7 `overlapping`, 2 `search_thrash`.
- Recoverability: 67 `no_recovery_needed`, 11 `clear_next_step`.
- Overall confidence: 64 `high`, 14 `medium`.

The most important protocol blind spot is segment 11, covering calls 64 and 65:
final `cargo test --package grep-printer` and final bare `cargo check`. Its
segment review marked the segment as `key_progress`, `distinct`,
`no_recovery_needed`, and `focused_progress`. That is mechanically reasonable
from the packet preview, but it missed the validation-audit fact that call 65
resolved to `crates/globset/Cargo.toml` and did not cover `util.rs`.

For future adjudication, protocol packets need manifest, resolved scope, and
changed-file coverage fields when reviewing cargo calls. The current review can
credit successful model-visible package testing while still penalizing final
validation mismatch.

## Malformed JSON And Retry Evidence

No protocol artifact or protocol log contained a durable provider retry record
for malformed JSON. The protocol log contained 250 provider attempts, all HTTP
200, all with `retry_decision: none`.

Strict JSON parsing of persisted protocol `provenance.raw_content` strings
found two malformed model outputs that were nevertheless normalized into usable
artifact `output` fields:

1. `1779744242800_tool_call_review_BurntSushi__ripgrep-2209.json`
   - Focal item: `call:30`.
   - Tool: `request_code_context`.
   - Search term: `replacer.replace_all`.
   - Branch: redundancy review raw content.
   - Parse error: `Expecting ',' delimiter`.
   - Shape: valid-looking object followed by a stray punctuation fragment.
   - Normalized result: redundancy `distinct`, confidence `high`.
2. `1779744243166_tool_call_review_BurntSushi__ripgrep-2209.json`
   - Focal item: `call:35`.
   - Tool: `request_code_context`.
   - Search term: `crates/printer/src/standard.rs from_sink_match`.
   - Branch: recoverability review raw content.
   - Parse error: `Expecting ',' delimiter`.
   - Shape: valid-looking object followed by a stray punctuation fragment.
   - Normalized result: recoverability `no_recovery_needed`, confidence `high`.

This should be tracked as local malformed-output repair, not provider retry.
The artifact layer currently hides the repair unless raw provenance is audited.

## Model-Performance Stats

Eval LLM response usage from `llm-full-responses.jsonl`:

- Responses: 67.
- Model: `google/gemini-3.5-flash` for all responses.
- Finish reasons: 66 `tool_calls`, 1 `stop`.
- Prompt tokens, summed: 5,223,005.
- Completion tokens, summed: 5,863.
- Total tokens, summed: 5,255,817.
- Max prompt tokens in one response: 147,547.

Eval provider-attempt stats from the eval log:

- Provider attempts: 67.
- HTTP status: 200 for all attempts.
- Retry decisions: none.
- Elapsed time: min 1199 ms, max 12834 ms, average about 3450 ms.
- Response bytes, summed: 194,814.

Protocol provider-attempt stats from the protocol log:

- Provider attempts: 250.
- HTTP status: 200 for all attempts.
- Retry decisions: none.
- Elapsed time: min 2108 ms, max 23368 ms, average about 4830 ms.

Trace `PerformanceMetrics` entries persisted usage data, but
`tokens_per_second`, `time_to_first_token`, and `queue_time` were zero-valued
placeholders. Do not treat those fields as measured latency/throughput.

## Child-Plan And Transition Records

The post-protocol state did advance toward child planning. The child-plan log
records:

- Parent runtime identity loaded for
  `node-18f71c7f3b1718b8`.
- Parent startup validation admitted:
  `Parent<Checked>->Parent<Ready>`.
- Child-plan authority resolution:
  `Parent<Ready>->ChildPlan`.
- Broad harness request published:
  `Parent<Ready>->Parent<AwaitingHarnessPlan>`.
- Request ID:
  `broad-harness-request:node-18f71c7f3b1718b8`.
- Request hash:
  `fa983...`.

The same log then records repeated broad headless-TUI slot attempts, r0 through
r9, failing during worktree preparation with `git worktree add -b ...` status
255 and `admitted=0 required_min=2 configured_max=3`.

Durable child result records were not present in the checked surfaces. The node
directory contains `node.json`, `runner-request.json`, and `bin/`, but no
`runner-result.json`. The campaign contains edit-harness request JSON/Markdown
files for `node-18f71c7f3b1718b8` and retries r2 through r9, but no
`edit-harness-result` record. No transition journal or channel JSONL file was
found under the campaign prototype1 tree.

## Red Flags For Future Adjudication Fields

- `patch_exported`: true, but separate from behavioral correctness.
- `submission_non_empty`: true, with one-file `util.rs` diff.
- `issue_shaped_test_present`: weak or false; the new test does not reproduce
  the PCRE look-around shape directly.
- `model_visible_tool_outputs`: true; cargo outputs reached the model.
- `successful_cargo_covering_changed_files`: true; earlier package cargo
  covered `util.rs`.
- `final_cargo_covers_changed_files`: false; final check resolved to
  `globset`.
- `fmt_check_observed`: false.
- `final_claim_vs_tool_record_mismatch`: true; final answer omits the final
  cargo-scope mismatch.
- `protocol_completed`: true; segmentation/review completion is mechanically
  correct.
- `protocol_detected_validation_scope_mismatch`: false; segment review lacked
  cargo manifest/scope detail.
- `malformed_json_repair_count`: 2; repaired protocol raw outputs should be
  surfaced explicitly.
- `provider_retry_count_for_json`: 0 in durable logs.
- `tool_failure_recovered`: true; failed same-file/test insertion attempts did
  not stop the turn.
- `child_plan_result_present`: false for the checked child-plan phase.
- `scheduler_projection_stale`: likely; node record says `running`, scheduler
  frontier still says `planned`.
