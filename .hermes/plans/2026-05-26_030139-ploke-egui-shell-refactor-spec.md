# ploke-egui `shell.rs` Refactor Spec

> **For Hermes:** Use `subagent-driven-development` only after this spec is accepted. Implementation must be sequential for source edits touching `shell.rs`; parallel subagents are allowed only for read-only review/cartography.

**Goal:** Refactor `crates/ploke-egui/src/ui/app/shell.rs` from one 8,469-line mixed-responsibility file into a small facade plus focused modules, without changing UI behavior, protocol authority semantics, cache behavior, or WASM/native cfg boundaries.

**Architecture:** Keep `crate::ui::app::shell` as the stable public surface used by `ui/app/mod.rs` and `ui/dashboard/tiles.rs`. Move implementation behind that facade into child modules under `crates/ploke-egui/src/ui/app/shell/`. Extract by behavioral islands, one slice at a time, with tests staying near the code they protect.

**Tech Stack:** Rust 2024 workspace, `eframe::egui`, `egui_tiles`, `ploke_tree::Graph`, `ploke_records`, existing `ploke-egui` test harness.

---

## 0. Current Evidence and Baseline

Inspected worktree:

- Repo root: `/home/brasides/code/agent-dir/ploke-ui-wt-01`
- Target file: `crates/ploke-egui/src/ui/app/shell.rs`
- Current size: 8,469 lines, 297,365 bytes
- Approx function count: 302
- Existing direct callers of `ui::app::shell`: mostly `crates/ploke-egui/src/ui/app/mod.rs` and `crates/ploke-egui/src/ui/dashboard/tiles.rs`
- Current public shell surface includes:
  - `EvalProtocolRenderMode`
  - `InspectorOpenState`
  - `InspectorRenderCache`
  - `InspectorPanelSection`
  - `render_top_strip`
  - `render_right_inspector`
  - `add_inspector_scroll_end_padding`
  - `render_bottom_timeline`
  - `render_eval_protocol_for_graph`
  - `render_eval_protocol_pane`
  - section renderers such as `render_identity`, `render_roles_and_metrics`, `render_parent_create_for_inspector`, `render_run_records_for_inspector`, `render_graph_edges_for_inspector`, `render_artifact_edges_for_inspector`, `render_patches_for_inspector`, `render_candidate_comparison_for_inspector`, `render_lineage_authority_for_inspector`, `render_source_refs_for_inspector`, `render_artifact_ids_for_inspector`, `render_parent_create_llm_calls`, `render_parent_create_llm_calls_body`.

Baseline checks run before this spec:

```bash
cargo check -p ploke-egui
# PASS, with 2 warnings:
# - unused import `text::CachedTextKind` in ui/render/mod.rs
# - EvalProtocolRenderMode::CallReviewScanOnly dead-code warning in non-benchmark cfg

cargo test -p ploke-egui --lib
# PASS: 101 passed, 0 failed, 1 ignored
# Same 2 warnings.
```

Important dirty-worktree warning:

- The worktree already has many modified files and untracked paths, including `crates/ploke-egui/src/ui/app/shell.rs`, `crates/ploke-egui/src/ui/render/`, `crates/ploke-egui/src/ui/app/section/`, docs, and benchmark artifacts.
- Treat the current worktree as user-owned. Do not reset, clean, delete, or overwrite untracked files unless the user explicitly approves.
- Before implementation, capture `git status --short` again and state which files the slice will touch.

---

## 1. Non-goals

This refactor must not:

1. Redesign the UI or add new UI features.
2. Change graph/protocol semantics, selection behavior, evidence labels, or authority/projection boundaries.
3. Move typed domain logic out of `ploke_tree` / `ploke_records` into egui modules.
4. Replace existing borrowed/scalar projection patterns with eager owned maps or strings in render paths.
5. Collapse cfg gates. Native-only decoding/tool-contract code must stay `#[cfg(not(target_arch = "wasm32"))]`; benchmark code must stay behind the existing benchmark/dev features.
6. Parallel-edit `shell.rs` from multiple subagents.
7. Do broad formatting, renaming, or cleanup outside the active extraction slice.
8. Treat current warnings as refactor blockers unless the slice directly touches them; final cleanup may remove them.

---

## 2. Success Criteria

### Structural success

