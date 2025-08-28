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
- Live LLM tests (OpenRouter): require OPENROUTER_API_KEY and an explicit gate. By default, these are skipped unless one of the following is set:
  - `PLOKE_RUN_LIVE_TESTS=1` (top-level E2E live tests)
  - `PLOKE_RUN_EXEC_LIVE_TESTS=1` (in-crate exec_live_tests diagnostics)
  - `PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1` (real tools roundtrip smoke)
  Choose low‑cost models/providers when enabled.
- Expensive integration (git/MCP): require PLOKE_E2E_MCP=1.

Live Gates: No Green‑on‑Skip
- When any live gate is ON (e.g., `PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1`), tests MUST NOT pass if prerequisites are unmet or the live path is not exercised.
  - No tools‑capable endpoint discovered → fail with actionable message and write endpoint diagnostics under `target/test-output/openrouter_e2e/`.
  - No provider `tool_calls` observed for required tools within timeout → fail with explicit assertion.
  - No `EditProposal` staged or status does not transition to `Applied` after approval → fail.
  - No file delta observed post‑apply → fail.
  - Treat “skip” as “did not validate the requirement”; returning ok is forbidden under live gates.

Evidence Requirements (Production‑Readiness)
- For readiness gates (Phase 1/2/3), every passing live test must produce verifiable evidence:
  - Summarize satisfied properties in the test output (tool_calls seen, proposal staged, Applied status, file change confirmed).
  - Persist compact traces/artifacts in `target/test-output/...` (e.g., OpenRouter endpoints response, observed tool events, selection reasoning).
  - Link to these artifacts in the test run summary or follow‑up report. Do not commit artifacts unless explicitly requested.

Endpoint Detection Hardening
- When selecting tools‑capable endpoints, accept either indicator:
  - `supported_parameters` contains `"tools"`, or
  - `capabilities.tools == true`.
- Try both author/slug forms if applicable (e.g., `kimi/kimi-k2` and `moonshotai/kimi-k2`).
- Fail fast with clear guidance if no candidate is found when live gate is ON.

External Tests (Human Verified)
- All properties relied on for agent behavior must have corresponding external/integration tests (gated by env). These tests must be observed as passing by a human reviewer before progressing beyond stopping points in AGENTIC_SYSTEM_PLAN. Examples:
  - OpenRouter tool round‑trip (request_code_context, get_file_metadata/apply_code_edit) on known tool‑capable endpoints.
  - Git branch apply/revert on a temp repo (local‑only) using the chosen Rust git crate.
  - Full crate rescan after edit apply verifying DB integrity (counts, index availability) within expected bounds.

Advanced Methodologies
- Property‑based tests: use proptest/quickcheck for invariants (token trimming never exceeds budget; stable dedup preserves order; diff builder idempotence).
- Fuzzing: use cargo‑fuzz to target parsers/serializers (OpenRouter request/response types) and cozo query assembly helpers.
- Snapshot testing: ratatui buffers and compact JSON indices (session trace) should use a dual approach:
  - Keep semantic assertions for explicit properties (presence, ordering, counters, labels).
  - In addition, add visual snapshots via the insta crate for critical views (Approvals overlay, Model Browser, Context Items). Snapshots complement semantics and are not a replacement.
  - Redact non‑deterministic data (UUIDs, timestamps, absolute paths) and fix Rect sizes/sort order to minimize churn.

Ratatui/CLI Testing
- Separate rendering from state (pure functions returning layout data) and render using `ratatui::backend::TestBackend`.
- Always include semantic assertions (presence/ordering/labels) for intent clarity.
- For critical views, add insta snapshots in addition to semantics:
  - Add `insta` as a dev‑dependency with the `redactions` feature.
  - Convert the Buffer to lines; apply filters/redactions for dynamic data.
  - `insta::assert_snapshot!(name, redacted_text)` gives a stable visual check.
- Avoid relying on terminal dimensions; fix a test area `Rect` and stabilize sorting.

Insta Snapshots (visual)
- Cargo setup: in the crate under test add
  - `[dev-dependencies] insta = { version = "*", features = ["redactions"] }`
- Writing snapshots:
  - Render into a fixed `Rect` and collect rows to a `String`.
  - Redact dynamic bits with filters (UUIDs, timestamps, paths) or `redactions`.
  - Assert with `insta::assert_snapshot!("case_name", redacted_text);` alongside semantic asserts.
- Reviewing and maintaining snapshots:
  - Run the test once to create pending snapshots, then review with `cargo insta review`.
  - Commit the test and approved `snapshots/*.snap` files after human review.
  - CI must not auto‑update snapshots; failing diffs should block until reviewed.

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
- Tests: semantic assertions plus insta snapshots for overlays (Approvals, Context Items, Model Browser); scrolling interactions; input mode indicators; error/status banners.
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
- Do not claim phase readiness without a passing live run that exercised the real path; include evidence per the requirements above.

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

Test Documentation and Review
- Doc comment requirement: every non‑trivial test file must begin with a doc comment that:
  - States the purpose/scope of the tests in that file.
  - Explains how the tests adhere to these guidelines (env gating, deterministic setup, fixed Rect, semantic + snapshot checks, redactions).
  - Enumerates which properties are verified and which are intentionally not verified (and why). Examples: verify selection highlighting and labels; do not verify color codes to avoid over‑coupling to theming.
  - Notes any residual non‑determinism and how it is mitigated (sorting, seeds, redactions).
- Post‑pass human review checklist (performed after tests are green):
  - Validate semantic assertions cover intended behavior and constraints.
  - Inspect insta snapshots via `cargo insta review`; verify redactions and that diffs are meaningful.
  - Ensure dynamic data is normalized; Rect sizes fixed; sorting stabilized.
  - Confirm external behavior is gated/skipped and no artifacts leak outside `target/`.
  - Check test names and failure messages are descriptive and actionable.

Reporting Policy
- By default, do not write test outputs to docs files. Summarize results inline in the run output with a short report (pass/fail counts and notable failures).
- When deeper artifacts are necessary (e.g., live API diagnostics), allow tests to write to `logs/` or `target/test-output/` as already implemented, but avoid committing these. Only enable extended artifact writing when explicitly requested.
- Editor command resolution precedence: config override (optional) → `$PLOKE_EDITOR` → None (no-op with SysInfo guidance).
- Testing approach:
  - Unit test command construction and precedence; do not spawn by default.
  - Attempt up to three strategies to verify spawn (e.g., mockable command runner, `echo`-style harmless command, or dev-only helper). If unsuccessful after three attempts, abandon the attempt, add a report with findings, and annotate tests with any added dependencies or special handling.
  - Use `-q` for `--quiet` for most tests. Allow normal verbosity if needed.
Live API Endpoint Tests (OpenRouter)
- Prefer cheaper, tool-capable models to reduce cost and flakiness; `kimi/kimi-k2` is recommended and known to support tools.
- Budget: treat live tests as having a budget of three executions while iterating.
  - Each run consumes one budget unit; each failure must be maximally informative (clear diffs/logs) to guide the next edit.
  - If you exhaust the budget (3 failed runs) while trying to reach desired behavior without weakening test integrity, write a report:
    - What you tried, what failed/succeeded, and open questions.
    - Ask for guidance before continuing.
- Gating: require `OPENROUTER_API_KEY` and an explicit gate (e.g., `PLOKE_RUN_LIVE_TESTS=1`). Default to skip in CI.
- Capture useful metadata (endpoint chosen, prices, rate-limits) in logs/artifacts under `target/test-output/` when helpful.
