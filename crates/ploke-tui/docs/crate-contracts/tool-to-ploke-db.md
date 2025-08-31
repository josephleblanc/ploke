# Contract: Tools ↔ ploke-db

This document defines the expectations (inputs/outputs, invariants, and failure modes) shared between the ploke-tui tool-calling layer and the ploke-db layer. It focuses on the DB records and queries required by each tool so we can debug mismatches methodically.

Scope
- Tools: request_code_context, apply_code_edit (canonical + splice), get_file_metadata (DB: none).
- Supporting flows: change scan, indexing (BM25 + dense embeddings), and path normalization.

## Common Data and Semantics

Relations and fields (logical view; actual schema lives in ploke-db):
- Primary node relations (per kind): `function`, `struct`, `enum`, `trait`, `type_alias`, `const`, `static`, `module`, `macro`, `impl`, `import`.
  - Fields (for nodes used by code edits/context):
    - `id: Uuid`
    - `name: String`
    - `tracking_hash: TrackingHash` (aka `hash` in queries)
    - `span: (start_byte, end_byte)` (byte offsets; UTF-8 boundaries)
    - `embedding: <F32; D>` (nullable; present iff embedded)
- Module/file linkage:
  - `module.path: Vec<String>` (canonical module path, e.g. `["crate","imports"]`)
  - `module.tracking_hash: TrackingHash` (module-scope token hash at time)
  - `file_mod.owner_id: module_id` and `file_mod.file_path: String` (absolute path)
  - `file_mod.namespace: Uuid` (project/namespace id)

Time semantics:
- Most queries reference the NOW view with `@ 'NOW'`. The database is versioned; consumers must specify time where required.

Path normalization and crate root:
- ploke-db stores absolute `file_path` strings.
- ploke-tui may pass relative paths in UI/tool payloads. The tool layer resolves relative paths against `state.system.crate_focus` to produce absolute paths before querying DB or IO.

Tracking hashes (invariants):
- Node `tracking_hash` is a token-based hash of the item’s content at time NOW.
- Module `tracking_hash` (used in queries as `file_hash`) represents the module/file scope token hash, used to sanity-check that spans are current.
- IO verification (`IoManagerHandle::read_full_verified`) uses token-based tracking hash to ensure edits are applied to the expected content version.

Change detection and indexing:
- `ScanForChange` updates parsed rows (spans, node presence) by reparsing changed files; does not compute embeddings.
- `IndexerTask::index_workspace` updates embeddings for DB rows missing `embedding` (unembedded set), and also emits BM25 updates.
- `request_code_context` relies on non-null embeddings (dense) and updated BM25 to return meaningful results.

## Tool Contracts

### 1) request_code_context
- Input (canonical request in tools): `{ token_budget: u32, hint?: String }`.
- Expectation on DB:
  - Embeddings must be present (`embedding` non-null) for nodes intended to be returned as context.
  - BM25 should be seeded for sparse/rrf hybrid; otherwise only dense search contributes.
  - Spans must be current to retrieve correct code snippets.
- Retrieval invariants:
  - Selecting top-k nodes uses a hybrid of BM25 and dense search; selection returns per-node metadata including `file_path`, `span`, and `namespace`.
  - The tool/LLM layer may later issue verified IO reads to present code; token hash mismatches should be treated as stale data and reported.
- Failure modes:
  - Empty/degenerate results when embeddings are missing or BM25 not seeded.
  - Runtime DB errors are surfaced as `ToolCallFailed` with descriptive messages.

### 2) apply_code_edit (two modes)

Mode A: Canonical
- Input (typed):
  ```json
  {
    "edits": [
      {
        "mode": "canonical",
        "file": "src/module.rs" | "/abs/path/src/module.rs",
        "canon": "crate::path::Item",
        "node_type": "function" | "struct" | ...,
        "code": "replacement code"
      }
    ]
  }
  ```
- Resolution contract:
  - The tool resolves `file` to an absolute path using `crate_focus` if needed.
  - The tool calls a DB helper to resolve nodes by canonical module path + simple name within the specified file.
    - Current strict helper: `resolve_nodes_by_canon_in_file(db, relation, abs_file_path, module_path=["crate", ...], item_name)`.
  - Expected DB shape:
    - `module.path == ["crate", ...]` must match the canon’s modules.
    - `file_mod.file_path == abs_file_path` must match the file.
    - `module.tracking_hash` (as `file_hash`) exists at NOW.
    - Node exists in the given primary relation with `span` and `tracking_hash` at NOW.
  - Output from helper: `EmbeddingData { id, name, file_path, file_tracking_hash, tracking_hash (node), span, namespace }`.