- `crates/ploke-egui/src/ui/app/shell.rs` becomes a stable facade plus, at most, small shell/chrome glue.
- Target final `shell.rs` size: under 700 lines. Stretch target: under 400 lines.
- No extracted module should become the new 3k-line dumping ground. Initial soft caps:
  - cache/render helpers: < 500 lines each
  - eval protocol facade: < 900 lines
  - call review scan: < 900 lines
  - run records/tool payloads: < 1,200 lines initially, then split if needed
  - inspector sections: each section module < 800 lines
- `ui/app/mod.rs` and `ui/dashboard/tiles.rs` should continue to import through `crate::ui::app::shell`; callers should not learn child module paths.

### Behavioral success

All existing behavior must remain intact:

- Dashboard panes render the same shell surfaces.
- Pinned inspectors and pinned inspector sections keep working.
- `InspectorPanelSection` titles and serde behavior remain stable for persisted dashboard layouts.
- `InspectorOpenState` still supports benchmark-forced section opening/exclusive opening.
- `InspectorRenderCache` preserves cache keys, rebuild counters, and galley reuse behavior.
- Call-review scan sort/filter/selection behavior remains stable.
- Patch diff scroll areas keep unique IDs and natural heights.
- Artifact ID/source-ref/protocol evidence tracing spans keep their expected fields unless intentionally reviewed.
- Native-only decoded tool argument/result rendering remains unavailable on WASM and available natively.
- Benchmark-only `EvalProtocolRenderMode::CallReviewScanOnly` remains available under the same feature assumptions.

### Validation success

After each extraction slice:

```bash
cargo fmt --all
cargo check -p ploke-egui
cargo test -p ploke-egui --lib
```

For slices touching inspector/cache/render behavior, also run focused tests first, then the full lib suite. Useful focused filters from current tests:

```bash
cargo test -p ploke-egui --lib ui::app::shell::render_cache_tests
cargo test -p ploke-egui --lib patch_diff_galleys_keep_natural_height_when_repeated
cargo test -p ploke-egui --lib artifact_id_section_traces_expected_compact_rows --features native-benchmark
cargo test -p ploke-egui --lib patch_debug_diff_scroll_areas_have_unique_ids --features native-benchmark
```

For any slice that changes visible browser/WASM behavior, add the `ploke-wasm-egui-dogfood` path:

- verify intended Trunk target before claiming it is `ploke-egui`;
- verify listener and HTTP response, not just Trunk success;
- check console/canvas/page title;
- capture screenshot evidence if behavior changed.

---

## 3. Current `shell.rs` Cartography

Approximate responsibility map from the inspected file:

| Lines | Responsibility |
| ---: | --- |
| 1-594 | imports, render/cache structs, text/galley layout helpers, parent-create row cache, inspector open/panel section types |
| 600-1052 | top strip, inspector wrapper/collapsing helpers, scroll padding, bottom timeline |
| 1053-1408 | shared `kv` and cached label/copyable/id/path value helpers |
| 1409-1934 | eval protocol dashboard pane, visual summary, protocol artifact drilldowns, selected call-review state |
| 1980-2941 | call-review scan filters, sorting, row order cache, table/header/row render helpers, scan label/rank/emphasis helpers |
| 2942-3741 | protocol artifact coordinates and typed artifact detail renderers |
| 3742-4335 | run-level LLM trace, request/response/tool call records, observed turn event renderers |
| 4336-4708 | patch generation records, patch artifacts/proposals, expected file changes, patch/evidence labels |
| 4729-4943 | badges, identity, run forest/artifact identity, roles/metrics, parent-create section entry |
| 4944-5190 | run-record section entry and run-record-specific label/value helpers |
| 5191-5272 | graph edges, artifact edges, patch section entries |
| 5273-5834 | candidate comparison, formula summary, metric delta helpers, optional-value renderers |
| 5835-6061 | lineage authority, source refs, artifact IDs, unavailable/fixed/prefixed ID rows |
| 6063-6658 | shell-local tests and benchmark-feature tests |
| 6660-6885 | parent-create attempt renderer and LLM call body/source status |
| 6887-8088 | run record turn summaries, tool steps, decoded tool arguments/results, tool UI payloads, tool kv helpers |
| 8089-8469 | generic agent turns, edges, source refs, patches, patch details, diff rendering, artifact utility helpers |

The central problem is not just file length. It is mixed ownership:

