Plan Title

- Tool↔DB Canonical Resolution And Tool System E2E Validation

Related Docs

- AGENTS: AGENTS.md
- Long-term: AGENT_SYSTEM_PLAN.md
- Contract: crates/ploke-tui/docs/crate-contracts/tool-to-ploke-db.md
- Canonical notes: docs/design/queries/canonical_resolution_notes.md
- Latest impl-log: docs/plans/agentic-system-plan/impl-log/impl_20250831-211500Z.md
- Test artifact: crates/ploke-db/tests/ai_temp_data/test-output.txt

Objectives & Scope

- Diagnose the helper failure precisely and document root causes.
- Update tool↔DB contract with invariants and failure handling.
- Add robust resolver path (strict + relaxed fallback) in ploke-db.
- Validate ploke-tui tool system offline, then live via OpenRouter.
- Produce evidence-backed reports, docs, and regression tests.
- Establish performance baselines with benchmarks.

Principles

- Strong typing: no stringly JSON; numeric fields use numeric types.
- Safety-first: staged edits with hash checks; approvals for destructive ops.
- Evidence-based: summarize pass/fail/ignored; deeper logs under target/....
- Live gates: do not claim green unless live path exercised and properties
verified.
- Rust docs: //! once at top, /// for items; no multiple //!.

Observability & Verification

- Ongoing documentation of changes: add new impl logs with reasoning
- Run tests before/after each Phase to prevent regressions

Phase 1: Diagnose Helper Failure (Artifact-Driven)

- Steps:
    - Re-run helper-vs-id test; collect test-output.txt tail for sampled nodes.
    - Expand diagnostics: list module-only candidates (drop file filter), log
module.path, file_mod.file_path, namespace, relation, NodeType scan order.
    - Log path normalization vs DB file_path (absolute/relative, ends-with).
    - Compare helper vs get_by_id rules: ancestor, file_mod join, module.path,
NOW semantics.
    - Validate hypotheses: absolute path brittleness; nested static/namespace
modeling.
- Acceptance:
    - Artifact explains zero-row outcomes with concrete fields.
    - Notes updated with root cause, query sketches (strict/relaxed), normalization
guidance, examples.
    - Minimal deterministic regression for a nested static (e.g.,
crate::const_static::inner_mod::INNER_MUT_STATIC).

Phase 2: Update Tool↔DB Contract

- Steps:
    - Document strict fast-path and relaxed module-only fallback with normalization
policy.
    - Define error semantics: 0 matches (actionable guidance), >1 matches
(ambiguity with candidates).
    - Add examples for statics, traits, unions, type aliases; link to notes and
helpers.
- Acceptance:
    - Contract reflects inputs/outputs, invariants, and failures with links.

Phase 3: DB Resolver Improvements (Pending Approval)

- Steps:
    - Add resolve_nodes_by_canon (module-only, no file_path ==) mirroring strict
projection.
    - Implement Rust-side path normalization and post-filtering.
    - Orchestrate strict→fallback at the call site; keep strict helper.
    - Unit tests: strict pass when paths match; fallback succeeds when strict
fails.
- Acceptance:
    - Helpers green on fixture DB; nested static and non-static cases covered.

Phase 4: Integrate With Tool System

- Steps:
    - Route canonical lookups through strict→fallback with strong types.
    - Emit structured diagnostics for 0/1/>1 matches (including candidate file
paths).
    - Update contract if API surface changes.
- Acceptance:
    - Fixture flows succeed; ambiguity and errors are actionable.

Phase 5: Offline E2E Tests (Harness)

- Steps:
    - Use crates/ploke-tui/src/test_harness.rs to drive tool calls end-to-end.
    - Verify events, request shapes, response handling, deserialization (incl.
GATs), tool outputs, return messages.
    - Add snapshots; gate non-determinism.
- Acceptance:
    - Shapes/types/state transitions asserted; snapshot stability; evidence under
target/test-output/offline/.

Phase 6: Live API Tests (OpenRouter)

- Steps:
    - Gate via cfg(feature = "live_api_tests"); use TEST_APP.
    - Validate request/response shapes, fields, types; observe tool_calls; ensure
tool outputs sent back to API.
    - Record latency/metrics in serializable structs; store logs under target/
test-output/live/.
- Acceptance:
    - Live path exercised; tool_calls observed; proposal staged; approval→applied;
file delta verified; skips marked “not validated”.

Phase 7: Docs & Test Review

- Steps:
    - Link tests from item and feature docs.
    - Write review covering coverage, gaps, weaknesses, explicit properties
validated; triage follow-ups.
- Acceptance:
    - Docs updated; review in docs/reports/ with action items.

Phase 8: Benchmarks & Profiling

- Steps:
    - Add Criterion benches for critical paths (gate online benches as needed).
    - Record results; profile as helpful; store under benches/bench-output/.
- Acceptance:
    - Benches run reliably; baseline summarized; initial hotspots identified.

Validation & Evidence

- Evidence: concise pass/fail/ignored counts per phase; artifact paths (target/...
or tests/ai_temp_data/) referenced in impl-logs.
- Live gates: only count live tests as green when key properties are observed
(tool_calls, staging, approval→apply, file delta).

Milestones

- M1: Root cause documented; contract updated.
- M2: Resolver improvements implemented; offline tests green.
- M3: Offline E2E stable with snapshots.
- M4: Live E2E validated with live gates satisfied.
- M5: Docs + review complete; benchmarks recorded.

Open Questions

- Normalization policy: absolute vs ends-with vs canonicalized (symlink-aware).
  - Initially absolute, add canonicalized (symlink-aware) with user config after this plan.
- Namespace/static modeling for nested modules: confirm DB relations.
- Embedding independence: ensure canonical flows don’t require embeddings.
  - Incorrect, we may assume embeddings exist. If they do not, print message to user with info on how to embed.
- Live quotas/provider variance for matrix testing; timeouts and gating.
  - API key for openrouter (OPENROUTER_API_KEY) exists in `.env` in workspace root.
  - Specifically created for your use. If credits run out, inform user (unlikely).

Deliverables

- Updated notes in canonical_resolution_notes.md with evidence and query sketches.
- Updated tool-to-ploke-db.md with invariants, examples, and error semantics.
- DB helpers (strict+relaxed) with unit tests and regression cases.
- Offline and live E2E test suites with snapshots and evidence summaries.
- Test review report under docs/reports/.
- Benchmarks and baseline summary under benches/bench-output/.

Check original user request for further details on testing:

- crates/ploke-tui/docs/feature/agent-system/plans/tool-e2e-usr-rqst.md
