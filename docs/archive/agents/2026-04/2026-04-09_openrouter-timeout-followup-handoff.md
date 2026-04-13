# 2026-04-09 OpenRouter Timeout Follow-up Handoff

- Date: 2026-04-09
- Task title: Follow-up on provider/network reliability after ripgrep setup fix
- Focus: separate downstream OpenRouter timeout/retry behavior from the completed ripgrep setup/parser fix, and capture the next investigation slice.

## Why This Exists

While validating the historical ripgrep replay, we initially saw repeated OpenRouter retry noise after setup succeeded. That was **not** the root cause of the ripgrep indexing failure. It only became visible once the `convert_keyword_2015` parser fallback got the replay past indexing and into the live agent/model turn.

This document exists so the provider/runtime concern does not get conflated with the parser/setup fix.

## Current Conclusion

The observed OpenRouter issues appear to be a **separate reliability concern** in the live chat/agent loop, not evidence that the ripgrep parser fix is unsound.

Most likely shape:

- provider/network instability or slow response-body delivery
- retried by our chat loop as a transport failure
- surfaced as repeated `chat_step failed; retrying` warnings

Less likely, based on current evidence:

- malformed request construction on our side
- deterministic provider-protocol mismatch

## What We Observed

During earlier replay runs that used the agent path, the historical ripgrep setup no longer failed during indexing, but the run later entered repeated OpenRouter retry loops.

Reported symptom shape:

- `chat_step failed; retrying`
- `error decoding response body`
- repeated `retried_request_errors=N`

These only appeared once the replay got beyond setup and into the benchmark turn.

## Important Distinction

There are two runner paths in `ploke-eval`:

- setup-only path:
  - [`RunMsbSingleRequest::run`](../../../crates/ploke-eval/src/runner.rs)
- agent/live-model path:
  - [`RunMsbAgentSingleRequest::run`](../../../crates/ploke-eval/src/runner.rs)

The timeout/retry noise came from the **agent/live-model path**, not the setup-only path now used by the historical replay tests.

## Relevant Code Paths

### HTTP request and response-body handling

- [`crates/ploke-llm/src/manager/session.rs`](../../../crates/ploke-llm/src/manager/session.rs)

Key details:

- `chat_step` applies a single reqwest timeout with:
  - `.timeout(cfg.timeout)`
- it then:
  - sends request headers/body
  - awaits the response
  - calls `resp.text().await`
- failures while reading the body are mapped to `LlmError::Request`

This means slow or unstable response-body delivery can look like a generic request/transport failure.

### Chat loop retry behavior

- [`crates/ploke-tui/src/llm/manager/session.rs`](../../../crates/ploke-tui/src/llm/manager/session.rs)
- [`crates/ploke-tui/src/llm/manager/loop_error.rs`](../../../crates/ploke-tui/src/llm/manager/loop_error.rs)

Key details:

- request errors from `chat_step` are classified by `classify_llm_error`
- transport timeouts are treated as retryable
- the loop logs:
  - `chat_step failed; retrying`
- retries are bounded by `chat_policy.error_retry_limit`

### Timeout configuration

- [`crates/ploke-tui/src/user_config.rs`](../../../crates/ploke-tui/src/user_config.rs)
- [`crates/ploke-tui/src/llm/manager/mod.rs`](../../../crates/ploke-tui/src/llm/manager/mod.rs)

Key details:

- HTTP timeout for chat calls comes from `llm_timeout_secs`
- tool execution timeout is separate: `tool_call_timeout_secs`
- retry policy uses `ChatTimeoutStrategy` and `error_retry_limit`

This is easy to confuse during eval debugging because `tool_call_timeout_secs` is not the same thing as the HTTP request timeout.

### Provider resolution

- [`crates/ploke-eval/src/runner.rs`](../../../crates/ploke-eval/src/runner.rs)

Key detail:

- provider selection is based on tool capability and preferences, not observed provider health/stability

## Current Assessment

### What looks fine

- We appear to be constructing normal OpenRouter requests.
- The failures we saw do not look like clean 4xx API misuse or deterministic schema breakage.
- The retry path is at least classifying transient transport failures as retryable, which is reasonable.

### What still looks weak

- We use one coarse timeout across the whole request/body-read lifecycle.
- We do not distinguish well between:
  - connect failure
  - header wait timeout
  - response-body read timeout
  - malformed/incomplete provider payload
- Retry/backoff is generic and does not include:
  - jitter
  - provider failover
  - stronger classification of repeated body-read failures
- Replay/debugging can accidentally mix setup problems with live-provider problems if the wrong runner path is used.

## Suggested Next Investigation Slice

Keep this narrowly scoped and observational first.

### 1. Improve telemetry around request failures

Add or refine logs/diagnostics to distinguish:

- send/connect timeout
- response header arrival vs body read failure
- deserialization failure after full body read
- provider HTTP status vs transport error

Goal:

- be able to say exactly which phase OpenRouter is failing in

### 2. Review timeout policy defaults

Confirm whether current defaults are sensible for eval runs:

- `llm_timeout_secs`
- `error_retry_limit`
- `ChatTimeoutStrategy`

Goal:

- avoid wasting long eval wall-clock on retries that are unlikely to recover

### 3. Consider provider-health fallback

If repeated transport/body-read failures occur for a selected provider:

- optionally switch to another tool-capable provider for the same model
- or fail fast with a clearer typed transport verdict

Goal:

- improve eval stability without masking real provider issues

### 4. Keep setup tests separate from live-model tests

Continue using the setup-only runner for setup reliability assertions.

Goal:

- prevent provider noise from obscuring parser/indexing regressions

## Resume Prompt

Continue from `docs/active/agents/2026-04-09_openrouter-timeout-followup-handoff.md`.
Inspect the chat HTTP/session path and classify the observed OpenRouter failures more precisely, focusing first on telemetry and timeout semantics before changing retry behavior.
