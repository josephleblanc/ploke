# Error Report — Canonical Edit Resolution Failing on fixture_nodes

Summary
- Test: `crates/ploke-tui/tests/e2e_tool_calls.rs::e2e_apply_code_edit_canonical_on_fixture` fails with:
  - `DB resolve failed: Database error: No matching node found for canon=crate::imports::use_imported_items in file=.../tests/fixture_crates/fixture_nodes/src/imports.rs`
- Earlier Cozo error (`Symbol 'file_hash' in rule head is unbound`) was fixed by binding `file_hash` from `*module{ tracking_hash: file_hash @ 'NOW' }` in `crates/ploke-db/src/helpers.rs`.
- After the file_hash fix, the resolver returns zero rows.

Ground Truth
- Canon used: `crate::imports::use_imported_items`
- File path: `<workspace>/tests/fixture_crates/fixture_nodes/src/imports.rs`
- Node type: `function`
- The function is present in the fixture file at module `crate::imports`.

Resolution Query (current)
- Helper: `ploke_db::helpers::resolve_nodes_by_canon_in_file`.
- Cozo script (simplified):
  - Joins primary relation `{rel}{ id, name, tracking_hash: hash, span @ NOW }`
  - `ancestor[id, mod_id]` with `*module{ id: mod_id, path: mod_path, tracking_hash: file_hash @ NOW}`
  - `*file_mod{ owner_id: mod_id, file_path, namespace @ NOW }`
  - Filters `name == item_name`, `file_path == abs_path`, `mod_path == ["crate", ...]`.

Hypotheses
1) File path equality mismatch
   - The imported backup DB may have different absolute paths (e.g., different workspace root). A strict `file_path == abs_abs_path` filter would then eliminate rows.
   - In the passing splice E2E we rely on IoManager hashing against the on-disk file; canonical resolution strictly filters by DB `file_path`.
2) Module path alignment
   - The `mod_path` array may differ in shape (e.g., missing `crate` or different normalization). Less likely given fixtures, but possible depending on parser rules at the time of the backup.
3) Relation classification
   - The function could be recorded under a different relation name or have time qualifiers that differ (`@ NOW`). Unlikely given other queries use the same pattern and succeed.

Immediate Diagnostics Proposed
- Add a targeted DB check in a test-only helper (or a one-off console script) to inspect `*module{ path, tracking_hash }` and `*file_mod{ owner_id, file_path }` for modules with `path == ["crate","imports"]` to see the concrete `file_path` recorded in the backup.
- Add a variant helper `resolve_nodes_by_canon` (no `in_file`) to retrieve candidates by `mod_path + name` only, then filter by file on the application side comparing normalized paths (e.g., root-joined, symlink policy) to increase resilience across environments.

Candidate Fixes
- In `ploke_db::helpers::resolve_nodes_by_canon_in_file`:
  - Make the `file_path` filter optional or looser (e.g., allow `ends_with` comparison against the `crate_focus`-relative path), or
  - Provide two helpers: one WITH file_path and one WITHOUT (use WITHOUT as fallback when WITH returns 0 rows, and then filter the results by normalized on-disk path on the app side).
- In tool call for canonical edits (rag/tools):
  - If the strict helper returns no rows, call the relaxed helper and filter in Rust by comparing normalized absolute paths against `state.system.crate_focus` joined with input `file`.

Why prefer the relaxed fallback
- Strict equality to absolute paths can break across machines since persistence captures the absolute path of the indexer’s workspace.
- Matching by `mod_path + name` and then verifying file at the app-level makes canonical resolution robust to environment differences while still ensuring the correct file is chosen (single candidate expected).

Action Items
1) Implement `resolve_nodes_by_canon` (module-only) in `ploke-db` with the same projection, bound `file_hash` and time semantics.
2) Update `rag/tools.rs` canonical path edit to try `in_file` first, then fallback to module-only + app-side file match.
3) Add a regression test: canonical edit against `fixture_nodes` with a known canon; assert unique match and successful staging.
4) Optional: add a quick DB inspection test that logs `mod_path -> file_path` mappings for `crate::imports` to help diagnose path mismatches in CI.

References
- Helper script fix: `crates/ploke-db/src/helpers.rs`
- Tool path: `crates/ploke-tui/src/rag/tools.rs` (canonical handling)
- Fixture: `tests/fixture_crates/fixture_nodes/src/imports.rs` (function `use_imported_items`)