- shell chrome,
- inspector orchestration,
- render cache ownership,
- protocol dashboard,
- table model/sorting,
- typed protocol artifact detail rendering,
- run record/tool payload rendering,
- patch/diff rendering,
- generic reusable label/kv helpers,
- tests for several unrelated subsystems.

The refactor should separate these ownership axes while preserving the public facade.

---

## 4. Target Module Layout

Use a child-module tree under the existing module name so callers keep using `crate::ui::app::shell`:

```text
crates/ploke-egui/src/ui/app/shell.rs                # facade/re-exports + tiny chrome glue only
crates/ploke-egui/src/ui/app/shell/cache.rs          # InspectorRenderCache and cache entries owned by shell
crates/ploke-egui/src/ui/app/shell/chrome.rs         # top strip, bottom timeline, scroll padding
crates/ploke-egui/src/ui/app/shell/inspector.rs      # InspectorOpenState, InspectorPanelSection, right inspector orchestration
crates/ploke-egui/src/ui/app/shell/fields.rs         # kv/cached label/copyable/id/path primitive row helpers
crates/ploke-egui/src/ui/app/shell/eval_protocol.rs  # eval protocol pane/dashboard/drilldown facade
crates/ploke-egui/src/ui/app/shell/call_review.rs    # call-review scan table, sorting/filtering/order cache
crates/ploke-egui/src/ui/app/shell/protocol_detail.rs# typed protocol artifact detail renderers
crates/ploke-egui/src/ui/app/shell/llm_trace.rs      # run-level/request/response/tool-call trace renderers
crates/ploke-egui/src/ui/app/shell/patches.rs        # patch generation, patch details, patch diff rendering
crates/ploke-egui/src/ui/app/shell/identity.rs       # badges, identity, roles/metrics, artifact ids, source refs
crates/ploke-egui/src/ui/app/shell/edges.rs          # graph/artifact/generic edge renderers
crates/ploke-egui/src/ui/app/shell/candidates.rs     # candidate comparison and scoring/metric helpers
crates/ploke-egui/src/ui/app/shell/run_records.rs    # run records, turns, token usage, tool steps/results
crates/ploke-egui/src/ui/app/shell/parent_create.rs  # parent-create attempt and LLM calls
crates/ploke-egui/src/ui/app/shell/tests.rs          # only if tests cannot live in owner modules
```

Notes:

- Prefer tests inside their owner modules (`cache.rs`, `call_review.rs`, `identity.rs`, `patches.rs`) rather than one giant `tests.rs`.
- `crates/ploke-egui/src/ui/render/text.rs` already exists and owns generic cached text kinds/style keys. Do not duplicate those types. Either keep using it or consciously move only shell-specific cache ownership to `shell/cache.rs`.
- `crates/ploke-egui/src/ui/render/mod.rs` currently has an unused `use text::CachedTextKind;`. The final refactor should remove that warning if the touched slice makes it natural, but do not chase it as a first step unless it blocks checks.
- `crates/ploke-egui/src/ui/app/section/mod.rs` is currently empty. Do not build on it blindly. Either remove it in a cleanup slice after confirming no user plan depends on it, or repurpose it only with explicit approval.

---

## 5. Dependency Rules

Use these rules to prevent the new modules from recreating the old knot.

1. `shell.rs` is the public facade.
   - It declares private child modules.
   - It `pub(crate) use`s only the existing stable shell API needed by callers.
   - External callers should not import `shell::cache::...` or `shell::call_review::...`.

2. `cache.rs` is low-level.
   - It may depend on `egui`, `crate::ui::render::text`, `id_display`, and native-only tool-contract decoding behind cfg.
   - It must not depend on section renderers.

3. `fields.rs` is low-level render vocabulary.
   - It may depend on `cache.rs`, `id_display`, and `egui`.
   - It must not depend on `Graph` or inspector section types unless unavoidable.

4. `inspector.rs` orchestrates sections but should not own section internals.
   - It may call public functions from `identity`, `parent_create`, `run_records`, `edges`, `patches`, `candidates`, `eval_protocol`.
   - It owns `InspectorOpenState`, `InspectorPanelSection`, collapsing headers, section popout buttons.

5. Domain-specific modules render their own section body.
   - `eval_protocol.rs` and `call_review.rs` own eval/protocol UI only.
   - `run_records.rs` owns compressed/native record/tool payload rendering.
   - `patches.rs` owns patch debug/diff rendering.
   - `identity.rs` owns identity/artifact ID/source-ref rows.

