# ploke-egui WASM dogfood UX review

**Date:** 2026-06-04 (provenance + inspect popover pass)  
**Reviewer:** Cursor **foreground** session ([`.cursor/rules/ploke-egui-ux-review.mdc`](../../../../.cursor/rules/ploke-egui-ux-review.mdc))  
**Checklist:** [`2026-06-03_ploke-egui-wasm-ux-checklist/README.md`](../2026-06-03_ploke-egui-wasm-ux-checklist/README.md)  
**Style:** [`operator-ui-policy.md`](../../../crates/ploke-egui/docs/style/operator-ui-policy.md) — **§8 Emerging operator preferences** (density, provenance honesty, dogfood process; codifies this review thread)

## 2026-06-04 follow-up (doc themes for next agents)

Policy capture after bar profiles + layout passes—iterate, do not treat as “UX done”:

- **[Bar profiles](../../../crates/ploke-egui/docs/style/operator-ui-policy.md#bar-profiles-segment-lane-family)** — shared `bar_profiles.rs` contract; spotlight tiers are separate; tune via dogfood feedback.
- **Layout patterns** — Analyst Snapshot 140/40 grid + 560/500 hysteresis; footer `TrackLaneLayout` + 6px lane padding; eval status-board cards and flat protocol sections.
- **Known imperfections** — resize band 500–560px, bar hue/gap tuning, sidebar counter semantics, manual inspect-popover QA.
- **Evidence lanes (§7)** — harness vs WASM decode vs snapshot load; inspect popover anchors + jq hints.
- **Dogfood process** — foreground trunk + real screenshots only; see policy §8 dogfood table and [`.cursor/rules/ploke-egui-ux-review.mdc`](../../../../.cursor/rules/ploke-egui-ux-review.mdc).

## Miscommunication note

Sidebar **`selections 0`** / **`artifacts 0`** on `protocol-graph.json` does **not** mean the run is empty — eval/protocol evidence is present (`history_blocks: 0` in fixture). Trajectory generation table and multi-gen `score_child_prop` columns require `?graph=trajectory-multi-gen.json`.

## Environment (this pass)

| Item | Value |
|------|--------|
| Branch / commit | `feature/better-egui` @ `822a5cad` + **uncommitted** inspect-popover edits (`provenance.rs`, wiring) |
| Trunk | Restarted this pass after `trunk build`; `curl` → HTTP 200 |
| Tests | `cargo test -p ploke-egui --lib provenance` → **5 passed**; `render_cache_tests` → **13 passed** |
| `cargo check -p ploke-egui` | OK; **deprecated** `popup_below_widget` / `toggle_popup` warnings |
| Fixture | `/` → `protocol-graph.json` (~7.7 MB) |
| Browser | CDP `Page.captureScreenshot` (MCP `browser_take_screenshot` timed out) |

## Provenance / inspect popover — focused results

| Check | Result | Evidence |
|-------|--------|----------|
| Context strip Source/Host/Decode | **PARTIAL** | Wired in [`llm_trace.rs`](../../../crates/ploke-egui/src/ui/app/shell/llm_trace.rs) before Run LLM Trace; not clearly visible in viewport of [`dogfood-provenance-13-rebuilt-wasm.png`](../../../dogfood-provenance-13-rebuilt-wasm.png) (scroll/crop). |
| `Recorded · tool failed` + **Harness error (recorded)** | **PASS** | Screenshot shows harness headline + TypeTargetPaths text (persisted export, not UI invention). |
| **Inspect provenance** popover (click **i**) | **FAIL** (visual dogfood) | `dogfood-provenance-popover-open.png` is **misnamed** — shows **i** icons + lane chips only, **no** “Error provenance” overlay. Code + 5 unit tests OK; **manual** click on **i** still required to verify UI. |
| `UI · decode (WASM)` + popover trust line | **PARTIAL** | Native-only decode chips in code; not expanded in screenshot session. |
| Theme switch garbled text | **PARTIAL** | Not re-tested this pass; prior pass PASS after early-return fix @ `822a5cad`. |
| Bad `?graph=` snapshot banner | **N/A** | Not exercised (time budget). |

## Perf (this pass)

| Check | Result |
|-------|--------|
| Unit tests (provenance + render cache) | **PASS** |
| Popover path allocations | **PASS** (static strings + copy-on-click only; no graph re-parse) |
| Full benchmark regression | **N/A** (skipped) |
| WASM `check` | **N/A** (not run; native check OK) |

## Prior full checklist (2026-06-04 @ `0b762f28`)

See table below for setup, north star, trajectory, a11y. Still valid except provenance rows above supersede checklist § Error rows for this build.

## Checklist (archive — prior session)

| # | Section | Result | Evidence |
|---|---------|--------|----------|
| 1 | **Setup** | **PASS** | Trunk HTTP 200; `/` loads protocol without picker. |
| 2 | **First impression & north star** | **PARTIAL** | Protocol eval/protocol OK; multi-gen `score_child_prop` all `-` (no formula witnesses). |
| 3 | **Information architecture** | **PARTIAL** | Sidebar `artifacts 0` / `selections 0` misleading on protocol export. |
| 4 | **Trajectory pane** | **PARTIAL** | Multi-gen shell OK; score columns empty (data gap). |
| 5 | **Inspector progressive disclosure** | **PASS** | Failed RCC rows, typed errors, collapsibles. |
| 6 | **Readability & visual design** | **PASS** | Theme tokens, failure red, monospace IDs. |
| 7 | **Performance (perceived)** | **PASS** | Interactive after load; no multi-second inspector stall. |
| 8 | **Error & edge states** | **PASS** | Explicit empty/missing labels. |
| 9 | **Cross-surface parity** | **PASS** | Native smoke + jq align with WASM fixture. |
| 10 | **Accessibility & automation** | **FAIL** | Canvas-only; no widget refs for Call Review / **i** buttons. |
| 11 | **Regression spot-checks** | **PASS** | `render_cache` + theme invalidation tests green. |

## Blockers (unchanged)

1. Multi-gen `score_child_prop` / `trend` need `formula` in export (not UI-only).
2. Sidebar counters on protocol export mislead operators.
3. No WASM load progress for ~8 MB fetch.
4. Canvas-only automation (includes new provenance **i** buttons).

## Nice-to-haves

- Migrate popover off deprecated `popup_below_widget`.
- Wire `render_snapshot_load_banner` (currently unused import warning) or remove dead export.
- Re-capture popover: rename or replace `dogfood-provenance-popover-open.png` (current file does **not** show an open popover).

## Screenshots (provenance pass)

| File | Description |
|------|-------------|
| [`dogfood-provenance-12-protocol-loaded.png`](../../../dogfood-provenance-12-protocol-loaded.png) | Pre-restart trunk capture |
| [`dogfood-provenance-13-rebuilt-wasm.png`](../../../dogfood-provenance-13-rebuilt-wasm.png) | After `trunk build` + reload — **Harness error (recorded)** visible |

Prior: `dogfood-ux-review-10` … `11` at repo root.

## Sign-off

| Field | Value |
| --- | --- |
| Reviewer | Cursor agent (foreground, no Task subagent) |
| Date | 2026-06-04 |
| Fixture | `protocol-graph.json` (`/`) |
| Commit | `822a5cad` + uncommitted inspect popover |
| Viewports | Desktop (~1024×576 browser capture) |

**Verdict:** Provenance labeling for harness failures is **shippable** on protocol dogfood. **Inspect popover** is implemented and test-covered; treat click UX as **manual QA** until automation can hit canvas **i** glyphs or we add AccessKit hooks.
