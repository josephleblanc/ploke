# ploke-egui design autonomy

**Date:** 2026-06-03  
**Title:** Autonomous egui design workflow (theme + parallel design branches)  
**Description:** Repeatable agent loop for `ploke-egui` UI work: wasm check, native benchmark gate, Trunk dogfood, MCP browser screenshots. Phase 1 lands shared theme on `feature/better-egui`; Phase 2 forks parallel design worktrees.

**Related:**

- Cursor plan: `.cursor/plans/autonomous_egui_design_5e129f9d.plan.md`
- Theme tokens: [`crates/ploke-egui/docs/style/theme.md`](../../../crates/ploke-egui/docs/style/theme.md)
- Inspector UX: [`crates/ploke-egui/docs/style/inspector-ux.md`](../../../crates/ploke-egui/docs/style/inspector-ux.md)
- Dogfood scripts: [`dogfood-smoke.sh`](../../../crates/ploke-egui/scripts/dogfood-smoke.sh), [`dogfood-theme-matrix.sh`](../../../crates/ploke-egui/scripts/dogfood-theme-matrix.sh)

## Autonomous loop

1. Implement UI slice (surgical diff; no per-frame graph clone/alloc).
2. `cargo check -p ploke-egui --target wasm32-unknown-unknown`
3. `cargo test -p ploke-egui --features "dev,native-benchmark" benchmark_regression` (blocking before commit)
4. `./crates/ploke-egui/scripts/dogfood-smoke.sh` (wasm + trunk build + curl; no vision)
5. Optional: `trunk serve` + MCP browser pass (see checklist below)
6. Commit on green gate

Escalation: `cargo test -p ploke-egui --features "dev,native-benchmark" benchmark_regression_against_baseline -- --ignored` — **do not update baseline JSON without explicit approval**.

## Fixture URL (WASM dogfood)

After `trunk serve --config crates/ploke-egui/Trunk.toml`:

`http://127.0.0.1:8080/?graph=/benchmark-fixtures/standard-prototype1-graph-snapshot.json`

(Use `protocol-graph.json` when that export is copied into `benchmark-fixtures/`. Default startup uses the standard snapshot.)

## Theme ids (automation)

Valid `?theme=` and `--theme` values (snake_case):

| ID | Label |
|----|-------|
| `tokyo_night` | Tokyo Night (default) |
| `dracula` | Dracula |
| `gruvbox_dark` | Gruvbox Dark |
| `one_dark` | One Dark |
| `gruvbox_light` | Gruvbox Light |
| `one_light` | One Light |

Examples:

- WASM: `http://127.0.0.1:8080/?graph=/benchmark-fixtures/standard-prototype1-graph-snapshot.json&theme=gruvbox_light` (hard reload to switch palette without using the canvas combo)
- Native: `cargo run -p ploke-egui -- --theme tokyo_night --graph-snapshot /path/to/snapshot.json`

Script: [`crates/ploke-egui/scripts/dogfood-theme-matrix.sh`](../../../crates/ploke-egui/scripts/dogfood-theme-matrix.sh)

## Concurrent Trunk ports (Phase 2 worktrees)

When multiple worktrees serve simultaneously, assign distinct ports to avoid collisions:

| Worktree / branch | Suggested port |
|-------------------|----------------|
| Base / `feature/better-egui` | 8080 |
| `design/analyst-dashboard` | 8081 |
| `design/trace-grid` | 8082 |
| `design/density-polish` | 8083 |

Example: `trunk serve --config crates/ploke-egui/Trunk.toml --port 8081`

## MCP browser screenshot checklist

Run after graph load (non-zero canvas; wait for fixture fetch if using `?graph=`).

1. **Eval Protocol** — dashboard eval pane, verdict mix / analyst surfaces visible.
2. **Call Review** — scan table + spotlight when a row is selected.
3. **Agent Trace** — LLM trace pane with turn list.
4. **Timeline** — bottom playback timeline with colored segments.
5. **Palette smoke** — at least one dark palette (e.g. Tokyo Night) and one light (e.g. Gruvbox light).

Per-branch captures (Phase 2): `docs/active/agents/ploke-egui-design-autonomy/captures/<branch-name>/`.

**Canvas tips:** Lock browser tab before automation; scroll with focus on `#ploke-operator-canvas` + PageDown; confirm no console errors after theme switch.

## Phase boundaries

- **Phase 1 (this branch only):** theme module, token migration, benchmark gate — no surface redesign beyond theme selector.
- **Phase 2:** `git worktree add` from theme tip; do not fork palette definitions on design branches.

## Do not commit

`.dist/` (Trunk output), legacy `dist/` / `dist-*`, `.embed-measure/`, local `Trunk.*` / `index.*` asset/embed variants (see `.gitignore`), large `protocol-graph.json` exports (~12 MB), screenshots, `web.rs.bak.final`.
