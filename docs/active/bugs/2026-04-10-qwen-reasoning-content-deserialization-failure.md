# Bug: qwen/qwen3.6-plus returns reasoning without content causing deserialization failure

**Date Discovered:** 2026-04-10  
**Crate Affected:** `ploke-llm` / `ploke-tui`  
**Severity:** Medium - Affects specific models  
**Status:** Open

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

## Proposed Fix Options

### Option A: Make content optional in deserialization

Change the `content` field in the message struct to `Option<String>` and handle `None` gracefully.

```rust
// In ploke-llm OpenRouter response types
struct ResponseMessage {
    role: String,
    content: Option<String>,  // Was: String
    reasoning: Option<String>,
}
```

### Option B: Coalesce reasoning to content

When content is missing but reasoning is present, use reasoning as content (with appropriate logging).

```rust
let content = message.content
    .or_else(|| {
        tracing::warn!("Model returned reasoning without content, using reasoning as content");
        message.reasoning.clone()
    })
    .ok_or_else(|| DeserializationError::MissingContent)?;
```

### Option C: Reject with specific error

Keep current behavior but add a more descriptive error message indicating this is a model behavior issue, not a protocol violation.

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
