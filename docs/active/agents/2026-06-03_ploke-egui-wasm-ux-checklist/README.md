# 2026-06-03 ploke-egui WASM UI/UX review checklist

## Agent execution (required)

Cursor agents must follow [`.cursor/rules/ploke-egui-ux-review.mdc`](../../../../.cursor/rules/ploke-egui-ux-review.mdc) and log results in [`2026-06-03_ploke-egui-wasm-ux-review/README.md`](../2026-06-03_ploke-egui-wasm-ux-review/README.md).

- **Foreground only** — never spawn a background Task subagent for the full checklist (browser MCP + trunk always times out).
- **Write the review README first** (~2 min): environment + empty results table, then fill in.
- **Defaults:** `/` → `protocol-graph.json`; trajectory-only rows → `?graph=trajectory-multi-gen.json`; `selections 0` on protocol export is not an empty run.
- **Always:** `cargo check -p ploke-egui`; `timeout 15 cargo run -p ploke-egui`; targeted `jq` on fixture `records`; read `inspector-ux.md` and `bootstrap.rs`.
- **Trunk:** if `curl -sf http://127.0.0.1:8080/` fails, agent starts trunk in the **parent** session (background; wait for HTTP 200, max ~90s) — do not skip dogfood or mark trunk rows **N/A** because the server was down; never cold-start trunk inside a subagent.
- **Browser:** after trunk is up; ≤2 screenshots, 60s budget; if trunk cannot start, PARTIAL + report blocker (not **N/A** without trying trunk).
- **Complete:** PASS/FAIL/PARTIAL/N/A per section, blockers, sign-off.
- **Screenshots:** tell user: *With trunk on :8080, capture `dogfood-ux-review-NN.png` at repo root.*

**Status:** Active review packet — manual QA for the browser-hosted `ploke-egui` surface.

**Related planning**

- Inspector style contract: [`crates/ploke-egui/docs/style/inspector-ux.md`](../../../crates/ploke-egui/docs/style/inspector-ux.md)
- Eval & Protocol analyst surface plan: [`crates/ploke-egui/docs/plan/eval-protocol-analyst-surface/README.md`](../../../crates/ploke-egui/docs/plan/eval-protocol-analyst-surface/README.md) ([`main-plan.md`](../../../crates/ploke-egui/docs/plan/eval-protocol-analyst-surface/main-plan.md))
- WASM protocol dashboard plan (serve URLs, fixtures): [`crates/ploke-egui/docs/plan/wasm-protocol-dashboard/README.md`](../../../crates/ploke-egui/docs/plan/wasm-protocol-dashboard/README.md)
- Thread handoff: [`2026-05-09_egui-wasm-observability-handoff.md`](../2026-05-09_egui-wasm-observability-handoff.md)

## Purpose

Structured pass for whether the WASM build answers operator questions quickly: trajectory context, eval/protocol evidence, progressive inspector disclosure, and perceived performance. Use after UI changes that touch layout, inspector sections, trajectory table, or render caching.

## How to run this review

1. Ensure the dev server is up on `:8080`. **Agents:** if `curl -sf http://127.0.0.1:8080/` fails, start trunk (do not ask the user to do it first or mark checklist rows **N/A**):

   ```bash
   cd crates/ploke-egui && env -u NO_COLOR trunk serve --config Trunk.toml --address 127.0.0.1 --port 8080
   ```

   Run in background; poll until HTTP 200 (max ~90s). Humans may use the equivalent from repo root: `trunk serve --config crates/ploke-egui/Trunk.toml --address 127.0.0.1 --port 8080`.

