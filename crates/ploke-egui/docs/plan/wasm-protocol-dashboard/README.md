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

### Startup fetch flow (no WASM embed)

On WASM startup, `web.rs` calls `GraphCatalog::enqueue_startup_graph_load()` once. That path:

1. Reads `?graph=` from `window.location.search` (see `bootstrap.rs::graph_query_url_from_search`).
2. Rejects host/repo paths (`/home/...`, `crates/ploke-egui/...`, bare `benchmark-fixtures/...`) with a red error — the browser cannot read those.
3. Maps the query value to a fetch URL via `resolve_graph_fetch_url` (same-origin path, `http(s)://`, or basename alias).
4. If there is no `?graph=`, fetches the default fixture URL `/benchmark-fixtures/protocol-graph.json`.
5. `fetch` runs async; when bytes arrive, `apply_pending_graph()` replaces the placeholder sample graph.

Trunk must copy JSON into `.dist/default/` beside the WASM bundle (see `Trunk.toml`). Default `index.html` uses `copy-dir` for `benchmark-fixtures/` only.

| Constant / alias | Value |
|------------------|-------|
| Default fetch URL | `/benchmark-fixtures/protocol-graph.json` |
| Multi-gen trajectory fixture | `/benchmark-fixtures/trajectory-multi-gen.json` |
| Basename alias | `?graph=protocol-graph.json` → same default URL |
| Same-origin path | `?graph=/benchmark-fixtures/my-export.json` (file must exist under `.dist/default/`) |
| External URL | `?graph=https://example.com/export.json` (server must send CORS headers) |

| Method | How |
|--------|-----|
| Default (no query) | Open `http://127.0.0.1:8080/` — startup fetches the protocol graph fixture. |
| Query param | `http://127.0.0.1:8080/?graph=/benchmark-fixtures/protocol-graph.json` |
| Alias basename | `http://127.0.0.1:8080/?graph=protocol-graph.json` |
| File picker | Left nav → **Load graph snapshot (.json)…** — pick any export-graph JSON from disk |

**Does not work in WASM:** host filesystem paths (`/home/...`, `/tmp/...`, `crates/ploke-egui/...`) — use the file picker or a served `?graph=` URL. Errors appear in red under the graph catalog controls.

**Note:** Missing fixture URLs still return HTTP 200 from Trunk (HTML shell); the app detects HTML/invalid JSON and shows an explicit error instead of failing silently.

### Loading `.temp/protocol-graph.json` (or any export) via fetch

The browser only fetches URLs Trunk serves under `.dist/` (default build: `.dist/default/`). The default startup fixture is `protocol-graph.json` under `benchmark-fixtures/` (copy or symlink from `.temp/` if missing locally). For other exports, pick one:

**Option A — copy or symlink into `benchmark-fixtures/` (default Trunk config)**

```bash
cd crates/ploke-egui
ln -sf ../../.temp/protocol-graph.json benchmark-fixtures/protocol-graph.json
# or: cp /path/to/your-export.json benchmark-fixtures/my-graph.json

trunk serve --config Trunk.toml --address 127.0.0.1 --port 8080
```

Open: `http://127.0.0.1:8080/?graph=/benchmark-fixtures/protocol-graph.json`

**Option B — `graph-assets/` + asset-both Trunk config**

Local symlinks under `graph-assets/` (already used in this crate) plus `Trunk.asset-both.toml` / `index.asset-both.html` copy both `graph-assets/` and `benchmark-fixtures/`:

```bash
mkdir -p crates/ploke-egui/graph-assets
ln -sf /path/to/.temp/protocol-graph.json crates/ploke-egui/graph-assets/protocol-graph.json

trunk serve --config crates/ploke-egui/Trunk.asset-both.toml --address 127.0.0.1 --port 8080
```

Open: `http://127.0.0.1:8080/?graph=/graph-assets/protocol-graph.json`

**Option C — external URL**

If the export is hosted with CORS enabled:

`http://127.0.0.1:8080/?graph=https://your-host/export-graph.json`

### Quick reference

| Goal | Command | URL |
|------|---------|-----|
| Protocol graph at startup | `trunk serve --config crates/ploke-egui/Trunk.toml` | `http://127.0.0.1:8080/` |
| Multi-gen trajectory dogfood | same `trunk serve` | `http://127.0.0.1:8080/?graph=/benchmark-fixtures/trajectory-multi-gen.json&theme=gruvbox_light` |
| Standard benchmark snapshot | same `trunk serve` | `http://127.0.0.1:8080/?graph=/benchmark-fixtures/standard-prototype1-graph-snapshot.json` |
| Other custom export at startup | Add file under served `benchmark-fixtures/` or `graph-assets/` (options A/B) | `http://127.0.0.1:8080/?graph=/benchmark-fixtures/my-graph.json` |
| One-off local file | same `trunk serve` | file picker in left nav |

### `trajectory-multi-gen.json` (multi-generation fixture)

Checked-in export of campaign `p1-five-gen-1x3-20260516-1` (`STANDARD_RUN_ROOT` in `benchmark.rs`): five generations, six history blocks, selection-rich data for trajectory UI dogfood. Same bytes as `standard-prototype1-graph-snapshot.json` when exported from the same run root.

**Regenerate locally** (requires eval-home run root):

```bash
cargo run -p ploke-egui --features dev -- export-graph \
  --run-root ~/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 \
  --output crates/ploke-egui/benchmark-fixtures/trajectory-multi-gen.json
```

**Fresh clone without eval-home:** symlink or copy from an existing export, or from `standard-prototype1-graph-snapshot.json` (same campaign):

```bash
cd crates/ploke-egui/benchmark-fixtures
ln -sf standard-prototype1-graph-snapshot.json trajectory-multi-gen.json
# or symlink the live run root export after running export-graph on your machine
```

`dogfood-smoke.sh` and `dogfood-theme-matrix.sh` warn if this file is missing from `.dist/default/` after `trunk build`.
