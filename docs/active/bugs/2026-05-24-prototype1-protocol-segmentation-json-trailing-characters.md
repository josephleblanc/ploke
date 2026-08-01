# Prototype 1 Protocol Segmentation JSON Trailing Characters

Status: fixed in source checkout; focused regression verified; live campaign
resume pending.

## Summary

The Gemini 3.5 Flash Prototype 1 campaign
`p1-gemini35-flash-multigen-2g3x3-20260523-223658` is blocked in
`baseline_protocol` because `tool_call_intent_segmentation` received a complete
JSON object followed by an extra top-level closing brace. The protocol parser
reported `failed to parse json response: trailing characters at line 62 column
1`, no protocol artifact was persisted, and closure still reports the protocol
stage as missing.

This is distinct from the older truncated-JSON blocker in
`2026-05-22-prototype1-protocol-segmentation-truncated-json.md`: the new failure
is not EOF or budget starvation. The model returned usable segmentation content
plus trailing syntax.

## Evidence

Campaign:

```text
p1-gemini35-flash-multigen-2g3x3-20260523-223658
```

Worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658
```

Operator report:

```text
.orchestrator/reports/loop-operator/2026-05-24T10-48-07Z-prototype1-step.md
```

The operator ran one bounded current-binary step:

```text
cargo run -p ploke-eval -- loop prototype1-step \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658 \
  --format json
```

Observed result:

```text
baseline_protocol blocked
segmentations_created = 0
call_reviews_created = 0
segment_reviews_created = 0
phase after doctor = baseline_protocol
closure protocol = missing
```

Relevant log:

```text
/home/brasides/.ploke-eval/logs/ploke_eval_20260524_034654_954818.log
```

The provider request succeeded with HTTP 200 and the response contained a
complete `segments` array and `overall_rationale`, but `message.content` ended
with an extra top-level `}` after the root object. The parser surfaced this as:

```text
failed to parse json response: trailing characters at line 62 column 1
```

## Broken Contract

The protocol phase should not permanently block on a syntactically recoverable
single-response formatting error when the model returned a complete adjudication
object. Either the protocol JSON parser should recover a single valid root
object prefix when safe, or the segmentation path should classify this as
retryable and produce a bounded retry/repair attempt.

## Reproduction Surface

Use a protocol-level regression. The narrow reproduction can be a parser test
using a valid segmentation-shaped JSON object followed by one extra top-level
closing brace. If the repair is implemented as retry classification instead,
add the regression beside
`intent_segmentation_truncated_json_parse_is_retryable`.

Suggested minimal shape:

```text
{"segments":[{"start_index":0,"end_index":0,"status":"labeled","label":"locate_target","confidence":"high","rationale":"x"}],"overall_rationale":"ok"}}
```

## Fix Direction

Prefer the smallest authority-bearing fix:

- if parser recovery is chosen, only accept a single complete JSON value prefix
  when the trailing content is ignorable or a clearly redundant brace;
- if retry classification is chosen, keep it scoped to
  `tool_call_intent_segmentation` and avoid treating arbitrary malformed content
  as success.

## Fix

Implemented at the protocol parser boundary in
`crates/ploke-protocol/src/llm.rs`.

`parse_protocol_json_content` now accepts a complete root JSON object prefix
only when the remaining suffix is non-empty and contains only redundant closing
braces plus whitespace. The parser still rejects arbitrary trailing text and
non-object roots with trailing braces, and continues to preserve the existing
alias and truncated-final-rationale recovery paths.

Added regression coverage for:

- complete segmentation-shaped JSON followed by an extra top-level `}`;
- valid segmentation-shaped JSON followed by non-brace trailing text, which
  remains a parse failure.
- array-root JSON followed by a trailing brace, which remains a parse failure.

## Verification

Verified:

```text
cargo test -p ploke-protocol parse_protocol_json_content_ -- --nocapture
```

Result: passed, 6 tests.

Orchestrator re-verified the final narrowed patch:

```text
cargo test -p ploke-protocol parse_protocol_json_content_ -- --nocapture
cargo test -p ploke-protocol
cargo fmt --all
```

Result: focused parser tests passed with 7 tests; crate tests passed with 19
tests plus doc tests; formatting completed.

Pending live resume:

```text
cargo run -p ploke-eval -- loop prototype1-step \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-multigen-2g3x3-20260523-223658 \
  --format json
```

Do not use the stale worktree-local `./target/debug/ploke-eval`; run from the
current source checkout so the repaired protocol parser is used.