6. Keep authority boundaries visible.
   - UI modules may project and label `Graph` facts.
   - UI modules must not synthesize authority-bearing facts from CLI text, monitor output, compressed raw records, or ad hoc JSON where typed records exist.

---

## 6. Phased Refactor Plan

Each phase is intended to be behavior-preserving. If a phase fails checks, stop and fix within that phase before moving on.

### Phase A: Freeze Baseline and Protect User Work

Objective: establish the exact starting point before edits.

Steps:

1. Run and save/quote:
   ```bash
   git status --short
   cargo check -p ploke-egui
   cargo test -p ploke-egui --lib
   ```
2. Confirm whether the user wants the existing untracked empty modules kept, removed later, or ignored.
3. Do not create implementation branches or commits unless the user asks.

Acceptance:

- Baseline status and checks are reported.
- Any pre-existing warnings are named as baseline, not attributed to the refactor.

### Phase B: Create the Shell Facade Skeleton

Objective: introduce child module declarations with the smallest possible move.

Candidate first slice:

- Move `EvalProtocolRenderMode`, `InspectorOpenState`, `InspectorPanelSection`, and title/open-state methods to `shell/inspector.rs`.
- In `shell.rs`, re-export:
  ```rust
  mod inspector;
  pub(crate) use inspector::{EvalProtocolRenderMode, InspectorOpenState, InspectorPanelSection};
  ```
- Keep render functions in `shell.rs` initially.

Why first:

- These types are small but used by `app/mod.rs`, `dashboard/tiles.rs`, and benchmark code.
- This validates facade/re-export mechanics before large moves.

Tests:

```bash
cargo fmt --all
cargo check -p ploke-egui
cargo test -p ploke-egui --lib benchmark_inspector_open_state
cargo test -p ploke-egui --lib
```

### Phase C: Extract Render Cache

Objective: isolate cache state and make later render modules share it without importing the entire old shell.

Move to `shell/cache.rs`:

- `InspectorRenderCache`
- `CachedIdGalley`
- native-only `ToolArgumentDecodeEntry`
- native-only `ToolResultDecodeEntry`
- `ParentCreateRowsKey`
- `ParentCreateRows`
- `CallReviewScanOrderCache` and `CallReviewScanOrderKey` if call-review order remains cache-owned; otherwise move those with call-review in Phase F.
- cache methods and tests:
  - `parent_create_render_rows_reuse_allocated_text_for_stable_key`
  - `inspector_text_galley_cache_reuses_stable_labels`
  - `inspector_id_galley_cache_reuses_short_id_labels`
  - `tool_decoded_payload_cache_reuses_stable_records`
  - `text_size_summary_cache_reuses_stable_size_labels`

Keep in or coordinate with `ui/render/text.rs`:

- `CachedTextKind`
- `TextSizeSummaryKey`
- `TextSizeSummaryEntry`
- `CachedTextStyleKey`
- `CachedTextGalley`

Acceptance:

- Cache tests pass.
- No renderer module depends on private internals except through intentional `pub(super)` APIs.

### Phase D: Extract Chrome and Field Helpers

Objective: separate pure UI primitives from domain sections.

Move to `shell/chrome.rs`:

- `render_top_strip`
- `render_bottom_timeline`
- `add_inspector_scroll_end_padding`
- `inspector_scroll_end_padding`

Move to `shell/fields.rs`:

- `kv`
- `cached_label`
- `cached_monospace_label`
- copyable/id/path/run-name helpers
- `render_copyable_text_preview`
- fixed/prefixed id row helpers if they are still generic enough

Acceptance:

- Domain modules can import `fields::*` rather than cloning helper code.
- No behavior changes in labels/copy interactions.

### Phase E: Extract Inspector Orchestration

Objective: make `render_right_inspector` readable as orchestration only.

Move to `shell/inspector.rs`:

- `render_right_inspector`
- section header/collapsing/popout helpers
- `InspectorOpenState`
- `InspectorPanelSection`

Expected shape:

```rust
pub(crate) fn render_right_inspector(...) {
    // select / missing state
    // render high-level sections in order
    // delegate each body to section modules
}
```

Acceptance:

