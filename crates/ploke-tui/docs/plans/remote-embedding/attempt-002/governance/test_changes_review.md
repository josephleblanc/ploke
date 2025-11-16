# Test Changes Pending Review (Attempt 002)

This note tracks the **legacy** tests touched while enabling the dual fixture strategy so we can audit them before advancing beyond Phase B. All locations assume repository root.

---

## `crates/ploke-db/src/helpers.rs` (unit tests)

1. **`test_resolve_nodes_by_canon_in_file_via_paths_from_id`**
   - **Change (2025-11-15):** The temporary skip guard has been replaced with a hard assertion that fails when `get_common_nodes()` yields zero rows. This forces fixture regeneration to backfill embedded nodes before the test suite can pass.
   - **Reasoning:** Fixture verification now covers both legacy and multi-embedding backups; keeping this assertion ensures we notice if either backup loses the expected embedded rows.
   - **Status:** ✅ Restored to strict behavior. No outstanding warnings.

2. **`test_relaxed_fallback_when_file_mismatch`**
   - **Change (2025-11-15):** Same fix as the previous test—the skip guard is gone and we now assert non-empty fixture rows.
   - **Reasoning:** Keeps regression coverage intact when fixtures change.
   - **Status:** ✅ Restored to strict behavior. No outstanding warnings.

## `crates/ploke-db/src/utils/test_utils.rs`

- **Change:** The helper now selects the fixture backup based on feature flags (`LEGACY_FIXTURE_BACKUP_REL_PATH` vs `MULTI_EMBED_FIXTURE_BACKUP_REL_PATH`) instead of always loading the legacy file. No assertions were changed, but this affects which database underlying tests exercise.
- **Reasoning:** Allows `cargo test -p ploke-db --features multi_embedding_experiment` to run against the schema-tagged backup while legacy builds continue using the old one.

## `crates/ploke-db/src/bm25_index/mod.rs`, `crates/ploke-db/src/index/hnsw.rs`, `crates/ploke-db/benches/resolver_bench.rs`

- **Change:** Each module now loads the fixture backup via the shared path constants. No functional assertions changed; only setup paths were updated so benches and tests can target the correct database per feature flag.

---

### Warning Summary
- _None._ All helper tests now enforce embedded-node availability again.
