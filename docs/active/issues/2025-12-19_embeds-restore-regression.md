**Context**
- Workflow: pick non-default embedding set (e.g., `mistralai/codestral-embed-2505`), index crate, `/save db`, exit, restart TUI with default embedding set, `/load <crate>`.
- On load we log/emit: “No populated embedding set found after restore; embedding searches will be unavailable,” even though vectors and an HNSW index were present in the backup.
- A later attempt can succeed (fallback to first populated set), so data exists; the first restore attempt is missing it.

**Repro**
- Manual: follow the workflow above.
- Automated: `cargo test -p ploke-tui load_db_restores_saved_embedding_set_and_index -- --nocapture` (currently fails with the warning).

**Expected vs Actual**
- Expected: load restores the saved embedding set (or first populated), builds HNSW for that set, and search works without warnings.
- Actual: initial load reports “No populated embedding set found…” and aborts embedding search setup.

**Evidence (logs)**
- Source: `crates/ploke-tui/logs/ploke.log.2025-12-18`
- Extraction: `rg -n "No populated embedding set" crates/ploke-tui/logs/ploke.log.2025-12-18`
- Example snippet:
  - `... Attempting to load code graph for ploke-db ...`
  - `WARN No populated embedding set found after restore; embedding searches will be unavailable`

**Hypotheses**
- Crate focus mismatch on load (lookup under wrong crate key) so metadata/populated set not found.
- Active embedding metadata not persisted/read for the crate before backup/restore.
- Counting embeddings against the wrong set/relation during restore.

**Next Actions**
- Verify crate_name/crate_focus handling on save/load and ensure it matches metadata lookup.
- Confirm `put_active_embedding_set_meta` is written before backup and restored on load.
- Inspect `restore_embedding_set` path: ensure it counts embeddings for the correct set and crate; add targeted logging if needed.
- Ensure HNSW creation uses the restored set and is invoked after restore.
- Re-run `cargo test -p ploke-tui load_db_restores_saved_embedding_set_and_index -- --nocapture` to validate once fixed.

**New Finding (Cozo import constraints)**
- `Db::import_from_backup` ([docs](https://docs.rs/cozo/latest/cozo/struct.Db.html#method.import_from_backup)) only imports rows into relations that already exist and have no indices; triggers/callbacks do not fire.
- Because the per-embedding-set vector relations are not known until after reading `embedding_set`, the restored DB was missing the vector relation for the non-default set, so its vectors imported as zero rows and restore fell back to “no populated set.”

**Proposed Fix Approach**
- Two-pass import from the same backup:
  1) Import only the `embedding_set` relation (base schema already exists).
  2) For each imported set, `ensure_vector_embedding_relation` (and drop any HNSW if present).
  3) Import the remaining relations, including all vector relations, now that they exist and have no indices.
  4) Rebuild HNSW indices for the restored set via `create_index_for_set`.
- Keep current selection order (metadata → first populated) once vector rows import correctly.
