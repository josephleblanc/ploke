#!/usr/bin/env bash
set -euo pipefail

OUT="${1:-project_context.txt}"

header() { printf "\n==== %s ====\n" "$1" >> "$OUT"; }
# Output mode controls: defaults to mini (capped). Set MODE=full to disable caps.
MODE="${MODE:-mini}"
MAX_LINES="${MAX_LINES:-1200}"
MAX_TREE_LINES="${MAX_TREE_LINES:-200}"
maybe_cap() {
  local n="${1:-$MAX_LINES}"
  if [ "${MODE:-mini}" = "full" ]; then
    cat
  else
    head -n "$n"
  fi
}

: > "$OUT"

header "Metadata"
{
  echo "Timestamp: $(date -Is)"
  echo "Git HEAD: $(git rev-parse --short=12 HEAD 2>/dev/null || echo 'n/a')"
  echo "Branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'n/a')"
} >> "$OUT"

header "Workspace structure (top-level crates, depth=2)"
{ command -v tree >/dev/null 2>&1 && tree -L 2 crates || find crates -maxdepth 2 -print; } 2>/dev/null \
  | sed 's/\x1b\[[0-9;]*m//g' | maybe_cap "$MAX_TREE_LINES" >> "$OUT" || true

header "DB relations referenced in queries (cozo *relation { ... })"
rg -n --no-heading -oP '\*[a-zA-Z_][a-zA-Z0-9_]*\s*\{' crates -g '!target' \
  | sed -E 's/.*\*([a-zA-Z_][a-zA-Z0-9_]*)\s*\{.*/\1/' \
  | sort -u | maybe_cap >> "$OUT" || true

header "Transform schema creation calls (where relations are defined/inserted)"
rg -n --no-heading -e 'create_and_insert_schema|SCHEMA\.create_and_insert' \
  crates/ingest/ploke-transform/src/schema -g '!target' | maybe_cap >> "$OUT" || true

header "ploke-db: Database public API (signatures)"
rg -n --no-heading -e '^\s*pub (struct|enum|trait|fn) ' crates/ploke-db/src/database.rs \
  | sed -E 's/\s{2,}/ /g' | maybe_cap >> "$OUT" || true

header "ploke-db: BM25 indexer surface"
rg -n --no-heading -e 'pub struct Bm25Indexer|impl Bm25Indexer|pub fn (new|is_empty)' \
  crates/ploke-db/src/bm25_index -g '!target' | maybe_cap >> "$OUT" || true

header "Core IDs and hashing (ploke-core)"
rg -n --no-heading -e 'pub (struct|enum|trait) (TypeId|CanonId|PubPathId|TrackingHash|ResolvedId|IdInfo|ItemKind|TypeKind)' \
  crates/ploke-core/src/lib.rs -g '!target' | maybe_cap >> "$OUT" || true

header "Common paths/helpers (workspace_root, fixtures)"
sed -n '/pub fn workspace_root/,/}/p' crates/common/src/lib.rs 2>/dev/null \
  | rg -v '^\s*///' | maybe_cap >> "$OUT" || true
sed -n '/pub fn fixtures_crates_dir/,/}/p' crates/common/src/lib.rs 2>/dev/null \
  | rg -v '^\s*///' | maybe_cap >> "$OUT" || true

header "Parser graph surfaces (types/traits central to code entities)"
rg -n --no-heading -e 'pub (struct|enum|trait) (CodeGraph|GraphAccess|GraphNode|VisibilityKind)' \
  crates/ingest/syn_parser/src/parser/graph -g '!target' | maybe_cap >> "$OUT" || true

header "Parser node IDs and module nodes (AnyNodeId, ModuleNode)"
rg -n --no-heading -e '^(pub )?(enum|struct) (AnyNodeId|ModuleNode)\b' \
  crates/ingest/syn_parser/src/parser/nodes -g '!target' \
  | maybe_cap >> "$OUT" || true

header "TUI Actions/Keymap and Events"
rg -n --no-heading -e 'enum Mode|enum Action|fn to_action|struct MessageUpdatedEvent|trait RenderMsg' \
  crates/ploke-tui/src -g '!target' | maybe_cap >> "$OUT" || true

header "TUI Event bus send surface"
sed -n '/impl EventBus {/,/^\}/p' crates/ploke-tui/src/lib.rs 2>/dev/null \
  | rg -n --no-heading 'pub fn send|pub fn new' | maybe_cap >> "$OUT" || true

header "LLM integration (TUI side)"
rg -n --no-heading -e 'pub (struct|trait|enum) .*Sender|StateCommand|CommandSender|try_send|send' \
  crates/ploke-tui/src/llm -g '!target' | maybe_cap >> "$OUT" || true

header "Embedding services/providers (ingest/ploke-embed)"
rg -n --no-heading -e 'pub (struct|trait|enum) |fn ' crates/ingest/ploke-embed/src/embedding_service.rs 2>/dev/null \
  | rg -n --no-heading 'Embedding|embed|dimensions|vector' | maybe_cap >> "$OUT" || true
rg -n --no-heading -e 'pub (mod|struct|trait|enum)|fn ' crates/ingest/ploke-embed/src/providers -g '!target' \
  | rg -n --no-heading 'openai|hugging|embed|client|provider' | maybe_cap >> "$OUT" || true

header "Indexer/embedding DB API touchpoints"
rg -n --no-heading -e 'update_embeddings_batch|get_unembed_rel|get_embed_rel|count_pending_embeddings|upsert_bm25_doc_meta_batch' \
  crates/ploke-db/src -g '!target' | maybe_cap >> "$OUT" || true

header "Tests: count and notable test modules"
echo "Total #[test] count:" >> "$OUT"
rg -n --no-heading -S '^\s*#\[test\]' -g '!target' | wc -l | tr -d ' ' >> "$OUT" || true
rg -l --no-heading -e 'mod_tree|resolution|uuid_phase|determinism|index|database' crates -g '!target' \
  | maybe_cap >> "$OUT" || true

header "TODO/FIXME (top 30)"
rg -n --no-heading -e 'TODO|FIXME' crates -g '!target' | head -n 30 >> "$OUT" || true

header "Line counts (quick LOC per major crate)"
for d in crates/common crates/ingest/syn_parser crates/ingest/ploke-transform crates/ploke-db crates/ploke-tui crates/ploke-core crates/ingest/ploke-embed; do
  printf "%s: " "$d" >> "$OUT"
  if command -v fd >/dev/null 2>&1; then
    fd -t f -e rs . "$d" 2>/dev/null | xargs wc -l 2>/dev/null | tail -n1 >> "$OUT" || true
  else
    find "$d" -type f -name '*.rs' 2>/dev/null | xargs wc -l 2>/dev/null | tail -n1 >> "$OUT" || true
  fi
done

echo "Wrote $OUT"