- Edit staging contract:
  - For a single match: build a `WriteSnippetData` from resolved `span`, `file_path`, `file_tracking_hash`, and `replacement`.
  - Resolve to an in-memory proposal with preview (diff/codeblock) and emit `ToolCallCompleted` with an `ApplyCodeEditResult` summary.
  - If 0 rows: return failure with a helpful message (`No matching node found for canon=... in file=...`).
  - If >1 rows: return failure indicating ambiguous resolution.

Mode B: Splice
- Input (typed or legacy):
  ```json
  {
    "edits": [
      {
        "mode": "splice",
        "file_path": "/abs/path",
        "expected_file_hash": "<TrackingHash>",
        "start_byte": 0,
        "end_byte": 10,
        "replacement": "...",
        "namespace": "<Uuid>"  // optional (defaults to project)
      }
    ]
  }
  ```
- Expectations on DB:
  - None for staging; IO verification uses `expected_file_hash` (tokenized) and fails if mismatched.
  - Spans are implicit (caller-specified).

### 3) get_file_metadata
- DB expectations: None.
- The tool reads the file bytes via tokio fs, computes tracking hash (v5 token hash of content), returns `{ ok, file_path, exists, byte_len, modified_ms, file_hash, tracking_hash }`.

## Where Mismatches Typically Occur

- Absolute path equality: If a restored DB was created under a different workspace root, `file_mod.file_path` may not equal the current absolute path. Strict equality in `resolve_nodes_by_canon_in_file` causes 0 matches.
- Module path shape: Canonical module vectors must include the leading `"crate"`; any discrepancy causes 0 matches.
- Time semantics: queries must include `@ 'NOW'` where appropriate or may read stale states.
- Indexing completeness: Without embeddings/BM25, RAG queries degenerate; canonical edits do not depend on embeddings but depend on spans and module/file relationships being current.
- Tracking hash polymorphism: IO uses token-based `TrackingHash`. In tests, use tokenized hashes (via syn parse → token stream) rather than v5 bytes hash from a raw read.

## Recommendations and Helpers

1) Provide a relaxed canonical resolver
- Add `resolve_nodes_by_canon(db, relation, module_path, item_name)` (no file filter) and filter results in Rust by normalized file path (using `crate_focus` + relative path). Use as fallback if the strict in-file resolver returns 0 matches.
- Note: Both strict and relaxed helpers must resolve file path via `file_owner_for_module` (self-or-ancestor with `file_mod`) to support nested modules.

2) Add DB inspection helpers/tests
- Small queries/utilities to list:
  - `module.path -> file_mod.file_path` mappings for a given canon (e.g., `crate::imports`).
  - Presence of `span` and `tracking_hash` at NOW for a node.

3) Document resolution error messages
- Standardize messages for 0-match and >1-match cases to aid debugging.

4) Reindex strategy in tests
- Prefer using pre-indexed fixture backups for canonical-path tests; only run full indexing in dedicated tests. Baking absolute path sensitivity into the DB requires consistent roots.

## Appendix: Current Helper (Strict)

`resolve_nodes_by_canon_in_file` (simplified):
```
parent_of[child, parent] := *syntax_edge{..., relation_kind: "Contains" @ 'NOW' }
ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
module_has_file_mod[mid] := *file_mod{ owner_id: mid @ 'NOW' }
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{ id, name, tracking_hash: hash, span @ 'NOW' },
  ancestor[id, mod_id],
  *module{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' },
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{ owner_id: file_owner_id, file_path, namespace @ 'NOW' },
  name == <item_name>,
  file_path == <abs_path>,
  mod_path == <["crate", ...]>
```
parent_of[child, parent] := *syntax_edge{..., relation_kind: "Contains" @ 'NOW' }
ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{ id, name, tracking_hash: hash, span @ 'NOW' },
  ancestor[id, mod_id],
  *module{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' },
  *file_mod{ owner_id: mod_id, file_path, namespace @ 'NOW' },
  name == <item_name>,
  file_path == <abs_path>,
  mod_path == <["crate", ...]>
```
