#!/usr/bin/env bash
# Non-interactive WASM dogfood: wasm check, trunk build, curl smoke on .dist/default/.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
CRATE="${ROOT}/crates/ploke-egui"
DIST="${CRATE}/.dist/default"

echo "==> wasm32 check (ploke-egui)"
cargo check -p ploke-egui --target wasm32-unknown-unknown

echo "==> trunk build"
trunk build --config "${CRATE}/Trunk.toml"

echo "==> curl smoke (.dist/default/)"
test -f "${DIST}/index.html"
curl -sf "file://${DIST}/index.html" | head -c 200 >/dev/null

FIXTURE="${DIST}/benchmark-fixtures/protocol-graph.json"
if test -f "${FIXTURE}"; then
  curl -sf "file://${FIXTURE}" | head -c 256 >/dev/null
  echo "OK: fixture present ($(wc -c <"${FIXTURE}") bytes)"
else
  echo "WARN: ${FIXTURE} missing (Trunk copy-dir may not have run)" >&2
fi

TRAJECTORY="${DIST}/benchmark-fixtures/trajectory-multi-gen.json"
if test -f "${TRAJECTORY}"; then
  curl -sf "file://${TRAJECTORY}" | head -c 256 >/dev/null
  echo "OK: trajectory fixture present ($(wc -c <"${TRAJECTORY}") bytes)"
else
  echo "WARN: ${TRAJECTORY} missing (export from p1-five-gen campaign; see docs/plan/wasm-protocol-dashboard/README.md)" >&2
fi

echo "dogfood-smoke: OK"
