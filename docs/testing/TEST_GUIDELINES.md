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

External Tests (Human Verified)
- All properties relied on for agent behavior must have corresponding external/integration tests (gated by env). These tests must be observed as passing by a human reviewer before progressing beyond stopping points in AGENTIC_SYSTEM_PLAN. Examples:
  - OpenRouter tool round‑trip (request_code_context, get_file_metadata/apply_code_edit) on known tool‑capable endpoints.
  - Git branch apply/revert on a temp repo (local‑only) using the chosen Rust git crate.
  - Full crate rescan after edit apply verifying DB integrity (counts, index availability) within expected bounds.

Advanced Methodologies
- Property‑based tests: use proptest/quickcheck for invariants (token trimming never exceeds budget; stable dedup preserves order; diff builder idempotence).
- Fuzzing: use cargo‑fuzz to target parsers/serializers (OpenRouter request/response types) and cozo query assembly helpers.
- Snapshot testing: ratatui buffers and compact JSON indices (session trace); assert semantic properties rather than exact strings when feasible.

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

Additional Phase Stopping Points
- Phase 1
  - Edit staging: staging tool produces proposal in state with preview; lookup by request_id works.
  - Approvals overlay: scroll behavior snapshot tested; actions wired to state changes.
  - Git local ops: branch create/commit/revert succeeds on temp repo; diff is rendered.
  - OpenRouter request types: serde round‑trip coverage for all outbound/inbound types.
- Phase 2
  - Planner step selection deterministic under seeds; retries bounded; state machine covered.
  - Editor canonical resolution test: DB span resolution correct for fixtures.
  - Critic gates: simulated lint/format/test results drive accept/deny paths.
- Phase 3
  - Rescan: DB counts stable after apply; index rebuild available; retrieval_event persisted with items and scores.
  - Session Trace: compact index generated with per‑request entries; overlay renders navigable timeline.

UI + Ratatui Tests and Benchmarks
- Tests: snapshot buffers for overlays (Approvals, Context Items, Model Browser); scrolling interactions; input mode indicators; error/status banners.
- Benchmarks: measure draw() latency per frame for the main event loop; target ≥60 fps (≈16.7 ms/frame) baseline; aspire to 120 fps (≈8.3 ms/frame). Track:
  - Event loop end‑to‑end time under synthetic loads.
  - Overlay render time (diff/codeblock) for N files.
  - Input latency from keypress to displayed character.
  - Scroll performance (lines/sec) over large histories.
  - Model browser search/populate latency.

Production‑Readiness Notes
- Keep all external tests gated; document required envs at test top.
- Add cost tracking tests with cheap models; skip by default.
- Prefer deterministic in‑memory DB where possible; otherwise prebuilt fixtures.

Using `fixture_nodes` + Backup DB (Pattern)
- Prefer realistic tests against the well-known `fixture_nodes` crate:
  - Path: `tests/fixture_crates/fixture_nodes`
  - If a test mutates files, restore from `tests/fixture_crates/fixture_nodes_copy` on teardown or panic to keep runs hermetic.
- Use the backed-up, parsed and embedded database to validate canonical edits without live indexing costs:
  - Path: `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`
  - This DB is heavily validated in `syn_parser` (fields and NodeId v5 UUID regeneration), making it a strong ground truth.
  - Load/import this DB in tests to verify spans, hashes, and canonical resolution prior to and after edits.
- Apply this pattern in integration tests for approvals overlay, apply/deny flows, and post-apply rescans. Record evidence logs alongside other test outputs.

Editor Command Testing
- Editor command resolution precedence: config override (optional) → `$PLOKE_EDITOR` → None (no-op with SysInfo guidance).
- Testing approach:
  - Unit test command construction and precedence; do not spawn by default.
  - Attempt up to three strategies to verify spawn (e.g., mockable command runner, `echo`-style harmless command, or dev-only helper). If unsuccessful after three attempts, abandon the attempt, add a report with findings, and annotate tests with any added dependencies or special handling.
