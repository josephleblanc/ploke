# Canonical Resolution Notes: paths_from_id vs resolve_nodes_by_canon_in_file

Goal
- Make canonical-path edits robust across environments and backups by clarifying the DB queries and proposing helpers that avoid brittle absolute-path equality.

Existing Queries

1) paths_from_id (ploke-db/src/get_by_id/mod.rs)
- Input: node UUID (already known).
- Key rules (simplified):
  - has_embedding[node_id, name, hash, span] (embedding must be non-null)
  - parent_of/ancestor graph to find containing module
  - module.path @ NOW → canon_path
  - containing_file uses ancestor + is_file_module to find file_mod.file_path
- Output: name, canon_path (Vec<String>, e.g., ["crate","imports"]), file_path (absolute path from DB)
- Properties:
  - Anchored at a specific ID; returns the DB’s authoritative canon and file path (no equality checks).
  - Requires embeddings via has_embedding; designed for embedded nodes.

2) resolve_nodes_by_canon_in_file (ploke-db/src/helpers.rs)
- Input: relation (e.g., function), abs file path, module_path (["crate", ...]), item_name.
- Key rules (simplified):
  - *{rel}{ id, name, tracking_hash: hash, span @ NOW }
  - ancestor[id, mod_id]
  - *module{ id: mod_id, path: mod_path, tracking_hash: file_hash @ NOW }
  - *file_mod{ owner_id: mod_id, file_path, namespace @ NOW }
  - Filters: name == item_name, file_path == <abs_path>, mod_path == <canon>
- Output: EmbeddingData-like row including file_path, file_hash (module.tracking_hash), node tracking_hash, span, namespace.
- Properties:
  - Independent of embedding; works on parsed rows and module/file relations at NOW.
  - Brittle across environments: absolute `file_path ==` filter requires identical workspace root as when DB was indexed.

Why our canonical E2E fails after restoring backup
- The DB’s stored `file_mod.file_path` is absolute and may not match the current machine’s absolute path. Strict equality zeroes results.
- Even with correct module path and name, the helper returns 0 rows due to the path mismatch.

Proposed Resolvers (robustness plan)

A) Relaxed canonical resolver (module-only)
- Query by (relation, module_path, item_name) w/o `file_path ==` filter.
- Post-filter in app:
  - Normalize candidate `file_path` from DB and the user-supplied `file` against `crate_focus` and chosen symlink policy.
  - Accept candidates whose normalized paths resolve to the same absolute location or whose `file_path` endswith the relative `file`.
- Invariants:
  - If 0 candidates → error with guidance ("no node for canon in module path; check DB and indexing").
  - If >1 candidates → ambiguous (emit count + canon + module path + file candidates list); prefer user-specified absolute match if present.

B) Two-step strict-fallback in tools
- First: run strict `resolve_nodes_by_canon_in_file` (fast path) using absolute file if we can normalize it.
- If 0 rows, run relaxed module-only resolver and filter in Rust.
- On success: proceed with staging WriteSnippetData.

Sketch: Relaxed Query
```
parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}
ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

?[id, name, file_path, file_hash, hash, span, namespace, mod_path] :=
  *{rel}{ id, name, tracking_hash: hash, span @ 'NOW' },
  ancestor[id, mod_id],
  *module{ id: mod_id, path: mod_path, tracking_hash: file_hash @ 'NOW' },
  *file_mod{ owner_id: mod_id, file_path, namespace @ 'NOW' },
  name == <item_name>,
  mod_path == <["crate", ...]>
```

App-side Filter
- If user provided `file` as relative:
  - abs_user = crate_focus.join(file)
  - accept candidate if normalize(candidate.file_path) == normalize(abs_user)
  - or: candidate.file_path.ends_with(file) (fallback)
- If user provided `file` as absolute:
  - normalize both and compare equality

Testing & Diagnostics
- Add DB inspection helper to list `module.path -> file_mod.file_path` pairs for a given canon prefix (e.g., ["crate","imports"]).
- Add regression tests:
  - Canonical resolver finds `crate::imports::use_imported_items` and returns exactly one candidate with a file path in fixture.
  - Fallback succeeds when strict path equality fails.

Temporary Workarounds for Tests
- When using a pre-indexed backup DB, prefer canonical resolution that does not require a fresh index. Avoid strict absolute path equality unless we know the test root equals DB root.

Notes on Embeddings
- `paths_from_id` requires nodes to be embedded (has_embedding). Our canonical edit flow should not depend on embeddings, only parsed spans and module relations.

Next Steps
- Implement `resolve_nodes_by_canon` in ploke-db (no file filter) with identical projections as the strict helper.
- Update `apply_code_edit` canonical tool path to strict→fallback using the relaxed query with app-side path normalization.
- Add instrumentation for 0/1/>1 matches, module path, and candidate file paths to ease debugging.
