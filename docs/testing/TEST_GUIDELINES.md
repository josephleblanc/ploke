Ploke Testing Guidelines

Purpose
- Ensure changes are safe, observable, and user‑focused. Tests validate behavior, properties, and invariants; they should be reliable, meaningful, and fast by default.

Principles
- Small, isolated unit tests for pure logic; integration tests for cross‑crate flows; end‑to‑end tests gated behind envs to protect CI time and cost.
- Tests must assert observable properties (inputs → outputs), not implementation details.
- Use fixtures deterministically; mock external services; gate network/tool‑calling tests behind env (e.g., OPENROUTER).
- Prefer per‑test env gates (FOO_RUN_TEST_X=1) over global flags; default to skip/ignore for long or external.

Env Gating Patterns
- Long running (embedding/indexing) tests: require PLOKE_EMBED_RUN_<TEST_NAME>=1.
- Live LLM tests (OpenRouter): require OPENROUTER_API_KEY and PLOKE_LIVE_MODELS=1; choose low‑cost models/providers.
- Expensive integration (git/MCP): require PLOKE_E2E_MCP=1.

Ratatui/CLI Testing
- Separate rendering from state (pure functions returning layout data); snapshot widget output via ratatui::buffer::Buffer into a string and assert patterns.
- Use golden files or insta‑style snapshots sparingly; prefer semantic assertions (presence/ordering of key lines/labels).
- Avoid relying on terminal dimensions; fix a test area Rect.

What Makes A Good Test
- Validates intended behavior and constraints (happy path + key failure modes).
- Fails deterministically when behavior regresses.
- Minimal setup; clear name; explains intent in comments when non‑obvious.

Benchmarks (criterion)
- Benchable criteria (top priority first):
  1) BM25 rebuild latency, avgdl computation throughput.
  2) Dense search throughput (queries/sec) at typical k.
  3) Hybrid fusion overhead (RRF, optional MMR) per request.
  4) Tool round‑trip latency (request_code_context → result) with local fixtures.
  5) Edit preview build time (diff rendering for N files).
  6) EventBus throughput under tool call fan‑out.
  7) Model request overhead (payload build + provider response parse).
- Record results in `target/benchmarks/YYYYMMDD/` and link from a run index.

Plan Stopping Points (must‑pass gates)
- End of Phase 1: approvals overlay + open‑in‑editor/revert (UI snapshot tests), proposal persistence; unit tests for SeaHash helpers.
- End of Phase 2: agent orchestrator minimal flow; tests for planner/editor/critic state transitions; tool dispatch smoke tests.
- End of Phase 3: post‑apply re‑scan/update; retrieval_event persistence; hybrid search regression tests.
- A PR cannot merge without green tests at current phase; benchmarks recorded at least once per phase.

Production‑Readiness Notes
- Keep all external tests gated; document required envs at test top.
- Add cost tracking tests with cheap models; skip by default.
- Prefer deterministic in‑memory DB where possible; otherwise prebuilt fixtures.

