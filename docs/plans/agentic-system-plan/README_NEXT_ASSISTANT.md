Onboarding Guide — Agentic System Plan

Status Snapshot (updated)
- Latest logs under `docs/plans/agent-system-plan/testing/` (see timestamped files for evidence):
  - m1_edit_proposal_flows now passes (3/3): proposal staging, approve apply, deny no-change.
  - Added proposals persistence test (roundtrip save/load) passing.
  - Remainder of suite unchanged; long/expensive tests remain ignored by default.

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
   - UI snapshot tests for Approvals overlay (scrolling, actions).
   - Proposal staging/lookup multistep tests (passing).
   - Git local ops tests on a temp repo (create/commit/revert, diff render).
   - OpenRouter request/response serde round-trip coverage for types.
   - UI event loop draw benchmark to verify ≥60 fps baseline on a fixed Rect.
   - Realistic tests using `fixture_nodes` + backup DB (see Testing guidelines).

How To Run Tests
- Full suite: `cargo test`
- Per crate: `cargo test -p <crate>`
- Long embed tests: set env per test name, e.g. `PLOKE_EMBED_RUN_TEST_BATCH_SS_NODES=1 cargo test -p ploke-embed`
- Live OpenRouter tests: set OPENROUTER_API_KEY and PLOKE_LIVE_MODELS=1 and choose low-cost tool-capable endpoints.
- Test logs: see `docs/plans/agent-system-plan/testing/` (latest run files by timestamp).
- Fixture-backed tests: use `tests/fixture_crates/fixture_nodes` and its backup DB at `tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92` for realistic spans/hashes.

Key Documents To Read
- Plan: AGENTIC_SYSTEM_PLAN.md
- Agents operating guide: AGENTS.md (workflow + type-safety policy)
- Affected code paths guide: docs/reports/affected_code_paths_20250826-204758Z.md
- Testing guidelines: docs/testing/TEST_GUIDELINES.md (stopping points, UI snapshot and benchmarks)
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
