# WASM Protocol Dashboard Plan

This directory contains the gated implementation plan for making `ploke-egui` the browser/WASM-facing Prototype 1 protocol/debug frontend.

- [`main-plan.md`](main-plan.md): task breakdown, gate matrix, and execution workflow.

The plan deliberately treats `ploke-tree-egui` and `PlaybackBrowserModel` as prior art only. The implementation source boundary is `&ploke_tree::Graph` plus typed graph-owned projections.

## WASM Trunk shell

The operator UI ships its own Trunk config at [`crates/ploke-egui/Trunk.toml`](../../../Trunk.toml) (not the workspace-root [`Trunk.toml`](../../../../../Trunk.toml), which still builds `ploke-tree-egui`).

```bash
cargo build -p ploke-egui --target wasm32-unknown-unknown
trunk build --config crates/ploke-egui/Trunk.toml
trunk serve --config crates/ploke-egui/Trunk.toml --address 127.0.0.1 --port 8080
```

### Loading a graph snapshot in the browser

Trunk copies `benchmark-fixtures/` into `dist/benchmark-fixtures/` (see `index.html` `copy-dir`).

| Method | How |
|--------|-----|
| Default (no query) | Open `http://127.0.0.1:8080/` — startup fetches the standard fixture automatically. |
| Query param | `http://127.0.0.1:8080/?graph=/benchmark-fixtures/standard-prototype1-graph-snapshot.json` |
| Alias basename | `http://127.0.0.1:8080/?graph=standard-prototype1-graph-snapshot.json` (maps to the path above) |
| File picker | Left nav → **Load graph snapshot (.json)…** — pick any export-graph JSON from disk |

**Does not work in WASM:** typing a host filesystem path (e.g. `/home/...` or `crates/ploke-egui/...`) — the browser cannot read those; use the file picker or a same-origin `?graph=` URL. Errors appear in red under the graph catalog controls.

**Note:** Missing fixture URLs still return HTTP 200 from Trunk (HTML shell); the app detects HTML/invalid JSON and shows an explicit error instead of failing silently.
