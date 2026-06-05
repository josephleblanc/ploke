# ploke-egui agent notes

**UI changes:** follow [`docs/style/operator-ui-policy.md`](docs/style/operator-ui-policy.md) (canonical operator/analyst policy; **§8** emerging preferences: evidence lanes, density, tone, dogfood).

**Operator bars:** timeline, Analyst Snapshot metrics, and protocol verdict rows use the shared **segment-lane bar profile** family in [`src/ui/bar_profiles.rs`](src/ui/bar_profiles.rs) (`MetricFill`, `SegmentLane`, `StackedOutcome`, `EmptyTrack`). Tune gaps, gamma fills, and lane padding there after dogfood—not ad-hoc painters. Eval spotlight under-bars are **verdict emphasis tiers**, not `BarProfile`—see policy **§8 Bar profiles** and **Known imperfections**.

## Analyst Snapshot resize (manual)

1. Narrow inspector → single column; labels, counts, and bars stay aligned (no overlap).
2. Widen past **~560px** → 2×2 sections; bars stay in their row (e.g. “recoverable detour”).
3. Shrink below **~500px** → back to single column without flicker at the boundary.
4. **500–559px** should remain single column (hysteresis band).

## WASM dogfood defaults

- Default graph (no `?graph=`): `/benchmark-fixtures/trajectory-multi-gen.json` (`trajectory-multi-gen.json`) — see `STANDARD_GRAPH_SNAPSHOT_FIXTURE_*` in `src/bootstrap.rs`.
- Eval/protocol export: `?graph=protocol-graph.json` → `/benchmark-fixtures/protocol-graph.json` (`PROTOCOL_GRAPH_SNAPSHOT_*`).
- Dev server: `trunk serve --config crates/ploke-egui/Trunk.toml --address 127.0.0.1 --port 8080` → [http://127.0.0.1:8080/](http://127.0.0.1:8080/).
- **WASM dogfood:** if `curl -sf http://127.0.0.1:8080/` fails, start trunk (`cd crates/ploke-egui && env -u NO_COLOR trunk serve --config Trunk.toml --address 127.0.0.1 --port 8080` in background; wait for HTTP 200, max ~90s) — do not skip dogfood or mark trunk rows **N/A**.

Sidebar **`selections 0`** on the protocol export does not mean an empty run; default `/` loads the multi-gen trajectory fixture. Use `?graph=protocol-graph.json` for eval/protocol-centric checklist items.

## UI/UX reviews

- Checklist: [`docs/active/agents/2026-06-03_ploke-egui-wasm-ux-checklist/README.md`](../../docs/active/agents/2026-06-03_ploke-egui-wasm-ux-checklist/README.md)
- Latest run log: [`docs/active/agents/2026-06-03_ploke-egui-wasm-ux-review/README.md`](../../docs/active/agents/2026-06-03_ploke-egui-wasm-ux-review/README.md)
- Agent playbook (foreground only; no background browser subagents): [`.cursor/rules/ploke-egui-ux-review.mdc`](../../.cursor/rules/ploke-egui-ux-review.mdc)
- Inspector contract: [`docs/style/inspector-ux.md`](docs/style/inspector-ux.md)

## Native vs WASM review split

- **WASM / browser:** checklist sections that need layout, panes, inspector disclosure, and screenshots — start trunk if `:8080` is down, then browser MCP in the **parent** session (`curl -sf http://127.0.0.1:8080/` → HTTP 200).
- **Native:** `cargo check -p ploke-egui`, `timeout 15 cargo run -p ploke-egui`, and parity notes vs WASM for the same fixture basenames.