- `dashboard/tiles.rs` still calls `shell::render_right_inspector`.
- Section order and default open/closed behavior remain unchanged.
- Pinned sections still use `InspectorPanelSection` through the shell facade.

### Phase F: Extract Eval Protocol and Call Review

Objective: isolate the largest typed protocol UI island.

Move to `shell/eval_protocol.rs`:

- `render_eval_protocol_for_graph`
- `render_eval_protocol_pane`
- `render_eval_protocol_call_review_scan_for_graph`
- `render_eval_protocol_visual_summary`
- `render_protocol_artifact_drilldowns`
- selected call-review helpers

Move to `shell/call_review.rs`:

- `CallReviewFilter`
- `CallReviewSort`
- `CallReviewSortColumn`
- `SortDirection`
- `CallReviewScanCounts`
- call-review order hash/order builders
- scan table/header/row/hover helpers
- failure/verdict/confidence label/rank/emphasis helpers
- `bounded_utf8_prefix` and its UTF-8 boundary test

Acceptance:

- Call-review scan filtering/sorting works through the same cache key semantics.
- `CallReviewScanOnly` benchmark render mode still compiles under the same feature gates.
- No eager label map generation is added to dashboard construction or render loops.

### Phase G: Extract Protocol Detail and LLM Trace

Objective: separate typed protocol artifacts from run-record traces.

Move to `shell/protocol_detail.rs`:

- protocol artifact coordinate renderer
- tool call review / segment review / intent segmentation / local analysis renderers
- protocol judgment/coverage/turn/call row/payload preview helpers
- copyable multiline body

Move to `shell/llm_trace.rs`:

- run-level LLM trace renderers
- request/response/tool-call record renderers
- observed turn event renderers
- message snapshot / turn finished helpers

Acceptance:

- Typed fields remain typed; do not replace structured rendering with string dumps.
- Missing/unsupported protocol artifacts still render explicit states rather than blank panels.

### Phase H: Extract Section Families

Objective: move each inspector section into the module that owns its vocabulary.

Move to `shell/identity.rs`:

- `render_badges`
- `render_identity`
- `render_run_forest_identity`
- `render_artifact_identity`
- `render_roles_and_metrics`
- metrics helpers
- `render_lineage_authority_for_inspector`
- `render_source_refs_for_inspector`
- `render_artifact_ids_for_inspector`
- artifact ID utility helpers if only identity uses them

Move to `shell/parent_create.rs`:

- `render_parent_create_for_inspector`
- `render_parent_create`
- `render_parent_create_attempt`
- `render_parent_create_llm_calls`
- `render_parent_create_llm_calls_body`
- source status/unavailable helpers
- `AgentTurnSummary` and summary helpers if only parent-create uses them

Move to `shell/run_records.rs`:

- `render_run_records_for_inspector`
- `render_run_records`
- run-record label/value helpers
- run record turns/arms/token usage/tool steps
- tool argument/result decoding and UI payload rendering
- tool kv helpers and path/list/optional helpers

Move to `shell/patches.rs`:

- `render_run_level_patch_generation_for_graph`
- `render_patch_generation_record`
- patch artifacts/proposals/expected file changes
- patch projection counts/status labels
- `render_patches_for_inspector`
- `render_patches`, `render_patch`, `render_patch_details`
- `render_diff`, `render_diff_galley`
- patch diff galley test

Move to `shell/edges.rs`:

- `render_graph_edges_for_inspector`
- `render_artifact_edges_for_inspector`
- generic `render_edges`
- source ref rendering if not kept in identity

Move to `shell/candidates.rs`:

- `render_candidate_comparison_for_inspector`
- formula/candidate renderers
- score/profile/outcome/metric delta helpers
- optional numeric/string render helpers if not shared through `fields.rs`

Acceptance:

- Each moved section has its tests either moved with it or still passing through public facade calls.
- `render_right_inspector` remains a thin orchestrator.

### Phase I: Test Relocation and Cleanup

Objective: prevent test modules from becoming the last giant island.

Move tests near owners:

- cache tests -> `cache.rs`
- call-review UTF-8 preview test -> `call_review.rs`
- tool UI payload rendering test -> `run_records.rs`
- patch diff galley height test -> `patches.rs`
- artifact ID tracing tests -> `identity.rs`
- benchmark open-state tests -> `inspector.rs`
- patch debug scroll area benchmark test -> `patches.rs` or a benchmark-specific test module

