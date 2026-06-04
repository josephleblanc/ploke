#!/usr/bin/env bash
# Theme automation smoke: wasm build + curl + agent URL matrix (no vision required for gate).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CRATE="${ROOT}/crates/ploke-egui"
DIST="${CRATE}/.dist/default"
FIXTURE_PATH="/benchmark-fixtures/protocol-graph.json"
TRAJECTORY_FIXTURE_PATH="/benchmark-fixtures/trajectory-multi-gen.json"
GRAPH_QUERY="graph=${FIXTURE_PATH}"
TRAJECTORY_GRAPH_QUERY="graph=${TRAJECTORY_FIXTURE_PATH}"

echo "==> wasm32 check (ploke-egui)"
cargo check -p ploke-egui --target wasm32-unknown-unknown

echo "==> trunk build"
trunk build --config "${CRATE}/Trunk.toml"

echo "==> curl smoke (.dist/default/)"
test -f "${DIST}/index.html"
curl -sf "file://${DIST}/index.html" | head -c 200 >/dev/null

FIXTURE="${DIST}${FIXTURE_PATH}"
if test -f "${FIXTURE}"; then
  curl -sf "file://${FIXTURE}" | head -c 256 >/dev/null
  echo "OK: fixture present ($(wc -c <"${FIXTURE}") bytes)"
else
  echo "WARN: ${FIXTURE} missing (Trunk copy-dir may not have run)" >&2
fi

TRAJECTORY="${DIST}${TRAJECTORY_FIXTURE_PATH}"
if test -f "${TRAJECTORY}"; then
  curl -sf "file://${TRAJECTORY}" | head -c 256 >/dev/null
  echo "OK: trajectory fixture present ($(wc -c <"${TRAJECTORY}") bytes)"
else
  echo "WARN: ${TRAJECTORY} missing (export from p1-five-gen campaign; see docs/plan/wasm-protocol-dashboard/README.md)" >&2
fi

BASE="http://127.0.0.1:8080"
echo ""
echo "Agent browser loop (after: trunk serve --config ${CRATE}/Trunk.toml):"
echo "  1. ${BASE}/?${GRAPH_QUERY}&theme=tokyo_night"
echo "  2. Hard reload: ${BASE}/?${GRAPH_QUERY}&theme=gruvbox_light"
echo "  3. Multi-gen trajectory: ${BASE}/?${TRAJECTORY_GRAPH_QUERY}&theme=gruvbox_light"
echo "  4. Optional: ${BASE}/?${GRAPH_QUERY}&theme=dracula"
echo ""
echo "Native (no headless): cargo run -p ploke-egui -- --theme gruvbox_light --graph-snapshot <path>"
echo "Unit gate: cargo test -p ploke-egui inspector_text_galley theme_query theme_id"

echo "dogfood-theme-matrix: OK"