2. Open the default fixture (no `?graph=` needed): [http://127.0.0.1:8080/](http://127.0.0.1:8080/) — loads `/benchmark-fixtures/protocol-graph.json` (latest protocol export: Eval & Protocol, call review). Multi-gen trajectory table QA: `?graph=trajectory-multi-gen.json`.

3. Work through the sections below. Capture baseline screenshots at desktop width, then repeat at a narrower viewport. Record the fixture ID (URL param or file basename) in **Sign-off**.

4. When findings conflict with [`inspector-ux.md`](../../../crates/ploke-egui/docs/style/inspector-ux.md) or the eval-protocol plan, note whether the checklist or the doc should win for that item.

---

# ploke-egui WASM UI/UX review checklist

## Setup

- [ ] Hard refresh / first load after `trunk serve` (no stale service worker or cached assets)
- [ ] Default fixture loads without manual file picker (`protocol-graph.json` or equivalent default URL)
- [ ] Desktop viewport (~1280px+): capture baseline screenshots (full shell, trajectory + inspector)
- [ ] Narrower viewport (~900px or tablet width): panes still usable, no critical clipping
- [ ] Note theme (default vs `?theme=`) and any query params used for the session

## First impression & north star

- [ ] Within ~10s of load, an operator can answer: *which generation / child is under review?*
- [ ] `score_child_prop` (or equivalent primary score) is visible without deep drilling
- [ ] Trend / verdict cues read at a glance (not buried in collapsed sections)
- [ ] Empty or missing scores are obvious (not confused with zero or N/A)
- [ ] Overall layout feels like “investigate a run,” not a raw graph debugger

## Information architecture

- [ ] Default layout: graph, trajectory, inspector roles are clear; nothing essential is off-screen without discovery
- [ ] **Trajectory review** preset (if present): restores expected pane weights and focus
- [ ] Pane split: resizing does not trap focus or hide the only path to selection/inspector
- [ ] Selection in graph or trajectory updates inspector predictably (single source of selection truth)
- [ ] **Selection Story** (or equivalent narrative strip): follows selection; readable without opening every inspector section

## Trajectory pane

- [ ] Generation table columns align with mental model (gen id, scores, status, key labels)
- [ ] Hover states communicate row focus; selection highlight is distinct from hover
- [ ] Multi-generation score columns readable (no overlapping headers, truncation explained)
- [ ] Table header sticky or otherwise findable when scrolling long trajectories
- [ ] Verdict / summary line for the selected generation is visible and matches inspector tone

## Inspector progressive disclosure

Work top-to-bottom; collapsed state should communicate status without opening.

- [ ] **Identity** — run/child/node identifiers compact, copyable, correct for selection
- [ ] **Eval Protocol** — primary eval/protocol facts visible collapsed; expand for proof
- [ ] **Call Review / Scan** — warn/error affordances visible when collapsed; detail on expand
- [ ] **Run records** — typed rows, not generic JSON soup; missing vs empty distinguished
- [ ] **Protocol detail** — artifact-oriented drilldown; truncated preview called out per inspector-ux
- [ ] **LLM trace** — long content behind disclosure; scroll stays in-pane
- [ ] **Candidate comparison** — only when applicable; empty state obvious when not
- [ ] Collapsed headers communicate status (pass/warn/error/missing), not `details` / clipped noise
- [ ] Expanding a section scrolls inside the inspector pane (no whole-page jump)
- [ ] Toggle expand/collapse is stable (no flicker, no re-open on unrelated selection tick)

## Readability & visual design

- [ ] Theme tokens consistent (background, borders, accent, semantic warn/error)
- [ ] Monospace used for ids, paths, hashes — proportional for prose
- [ ] Font sizes: table dense but legible; inspector body comfortable for long sessions
- [ ] Contrast sufficient for scores, verdict chips, and disabled/muted labels
- [ ] Long strings truncate with expand/copy; no unbounded horizontal overflow

## Performance (perceived)

- [ ] WASM initial load acceptable on a typical dev machine (spinner/progress if any)
- [ ] Graph pan/zoom/select responsive after load
- [ ] Inspector updates within ~1 frame feel after selection change (no multi-second stall)
- [ ] KV / long trace scroll smooth; no jank when expanding heavy sections
- [ ] No full UI freezes during expand, theme change, or preset switch
- [ ] Theme combobox change (e.g. Tokyo Night → Dracula): no garbled text in inspector, charts, or summary; labels stay readable after one frame (full-frame invalidation + pass-2 repaint)

## Error & edge states

- [ ] No selection: inspector shows helpful empty state, not blank or stale prior content
- [ ] Missing optional fields: labeled (missing / not recorded / N/A), not silent omission
- [ ] Failure-first presentation: errors and rejects visible before success noise
- [ ] Large fixture (`protocol-graph.json` default or `trajectory-multi-gen.json` for trend table): still navigable; note regressions
- [ ] Broken graph URL or fetch failure: clear error, recoverable via file picker or docs link
- [ ] **Provenance lanes** on `protocol-graph.json`: failed tool steps show `Recorded · tool failed` chip + **Harness error (recorded)** headline; `tool call arguments` shows `UI · decode (WASM)` (must not read as harness failure); bad `?graph=` shows top **UI · snapshot load** banner (not sidebar-only)

## Cross-surface parity

- [ ] Same fixture loaded in native `ploke-egui` (if available): selection + inspector facts match WASM
- [ ] Differences only where platform requires (file picker, fonts); document intentional gaps
- [ ] Keyboard shortcuts or nav that exist natively: note if missing in WASM

## Accessibility & automation

- [ ] Keyboard: tab order reaches trajectory and primary inspector actions where implemented
- [ ] Screen reader: note known limits (egui/WASM); don’t claim support that isn’t there
- [ ] Agent dogfood: can an agent complete “select gen N, read verdict, open Call Review” from UI structure alone?
- [ ] Copy actions work for ids and critical evidence strings

## Regression spot-checks

- [ ] **Call Review** warn/error rows still surface severity when collapsed
- [ ] Trajectory select → inspector drilldown path still works for a known multi-gen row
- [ ] **Render cache** / scroll: expand inspector section, scroll, change selection, return — no duplicate content or stale scroll offset bugs

## Sign-off

| Field | Value |
| --- | --- |
| Reviewer | |
| Date | |
| Fixture ID / URL | |
| Commit or branch | |
| Viewports tested | |

**Blockers (must fix before merge / release)**

-

**Nice-to-haves**

-
