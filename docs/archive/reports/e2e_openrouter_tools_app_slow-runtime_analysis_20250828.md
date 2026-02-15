# E2E OpenRouter Tools App — Slow Runtime Analysis

Scope
- Test: `crates/ploke-tui/tests/e2e_openrouter_tools_app.rs`
- Symptom: Runtime > 400s when running live (with `OPENROUTER_API_KEY`).
- Goal: Identify root causes and propose targeted mitigations without losing realistic coverage.

Summary of Findings
- Multiple factors compound to long wall time:
  - Per‑model observation window is generous (20s) and the loop evaluates up to 10 models by default (`PLOKE_LIVE_MAX_MODELS`), yielding ~200s of observation time alone.
  - Observation listens on the Background channel only; tool completion/failure events are emitted on the Realtime channel, so the loop often waits the full window per model.
  - Network timeouts for OpenRouter calls are 45s in the LLM session. Even when the observation loop moves on, spawned tasks can continue running.
  - Upfront BM25 index rebuild adds cost depending on fixture size.
  - Provider metadata fetching per model (catalog + endpoints) adds latency on slow networks.

Evidence and Code References
- Observation loop (background only + long windows):
  - File: `crates/ploke-tui/tests/e2e_openrouter_tools_app.rs`
    - `let mut rx = event_bus.subscribe(EventPriority::Background);`
    - `let observe_until = Instant::now() + Duration::from_secs(20);`
    - `timeout(Duration::from_secs(7), rx.recv())`
- Tool events routing (Realtime for terminal events):
  - File: `crates/ploke-tui/src/lib.rs`
    - `AppEvent::LlmTool(Completed|Failed) => EventPriority::Realtime`
- LLM HTTP timeout (45s):
  - File: `crates/ploke-tui/src/llm/session.rs`
    - `.timeout(Duration::from_secs(45))`
- Tool result wait timeout (30s default):
  - File: `crates/ploke-tui/src/llm/mod.rs` (LLMParameters default `tool_timeout_secs: Some(30)`) and
  - File: `crates/ploke-tui/src/llm/session.rs` uses that value when awaiting tool results.
- BM25 rebuild in test setup:
  - File: `crates/ploke-tui/tests/e2e_openrouter_tools_app.rs`
    - `rag.bm25_rebuild().await?;`

Primary Root Causes
1) Channel mismatch for observation: the test listens on Background while critical tool lifecycle signals (Completed/Failed) are sent on Realtime → observation window is frequently exhausted, even when tools are fired or completed.
2) Per‑model observation budget too high (20s), combined with iterating up to 10 models → long total.
3) Long‑lived network operations: 45s per HTTP request and 30s tool waits can outlive the test’s observation loop; no cancellation is propagated.
4) Expensive upfront indexing via `bm25_rebuild()` adds wall time, especially with larger fixtures.

Contributing Factors
- Catalog + endpoints metadata fetches per model (network dependent).
- No whole‑test deadline; spawned workers not cancelled.
- Default model sweep tries many models instead of starting with one known tools‑capable endpoint.

Mitigations (Prioritized)
1) Observe both channels:
   - Subscribe to Realtime and Background in the test and accept tool lifecycle events from either.
   - Impact: Immediate reduction in wasted waits; better responsiveness.

2) Reduce observation windows and model count:
   - Defaults: `observe_until = 12s`, per‑recv timeout `= 3s`, and `PLOKE_LIVE_MAX_MODELS=1` by default.
   - Impact: Cap worst‑case duration tightly while still exercising live flow.

3) Prefer a known tools‑capable model/provider in tests:
   - Allow `PLOKE_LIVE_MODEL` and optional `PLOKE_LIVE_PROVIDER` to skip catalog scan and endpoints search.
   - Impact: Avoids slow discovery paths and variance.

4) Add a whole‑test timeout and early cancellation:
   - Wrap the main body in `tokio::time::timeout(Duration::from_secs(90), ...)`.
   - After timeout or success, emit `AppEvent::Quit` (already wired) and, in future, add a cancellation signal to LLM tasks.

5) Make BM25 rebuild optional for live E2E:
   - Guard with `PLOKE_LIVE_SKIP_BM25=1` to skip rebuild (or rebuild a small subset).

6) Lower HTTP timeouts for tests:
   - Allow overriding the 45s request timeout via env (e.g., `PLOKE_HTTP_TIMEOUT_SECS=20`) and honor it in `session.rs`.

Proposed Implementation Plan
- Phase 1 (fast improvements):
  - Update the E2E test to:
    - Subscribe to both channels and break on tool Requested/Completed/Failed from either.
    - Lower observation window and per‑recv timeouts; default `PLOKE_LIVE_MAX_MODELS` to 1.
    - Add optional envs: `PLOKE_LIVE_MODEL`, `PLOKE_LIVE_PROVIDER` to short‑circuit discovery.
    - Wrap test flow in a 90s total timeout; on expiry, dump quick diagnostics and fail.
    - Make BM25 rebuild conditional on `PLOKE_LIVE_SKIP_BM25 != 1`.

- Phase 2 (robustness):
  - Expose an LLM/request timeout override via env; apply in `RequestSession`.
  - Add an internal cancellation facility (e.g., an `AppEvent::CancelLlmRequests` or a shared token) to stop active HTTP requests when tests end.

Acceptance Criteria
- Test completes within ~60–90s under typical conditions when a tools‑capable endpoint is found.
- Test fails fast with actionable logs when tools aren’t observed (no 400s hangs).
- Console logs remain readable; the TUI test does not hijack the developer terminal.

Notes
- The newly added `AppEvent::Quit` and headless `run_with(TestBackend, ...)` already prevent terminal takeover and ensure clean UI shutdown; background tasks still need explicit cancellation in a future pass.

