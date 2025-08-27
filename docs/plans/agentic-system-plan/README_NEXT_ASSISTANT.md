Onboarding Guide — Agentic System Plan

Status Snapshot (2025-08-27 02:56:05Z)
- Test runs (see full log: docs/plans/agent-system-plan/testing/test_run_20250827-025605Z.txt)
  - ploke-embed: long suite largely env-gated; 38 passed, 0 failed, 8 ignored (in this run). Two very long tests are now #[ignore] by default: test_batch_ss_nodes and test_index_bm25.
  - ploke-db: aggregate 50 passed, 0 failed, 13 ignored (in this run). DB backup availability fixed prior sandbox errors.
  - ploke-rag: 23 passed, 0 failed (we adjusted the trimming test to match the ApproxCharTokenizer semantics).
  - ploke-tui: 43 passed, 3 failed, 4 ignored (across targets). Failing: m1_edit_proposal_flows (3 tests)
    - stage_proposal_creates_pending_entry_and_preview — proposal not found
    - deny_marks_denied_and_does_not_change_file — proposal not found
    - approve_applies_edits_and_updates_status — file did not contain applied replacement

What’s Next (Phase 1 slice)
1) Approvals Overlay + Proposal Flow
   - Build the Approvals overlay with a reusable scroll trait abstraction.
   - Ensure staging creates proposal entries in state.proposals and previews are accessible.
   - Fix the three failing tests: crates/ploke-tui/tests/m1_edit_proposal_flows.rs.
   - Add open-in-editor action (spawn user’s editor with file and line when available).

2) Local Git Branch Apply/Revert (gix)
   - Use a minimal helper over gix (gitoxide) for: ensure repo/init, create/switch branch, stage/commit selected files, revert/switch back.
   - No remote pushes by default; explicit confirmation for commits.
   - Render `git diff` into the overlay for review.

3) Observability Persistence (Phase 1 scope)
   - Persist proposal and apply_result rows; add session trace compact index JSON per request.
   - Wire retrieval_event persistence in request_code_context tool handler.

4) Testing & Benchmarks Gates (must pass to close Phase 1)
   - UI snapshot tests for Approvals overlay (scrolling, actions).
   - Proposal staging/lookup multistep tests (fix the 3 failing tests).
   - Git local ops tests on a temp repo (create/commit/revert, diff render).
   - OpenRouter request/response serde round-trip coverage for types.
   - UI event loop draw benchmark to verify ≥60 fps baseline on a fixed Rect.

How To Run Tests
- Full suite: `cargo test`
- Per crate: `cargo test -p <crate>`
- Long embed tests: set env per test name, e.g. `PLOKE_EMBED_RUN_TEST_BATCH_SS_NODES=1 cargo test -p ploke-embed`
- Live OpenRouter tests: set OPENROUTER_API_KEY and PLOKE_LIVE_MODELS=1 and choose low-cost tool-capable endpoints.
- Test logs: see `docs/plans/agent-system-plan/testing/` (latest run: `test_run_20250827-025605Z.txt`).

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
- SysInfo verbosity: begin with simple On/Off; plan for levels and auto-aging transient messages.
- Post-apply rescan: rescan entire crate for integrity (incremental updates to follow later phases).
