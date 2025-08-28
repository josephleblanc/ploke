Onboarding Guide — Agentic System Plan

Status Snapshot (updated)
- Phase 1 core items are implemented and tested:
  - Approvals overlay (keys, semantic + insta snapshots, fixture-backed render).
  - Proposal persistence (roundtrip + missing/corrupt).
  - Post-apply rescan (SysInfo trigger verified).
  - Open-in-editor convenience (precedence + args; spawn smoke test).
  - Per-request outcome summaries (SysInfo mapping tests).
- Live tests are gated by env; a cheap tools-capable endpoint test for `kimi/kimi-k2` exists and is skipped by default.
- Snapshot tests are stable (UUIDs fixed); re-approval completed.

What’s Next (Phase 1 slice)
1) Approvals Overlay + Proposal Flow
   - Overlay: list proposals (id/status/files) and details pane (diff/code blocks).
   - Keybindings: open/close = e, approve = Enter or y, deny = n or d, navigate = j/k or arrows, open-in-editor = o.
   - Ensure staging creates entries in state.proposals and previews are accessible.
   - Tests: ratatui snapshot tests; proposal flow tests verified.

2) Local Git Branch Apply/Revert (gix)
   - Use a minimal helper over gix (gitoxide) for: ensure repo/init, create/switch branch, stage/commit selected files, revert/switch back.
   - No remote pushes by default; explicit confirmation for commits.
   - Render `git diff` into the overlay for review.

3) Observability Persistence (Phase 1 scope)
   - Persist proposal and apply_result rows; add session trace compact index JSON per request.
   - Wire retrieval_event persistence in request_code_context tool handler.

4) Testing & Benchmarks Gates (must pass to close Phase 1)
   - UI snapshot tests for Approvals overlay (scrolling, actions) — in place.
   - Proposal staging/lookup multistep tests — in place.
   - Git local ops tests on a temp repo (create/commit/revert, diff render) — TODO.
   - OpenRouter request/response serde round-trip coverage for types — in place.
   - UI event loop draw benchmark to verify ≥60 fps baseline — TODO (criterion present, tune target Rect).
   - Realistic tests using `fixture_nodes` + backup DB — partial (overlay render via TEST_APP; add canonical-edit overlay test).
   - Live tools-capable endpoint test (prefer `kimi/kimi-k2`) — in place and gated; add end-to-end tool call lifecycle smoke.

Phase 1 Test Coverage and Gaps
- Covered
  - Overlay UX: keys (approve/deny/open), semantic render + insta snapshots, fixture-backed render using TEST_APP.
  - Proposal lifecycle: stage → approve/apply (IoManager) → deny — all pass.
  - Persistence: proposals save/load roundtrip + corrupt/missing handling.
  - Post-apply: rescan trigger emits SysInfo.
  - Editor UX: command resolution precedence, arg formatting; spawn smoke test.
  - Outcome summaries: SysInfo classifications for success/404/429/generic error.
  - Live endpoints: list tools-capable endpoints for `kimi/kimi-k2`.
- Gaps (must address for production readiness)
  - Git local ops (temp repo): create branch, stage/commit, revert; diff render validation.
  - Overlay + canonical edits (fixture-backed): stage a canonical edit using backup DB spans, open overlay, approve/deny; assert DB integrity post-apply.
  - Full tool-call lifecycle on live endpoint (env-gated): request → tool calls → apply_code_edit attempt, with strict assertions on state transitions (at least smoke-level, without weakening integrity).
  - Cost estimation sanity: derive rough token/cost for constructed request and surface in SysInfo; add tests (pure logic) to ensure bounds and formatting; optionally add a cheap live check that reports provider pricing meta.
  - UI draw performance benchmark: ensure stable frame render time on fixed Rect under synthetic load (≥ 60 fps baseline).
  - Error-path UI tests: overlay displays Failed proposals cleanly; denies preserve file integrity; spawn failures report guidance without blocking.

Live API Testing Policy (Phase 1)
- Prefer cheaper models with tool support — `kimi/kimi-k2` recommended.
- Budget: three live runs per development effort; each failure must be informative; on exhausting budget without success, write a report and request guidance.
- Gating: require `OPENROUTER_API_KEY` and an explicit gate (e.g., `PLOKE_RUN_LIVE_TESTS=1`).
- Artifacts: keep live test diagnostics under `target/test-output/` and avoid committing artifacts unless explicitly requested.

How To Run Tests
- Full suite: `cargo test`
- Per crate: `cargo test -p <crate>`
- Long embed tests: set env per test name, e.g. `PLOKE_EMBED_RUN_TEST_BATCH_SS_NODES=1 cargo test -p ploke-embed`
- Live OpenRouter tests: by default, these are skipped to avoid cost. To enable, set OPENROUTER_API_KEY and one of the gates:
  - `PLOKE_RUN_LIVE_TESTS=1` (top-level E2E/live diagnostics)
  - `PLOKE_RUN_EXEC_LIVE_TESTS=1` (exec_live_tests diagnostics)
  - `PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1` (real tools roundtrip)
  Choose low-cost tool-capable endpoints when enabled.
- Reporting: prefer a brief inline summary (pass/fail/ignored counts and notable failures) over writing run outputs to docs. Only record artifact files when explicitly requested.
- Fixture-backed tests: use `tests/fixture_crates/fixture_nodes` and its backup DB at `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92` for realistic spans/hashes.

Key Documents To Read
- Plan: AGENTIC_SYSTEM_PLAN.md
- Agents operating guide: AGENTS.md (workflow + type-safety policy)
- Affected code paths guide: docs/reports/affected_code_paths_20250826-204758Z.md
- Testing guidelines: docs/testing/TEST_GUIDELINES.md (stopping points, UI snapshot and benchmarks)
  - Live API policy: budgeted runs, `kimi/kimi-k2` recommendation, gating, artifacts.
- Observability: docs/reports/observability_status_20250826-204758Z.md and docs/reports/db_observability_audit_20250826-204758Z.md
- Git for agents: docs/reports/agents_git_best_practices_20250826-204758Z.md
- Design reflections: docs/reports/design_reflections_20250826-204758Z.md
- Production readiness: docs/reports/production_readiness_notes_20250826-204758Z.md

Implementation Logs (create for each change)
- Location: `docs/plans/agentic-system-plan/impl-log/`
- Filename: `impl_YYYYMMDD-HHMMSSZ.md`
- Include:
  - Rationale for change(s)
  - Files touched (paths)
  - AGENTIC_SYSTEM_PLAN step operated on
  - Next steps / recommendations
  - References (docs, code locations, external specs)

Design/UX Preferences To Respect
- Strong typing for all OpenRouter request/response types (serde derives; numeric fields as numeric types). Consider GhostData if helpful for staged states.
  - Do not loosen typed parsing; prefer tagged enums over ad-hoc detection; make invalid states unrepresentable.
- Approvals overlay: readable diffs, scrolling, and fast revert path via local git; open-in-editor convenience.
  - Open-in-editor precedence: config `ploke_editor` → env `$PLOKE_EDITOR` → no-op with SysInfo guidance.
  - Vim-like keybinding for overlay toggle: `e`.
- SysInfo verbosity: begin with simple On/Off; plan for levels and auto-aging transient messages.
- Post-apply rescan: rescan entire crate for integrity (incremental updates to follow later phases).