Cleanup after all tests pass:

- remove unused imports created by moves;
- remove empty modules only if confirmed stale;
- remove baseline warnings if now local and obvious;
- run final format/check/test.

---

## 7. Orchestration Rules for Implementation

When this spec is executed, use sequential write slices.

For each slice, parent should prepare a subagent packet:

- Scope: one module extraction only.
- Non-goals: no behavior changes, no unrelated cleanup, no formatting outside touched files.
- Write boundary: focused patch touching `shell.rs` and the new target module(s) only.
- Expected handle: changed file list, exact tests run, and output summary.
- Parent join point: verify diff and tests before next slice.

Do not dispatch multiple implementers that edit `shell.rs` concurrently.

Good parallel read-only tasks before writing:

1. Reviewer A: inspect `shell.rs` line ranges and propose extraction dependency order.
2. Reviewer B: inspect tests and map each test to a future owner module.
3. Reviewer C: inspect `dashboard/tiles.rs` and `app/mod.rs` to validate the public facade contract.

The parent must verify any subagent self-report by reading changed files and running tests locally.

---

## 8. Risk Register

| Risk | Why it matters | Mitigation |
| --- | --- | --- |
| Hidden cfg breakage | Native/WASM/benchmark features expose different imports and dead-code paths | Run normal check/test after each slice; run feature-specific tests for benchmark slices; keep cfg attributes with moved items |
| Re-export privacy churn | Public facade types/functions are used by dashboard panes | Keep all external imports through `shell`; re-export deliberately from `shell.rs` |
| Cache behavior regression | Current tests depend on pointer reuse/rebuild counts | Move cache tests with cache before changing cache implementation |
| New mega-module | Moving 8k lines into one `sections.rs` does not solve maintainability | Enforce module soft caps and ownership rules |
| Protocol semantic drift | UI may accidentally treat raw records or strings as authority | Keep typed `Graph`/record access and explicit missing states; no ad hoc JSON parsing in UI |
| Existing user work overwritten | Worktree is dirty with user changes/untracked modules | No reset/clean; inspect status before each slice; patch only scoped files |
| Borrow checker churn causes redesign temptation | Moving functions can expose import/lifetime coupling | Prefer mechanical moves and `pub(super)` helpers; do not redesign APIs in extraction phases |
| Tests depend on private helpers | Moving tests may need changed module visibility | Keep tests in owner modules so they can access private helpers; avoid widening visibility only for tests unless necessary |

---

## 9. First Implementation Slice Recommendation

Start with Phase B, not a large render move.

First slice should move only:

- `EvalProtocolRenderMode`
- `InspectorOpenState`
- `InspectorPanelSection`
- their impl blocks

into:

```text
crates/ploke-egui/src/ui/app/shell/inspector.rs
```

and update:

```text
crates/ploke-egui/src/ui/app/shell.rs
```

with:

```rust
mod inspector;
pub(crate) use inspector::{EvalProtocolRenderMode, InspectorOpenState, InspectorPanelSection};
```

Why this is the right first cut:

- It proves child modules under a file-backed parent module work in this repo.
- It exercises the two external callers (`app/mod.rs`, `dashboard/tiles.rs`) without touching the 6k+ line render body.
- It keeps the first diff reviewable.
- It establishes the facade convention for every later slice.

Expected validation:

```bash
cargo fmt --all
cargo check -p ploke-egui
cargo test -p ploke-egui --lib benchmark_inspector_open_state
cargo test -p ploke-egui --lib
```

---

## 10. Final Handoff Checklist

Before declaring the refactor done:

- [ ] `shell.rs` is under the agreed line-count target.
- [ ] Public shell facade exports only intentional types/functions.
- [ ] Child modules have clear responsibility names.
- [ ] No source module exceeds the agreed soft cap without a written reason.
- [ ] Existing tests pass.
- [ ] New module owners contain their own tests where practical.
- [ ] No new warnings are introduced; baseline warnings are removed or explicitly preserved.
- [ ] `cargo fmt --all` run.
- [ ] `cargo check -p ploke-egui` passes.
- [ ] `cargo test -p ploke-egui --lib` passes.
- [ ] If visible browser/WASM behavior was touched, WASM/Trunk dogfood evidence is captured.
- [ ] Final diff is reviewed for scope creep.
