#!/usr/bin/env bash
# Build the Ploke TUI release binary and install it for the current user.
# Run from the repository root: ./install.sh
# Optional: INSTALL_DIR="$HOME/.cargo/bin" ./install.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found; install Rust stable (https://rustup.rs/)" >&2
  exit 1
fi

cargo build --release

BIN="${ROOT}/target/release/ploke"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

if [[ ! -f "$BIN" ]]; then
  echo "error: expected binary not found: $BIN" >&2
  exit 1
fi

mkdir -p "$INSTALL_DIR"
if [[ -f "$INSTALL_DIR/ploke" ]]; then
  echo "Existing ploke binary found, replacing."
fi
cp -f "$BIN" "$INSTALL_DIR/ploke"
chmod +x "$INSTALL_DIR/ploke"

echo "Installed ploke -> $INSTALL_DIR/ploke"

case ":$PATH:" in
  *:"$INSTALL_DIR":*)
    echo "PATH already includes $INSTALL_DIR"
    ;;
  *)
    echo "Add this directory to PATH, for example:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    ;;
esac

echo ""
echo "To try ploke, open a Rust workspace or crate directory and run: ploke"
