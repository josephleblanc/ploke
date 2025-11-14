# Blocker 07 – Cost Tracking, Rate Limits & Safety Policies

_Last updated: 2025-11-14_

## Problem statement
- Remote embedding runs can be expensive and rate-limited (HF free tier ≈ 200 req/min, OpenAI varies by org). We currently lack:
  - Budget controls or cost projections before launching an indexing job.
  - Backoff/limiter logic in `EmbeddingProcessor` to stay under provider quotas.
  - Integration with live-gate evidence to prove we respected safety policies (tool-call observation, approvals, etc.).
- Without a modeling doc we risk surprise bills or 429 storms, and we cannot satisfy AGENTS.md “safety-first editing” + “evidence” principles.

## Goals
1. Define cost estimation formulas per provider/model and expose them in `/embedding status` and indexing UI.
2. Add rate-limit policies (RPM/TPM) + batching heuristics to `EmbeddingService` implementations, with shared limiter code.
3. Integrate budgets + approvals into CLI commands (require user confirmation when estimated cost exceeds threshold).
4. Capture evidence artifacts proving live tool calls occurred under expected rate/approval gates.

## Cost model
### Inputs per provider/model (from registry/pricing data)
- `price_per_1k_tokens_input`
- `price_per_1k_tokens_output` (if relevant)
- `vector_dimension`
- `max_batch_size`

### Estimation formula
```
let chars = snippet.len()
let token_est = max(1, chars / provider.avg_chars_per_token)
let vectors = ceil(snippet_count / batch_size)
let tokens_total = token_est * snippet_count
let unit_cost = price_per_1k_tokens_input / 1000.0
let estimated_cost = tokens_total as f64 * unit_cost
```
- Provide per-batch estimate as well as full-run estimate (based on `Database::count_unembedded_nonfiles()` rows).
- Display cost in UI + `/embedding status` before running indexing; require `--yes` flag if estimate > user-configured budget.

### Budget controls
- Extend `user_config::EmbeddingConfig` with optional `monthly_budget_usd` and `per_run_budget_usd`.
- `EmbeddingManager::create_set` checks estimate vs budgets; if exceeded, prompt user or refuse (configurable).
- Telemetry artifact `target/test-output/embedding/cost_estimate_<ts>.json` records estimate + actual usage (based on telemetry sum) for audit.

## Rate limiting & retries
### Shared limiter
- Add `RateLimiter` struct (tokio semaphore + sliding window) to `EmbeddingService` implementations.
- Configure per provider from registry metadata (HF: `rpm=200` free, `rpm=2500` pro; OpenAI: use `/v1/usage` or config). Expose CLI command `/embedding limits` to view/edit overrides.
- `RateLimiter::acquire(batch_size)` waits as needed before sending request.

### Retry policy
- Standard exponential backoff with jitter for HTTP 429/5xx. Cap at 3 retries per batch.
- Telemetry logs include `retry_count` and whether call succeeded/fell back.
- Provide hooking for `IndexerTask`: if repeated 429s occur, pause job and instruct user to upgrade plan or lower concurrency.

## Safety + approvals
- Remote embedding commands require ack when `provider` not on allowlist (AGENTS: “request human input when instructions too unclear or tests behind gates”). Implementation: `EmbeddingManager` prompts user if provider flagged `requires_confirmation` (set for new providers, e.g., Cohere) and logs answer.
- For live-tool gating, we must record tool-call traces (per Blocker 05) and surface them in `/embedding status --live`.
- Integrate with IoManager to stage any config changes referencing credentials; never emit secrets in logs.

## Evidence artifacts
- `target/test-output/embedding/live/openai_<ts>.json` includes `retry_count`, `rpm_budget`, `cost_estimate` vs. `actual_cost` so we can prove budgets/rate limits were honored.
- `target/test-output/embedding/budget_<ts>.json` records user approvals when budgets exceeded.

## Tests
- Unit tests for cost estimator (various snippet lengths/dimensions) to ensure accuracy within ±5% of actual telemetry.
- Integration tests for rate limiter: mock provider returning 429 until limiter slows down, ensure job completes without panic.
- CLI tests verifying `/embedding rebuild` refuses to proceed when `estimated_cost > budget` without `--force`.

## Open questions
1. Should we integrate provider billing APIs (OpenAI `/v1/usage`) to reconcile actual charges? – Nice-to-have; record as follow-up.
2. Multi-tenant future: we may need per-user budgets; for now single-user TUI uses global budgets.

With this modeling doc we can implement predictable cost/rate-limit behavior and meet the project’s safety guarantees before enabling remote embeddings for end users.
