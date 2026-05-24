# Prototype 1 Protocol Review Malformed JSON Retry

Status: fixed in source checkout; focused regression verified; live campaign
resume pending.

## Summary

Campaign `p1-gemini35-flash-direct-profile-20260524-151353` reached
`baseline_protocol` after a successful eval step, but `protocol_tool_call_review`
blocked when Gemini returned malformed adjudication JSON for one local-analysis
subrequest. The provider HTTP response was valid; the assistant `message.content`
was supposed to be a JSON object and contained raw text after a closed
`rationale` string.

## Evidence

Run:

```text
/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-profile-20260524-151353/BurntSushi__ripgrep-2209/runs/run-1779660936587-structured-current-policy-e89a791e
```

Latest observed log:

```text
/home/brasides/.ploke-eval/logs/ploke_eval_20260524_153512_1221953.log
```

Malformed `message.content` shape:

```json
{
  "verdict": "key_progress",
  "confidence": "high",
  "rationale": "... search for 'replace_with_captures_at'."
  at Matcher' in call 16."
}
```

The parser correctly rejected this with `expected ',' or '}'`: the stray
`at Matcher' in call 16."` text is outside the string value.

## Broken Contract

Protocol local-analysis review should not permanently block on one malformed
JSON adjudication response. It should retry the adjudication request a bounded
number of times, while still refusing to persist invalid or semantically
rewritten malformed content as protocol evidence.

## Fix

Implemented in `crates/ploke-eval/src/cli.rs`.

`tool_call_review` and `tool_call_segment_review` now retry their three-branch
local-analysis procedure up to three times when a branch fails with
`ProtocolLlmError::ParseJson`. The parser still rejects semantic stray text; the
retry policy asks the model for a fresh valid adjudication instead of salvaging
the bad response.

Regression coverage:

```text
cargo test -p ploke-eval tool_call_review_malformed_json_parse_is_retryable_for_all_judgment_branches -- --nocapture
```

The test covers malformed JSON parse errors in all three judgment branches:
usefulness, redundancy, and recoverability. Non-parse errors such as missing
content remain non-retryable.

## Verification

Verified:

```text
cargo test -p ploke-eval tool_call_review_malformed_json_parse_is_retryable_for_all_judgment_branches -- --nocapture
cargo check -p ploke-eval
cargo fmt --all
```

Remaining resume guard: rebuild the main `ploke-eval` binary and run the next
bounded `prototype1-step` against the cleaned campaign worktree using the source
checkout binary, not a stale worktree-local binary.
