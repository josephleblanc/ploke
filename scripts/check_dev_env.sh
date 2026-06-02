#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mode="${1:-quick}"

case "$mode" in
  quick|fixtures|verify)
    ;;
  -h|--help|help)
    cat <<'EOF'
Usage: scripts/check_dev_env.sh [quick|fixtures|verify]

quick     Check local tool availability and lightweight repo state.
fixtures  Stage generated DB fixtures required by the default test suite.
verify    Run quick checks plus fixture staging and Rust parity gates used by CI.
EOF
    exit 0
    ;;
  *)
    echo "unknown mode: $mode" >&2
    exit 2
    ;;
esac

need_cmd() {
  local name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "missing required command: $name" >&2
    return 1
  fi
}

print_version() {
  local label="$1"
  shift
  printf '%-12s ' "$label"
  "$@" 2>/dev/null | head -n 1 || true
}

echo "repo: $repo_root"
echo "mode: $mode"

need_cmd git
need_cmd cargo
need_cmd rustc

if [[ "$mode" != "fixtures" ]]; then
  need_cmd rustup
  need_cmd gh
  need_cmd node
  need_cmd npm
  need_cmd pnpm
  need_cmd rg
  need_cmd zig
  need_cmd cc
  need_cmd ar
  need_cmd ranlib
fi

print_version git git --version
print_version rustc rustc --version
print_version cargo cargo --version

if [[ "$mode" != "fixtures" ]]; then
  print_version node node --version
  print_version npm npm --version
  print_version pnpm pnpm --version
  print_version gh gh --version
  print_version zig zig version
fi

echo "branch: $(git branch --show-current)"
echo "remote: $(git remote get-url origin)"

if [[ "$mode" != "fixtures" ]]; then
  if ! gh auth status >/dev/null 2>&1; then
    echo "gh auth status failed" >&2
    exit 1
  fi

  if ! rustup target list --installed | rg -q '^wasm32-unknown-unknown$'; then
    echo "missing rust target: wasm32-unknown-unknown" >&2
    exit 1
  fi

  if [[ ! -r "$HOME/.cargo/config.toml" ]]; then
    echo "missing $HOME/.cargo/config.toml" >&2
    exit 1
  fi

  if [[ ! -x "$HOME/.config/ploke-agent/github-askpass" ]]; then
    echo "missing executable GitHub askpass helper" >&2
    exit 1
  fi

  pnpm --dir /tmp dlx gitnexus --version >/dev/null
fi

if [[ "$mode" != "fixtures" ]]; then
  echo "quick checks passed"
fi

stage_fixtures() {
  local snapshot_dir="${PLOKE_DB_SNAPSHOT_FIXTURE_DIR:-$HOME/.config/ploke/db_snapshot_fixtures}"
  local typed_date
  typed_date="$(date +%F)"

  mkdir -p "$snapshot_dir"

  local checkout_local_fixtures=(
    fixture_nodes_canonical
    fixture_nodes_local_embeddings
    ploke_db_primary
    ws_fixture_01_canonical
    ws_fixture_01_member_single
  )

  local fixture
  for fixture in "${checkout_local_fixtures[@]}"; do
    local matches=("$repo_root/tests/backup_dbs/local/${fixture}__root-"*.sqlite)
    if [[ ! -e "${matches[0]}" ]]; then
      cargo xtask recreate-backup-db --fixture "$fixture"
    fi
  done

  echo "staging plain typed graph snapshots"

  local typed_graph_fixtures=(
    corpus_semver_type_graph
    corpus_memchr_type_graph
    corpus_generic_array_type_graph
    corpus_chrono_type_graph
    corpus_axum_type_graph
  )

  for fixture in "${typed_graph_fixtures[@]}"; do
    local registered_path="$snapshot_dir/${fixture}_2026-05-17.sqlite"
    if [[ ! -f "$registered_path" ]]; then
      cargo run -p xtask --features typed_type_graph -- recreate-backup-db --fixture "$fixture"
      cp "$snapshot_dir/${fixture}_${typed_date}.sqlite" "$registered_path"
    fi
  done

  local memchr_rev="24f5daa5257e00e87007c936761600e034827905"
  local memchr_cache="$snapshot_dir/_source_cache/corpus/checkouts/BurntSushi__memchr/$memchr_rev"
  local memchr_expected="$repo_root/tests/fixture_github_clones/corpus/BurntSushi__memchr"

  if [[ ! -d "$memchr_cache" ]]; then
    cargo run -p xtask --features typed_type_graph -- recreate-backup-db --fixture corpus_memchr_type_graph
    cp "$snapshot_dir/corpus_memchr_type_graph_${typed_date}.sqlite" "$snapshot_dir/corpus_memchr_type_graph_2026-05-17.sqlite"
  fi

  if [[ -d "$memchr_cache" ]]; then
    mkdir -p "$(dirname "$memchr_expected")"
    ln -sfn "$memchr_cache" "$memchr_expected"
  fi

  cargo xtask setup-fixtures
}

if [[ "$mode" == "fixtures" ]]; then
  stage_fixtures
  echo "fixtures staged"
  exit 0
fi

if [[ "$mode" == "verify" ]]; then
  stage_fixtures
  cargo fmt --all -- --check
  cargo check --workspace
  cargo clippy --all-targets -- -D warnings
  cargo test --workspace
fi
