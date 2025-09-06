Here’s a focused review of the tests still needed for production‑readiness, mapped to
concrete targets and acceptance.

Git Local Ops

- Purpose: Verify local branch apply/revert path is safe and typed.
- Files: crates/ploke-tui/tests/git_local_ops.rs (or crates/ploke-git/tests/*.rs once
helper lands).
- Setup: Temp repo; create/switch branch; write file; stage/commit; revert/switch back;
render unified diff text.
- Assertions: HEAD moves as expected; commit contains selected paths only; revert restores
bytes; diff text contains expected hunks.
- Gating/Artifacts: No network; write diffs under target/test-output/git/.

Canonical Edit + Overlay

- Purpose: Prove canonical DB spans drive correct edits through Approvals overlay.
- Files: crates/ploke-tui/tests/approvals_canonical_edit.rs.
- Setup: Use tests/fixture_crates/fixture_nodes and backup DB at tests/
backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92; resolve span via
ploke_db::helpers::resolve_nodes_by_canon_in_file; stage proposal; render overlay via
TestBackend.
- Assertions: Approve applies edit (IoManager write + SeaHash/TrackingHash verified),
deny preserves bytes; overlay shows file list + diff/codeblocks; post‑apply rescan SysInfo
observed; DB integrity unchanged except expected file hash/span update.
- Gating/Artifacts: No network; render insta snapshots for overlay; write compact evidence
under target/test-output/approvals/.

Live Tool Lifecycle (Smoke)

- Purpose: End‑to‑end tool loop on a tools‑capable endpoint.
- Files: crates/ploke-tui/tests/openrouter_live_tools_roundtrip_smoke.rs (or extend
openrouter_live_tools_roundtrip.rs).
- Setup: Model kimi/kimi-k2; small prompt that triggers request_code_context and attempts
apply_code_edit; let provider drive tool_calls (no forced SystemEvent path).
- Assertions: State transitions observed: Requested → Completed/Failed per tool; final
outcome SysInfo classified; no panics; artifacts written.
- Gating/Artifacts: OPENROUTER_API_KEY and PLOKE_RUN_LIVE_TESTS=1 (or
PLOKE_RUN_EXEC_REAL_TOOLS_LIVE_TESTS=1); artifacts under target/test-output/
openrouter_e2e/; default skip in CI.

Cost Estimation Sanity

- Purpose: Bounded, correctly formatted cost/token estimates without network.
- Files: crates/ploke-tui/src/llm/cost.rs (pure logic) + crates/ploke-tui/tests/
cost_estimator.rs.
- Setup: Feed synthetic prompt sizes and cached pricing structs; test input/output token
splits and totals.
- Assertions: Numeric bounds hold; formatting stable; gracefully handles missing pricing
(None → safe default).
- Gating/Artifacts: Pure unit; no network; no artifacts.

UI Draw Performance (Benchmark)

- Purpose: Baseline render time for overlays at fixed Rect to watch regressions.
- Files: crates/ploke-tui/benches/ui_draw.rs.
- Setup: Build synthetic App state with many proposals; render Approvals overlay via
TestBackend.
- Assertions: Criterion benchmark present and runnable; target ≥60 fps equivalent noted in
bench output.
- Gating/Artifacts: Bench only (non‑failing), no network; optional JSON report under
target/criterion/.

Error‑Path UI

- Purpose: UX remains clear on failures and denials; integrity preserved.
- Files: crates/ploke-tui/tests/approvals_error_paths.rs.
- Setup: Inject proposals with Failed status; simulate editor spawn failure; deny flow.
- Assertions: Overlay displays failure badges/messages; denies leave files untouched;
SysInfo guidance emitted; insta snapshot for failed view.
- Gating/Artifacts: Pure unit; snapshots under crates/ploke-tui/tests/snapshots/.

Notes

- Strong typing: Keep helper APIs and test fixtures strictly typed (no ad‑hoc JSON).
- Gating: Live tests off by default; require explicit env; keep diagnostics under target/
test-output/.
- Reuse: Prefer existing TEST_APP harness and fixture DB; align with docs/testing/
TEST_GUIDELINES.md.
