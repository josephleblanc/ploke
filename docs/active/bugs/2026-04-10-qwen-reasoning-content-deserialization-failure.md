# Bug: qwen/qwen3.6-plus returns reasoning without content causing deserialization failure

**Date Discovered:** 2026-04-10  
**Date Fixed:** 2026-04-10  
**Crate Affected:** `ploke-llm` / `ploke-tui`  
**Severity:** Medium - Affects specific models  
**Status:** Fixed (under stability verification)

## Summary

The `qwen/qwen3.6-plus` model (and potentially other reasoning-capable models) returns responses where the `message` object contains a `reasoning` field but **no `content` field**. This causes our OpenRouter response deserialization to fail with `RESPONSE_DESERIALIZATION_FAILED`.

## Error Details

**Error Code:** `RESPONSE_DESERIALIZATION_FAILED`  
**Error Kind:** `ProviderProtocol`  
**Location:** `crates/ploke-tui/src/llm/manager/mod.rs:470`

### Failing Response Structure

```json
{
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "reasoning": "Good. I see the `replace_with_captures_at` definition..."
      // MISSING: "content" field entirely!
    },
    "finish_reason": "stop",
    "native_finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 25354,
    "completion_tokens": 168,
    "total_tokens": 25522
  }
}
```

### Expected Response Structure

```json
{
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": "The assistant's response text here...",
      "reasoning": "Optional reasoning text..."
    },
    "finish_reason": "stop"
  }]
}
```

## Root Cause

Our deserialization struct for the OpenRouter chat completion response likely marks `content` as a required field. When `qwen/qwen3.6-plus` returns only reasoning (no content), deserialization fails.

The model is outputting chain-of-thought reasoning but then not providing an actual response message.

## Reproduction Steps

1. Configure active model to `qwen/qwen3.6-plus`
2. Run an eval instance: `cargo run -p ploke-eval -- run-msb-agent-single --instance BurntSushi__ripgrep-2209`
3. Observe `RESPONSE_DESERIALIZATION_FAILED` error in logs
4. Check response body - contains `reasoning` but no `content`

## Impact

- **Immediate:** Cannot use `qwen/qwen3.6-plus` for eval runs
- **Broader:** Any reasoning-capable model that returns reasoning-only responses will fail similarly
- **Data Loss:** Run records show `llm_response: None` for failed turns (see RunRecord capture)

## Fix Applied

**Selected Option:** B - Coalesce reasoning to content

**Implementation:**
- Feature flag: `qwen_reasoning_fix` in `crates/ploke-llm/Cargo.toml`
- Location: `crates/ploke-llm/src/manager/session.rs` (lines 354-369)
- Behavior: When `content` is `None` but `reasoning` is `Some`, the reasoning text is used as content with a warning log

```rust
// Coalesce reasoning to content when content is missing but reasoning is present.
#[cfg(feature = "qwen_reasoning_fix")]
if let Some(reasoning_text) = reasoning_opt {
    tracing::warn!(
        target: "chat-loop",
        "Model returned reasoning without content; coalescing reasoning to content"
    );
    let outcome = ChatStepOutcome::Content {
        reasoning: None,
        content: Some(ArcStr::from(reasoning_text.as_str())),
    };
    return builder.outcome(outcome).full_response(parsed).build();
}
```

## Test Coverage

Two tests added in `crates/ploke-eval/src/tests/llm_deserialization.rs`:

1. **Diagnostic test** (`test_qwen_reasoning_only_fails_deserialization`)
   - `#[cfg(not(feature = "qwen_reasoning_fix"))]`
   - Documents the exact failure mode pre-fix
   - Passes before fix, fails after fix
   - Will be removed after stability period

2. **Regression test** (`test_qwen_reasoning_only_coalesces_to_content`)
   - `#[cfg(feature = "qwen_reasoning_fix")]`
   - Verifies correct handling post-fix
   - Fails before fix, passes after fix
   - Kept permanently

## Related Code

- `crates/ploke-llm/src/router_only/openrouter/` - Response types
- `crates/ploke-tui/src/llm/manager/mod.rs:461-470` - Error handling
- `crates/ploke-eval/src/runner.rs` - RunRecord capture of failed LLM responses

## Workarounds

1. **Use a different model** that consistently returns content (e.g., `anthropic/claude-sonnet-4-20250514`)
2. **Add system prompt** explicitly requesting the model to always provide a content response
3. **Provider selection** - Some providers may filter/coalesce reasoning responses differently

## Test Case

```bash
# Reproduces the issue
cargo run -p ploke-eval -- model set qwen/qwen3.6-plus
cargo run -p ploke-eval -- run-msb-agent-single --instance BurntSushi__ripgrep-2209
```

Expected: Run completes (success or failure based on agent behavior)  
Actual: `RESPONSE_DESERIALIZATION_FAILED` error, run ends prematurely

---

**Tags:** `llm`, `openrouter`, `deserialization`, `qwen`, `reasoning-models`  
**Related Issues:** None yet
