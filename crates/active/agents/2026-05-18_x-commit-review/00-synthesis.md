# Synthesis

Scope: commit `bf870c84` (`x`) reviewed against `bf870c84^`, with the current
tree at `2f5c6b06` after `crates/ploke-db/src/bm25_index/mod.rs` was reverted.

## Findings

1. High: the direct `request_code_context` matrix test is now failing after the
   BM25 rollback.

   `03-rag-tui-tooling.md` reproduced:

   ```text
   cargo test -p ploke-tui --features typed_type_graph,test_harness request_code_context_tool_emits_matrix_type_context -- --nocapture
   ```

   The failure was on `named_generic_argument_chrono_weekday_set_single_day`,
   returning one snippet and no `type_context`.

   Parent correction: the production clamp
   `calc_top_k_for_budget(token_budget).min(cfg.rag.top_k)` at
   `crates/ploke-tui/src/tools/request_code_context.rs:141` already existed
   before `bf870c84`. The regression introduced by `x` is that the new direct
   and live matrix tests force `cfg.rag.top_k = 1` at
   `request_code_context.rs:294` and `request_code_context.rs:429`, making the
   test depend on BM25 ranking selecting exactly the intended seed. That is the
   same pressure that led to the reverted BM25 ranking hack.

2. Medium: the new TUI payload assertions are too loose.

   `05-test-quality.md` flags `request_code_context.rs:534-556`: matching a
   target by substring in `canon_path` or `snippet` is not unique enough for
   labels like `T`, `Item`, or `Map`. The assertion should compare resolved
   target UUIDs or a stricter canonical path/selector-derived identity.

3. Medium: the ignored live test can accept the wrong tool completion.

   `05-test-quality.md` and `03-rag-tui-tooling.md` both flag that
   `request_code_context.rs:483-528` waits for any parseable
   `RequestCodeContextResult` payload instead of tying `ToolCallCompleted` back
   to the specific requested `call_id`.

4. Medium: fixture loading now has silent seed fallback.

   `04-fixtures-docs-xtask.md` flags
   `crates/test-utils/src/fixture_dbs.rs:692-766`. `fresh_backup_fixture_db`
   now calls `backup_fixture_path_or_seed`, which can fall back to committed
   seed DBs when the shared snapshot directory is absent. That conflicts with
   the docs at `docs/testing/BACKUP_DB_FIXTURES.md:71-85`, which describe
   `fresh_backup_fixture_db` as loading from the shared snapshot directory.

   If this behavior is needed for isolated probe clones, make it an explicit
   helper or opt-in mode; keep the default helper strict.

5. Low/needs decision: `RagService::expand_hits_with_type_context` changed
   production expansion behavior.

   `crates/ploke-rag/src/core/mod.rs:618-681` now expands every seed as both
   `Owner(seed_id)` and `Target(seed_id)`, and then prioritizes
   owner-terminal targets ahead of score ordering. This may be a reasonable
   retrieval design, but it is a production ranking/expansion policy change and
   should be justified independently from the failing `top_k = 1` tests.

## Clear Areas

- Parser/transform/type-relation review found no correctness regressions.
  Focused parser test passed:
  `cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture`.
- DB/type-graph review found no concrete correctness regression in the scoped
  production changes. Remaining gaps are unproven alternate-path behavior in
  `TypeTargetPaths` and ordering ties that omit `type_use_id`.
- The BM25 directory is already restored to the parent of the bad commit and
  should stay that way.

## Recommended Recovery Order

1. Keep the BM25 rollback.

2. Fix or quarantine the TUI matrix tests before trusting them:
   - remove the forced `cfg.rag.top_k = 1`;
   - use normal production defaults or a test-specific nonproduction entrypoint
     that explicitly starts from resolved owner IDs;
   - assert target identity by UUID, not label substring;
   - tie live `ToolCallCompleted` to the specific `call_id`.

3. Decide whether the RAG production expansion changes in
   `expand_hits_with_type_context` are desired. If kept, add focused tests that
   exercise `RagService::get_context`, not only helper expansion.

4. Make fixture seed fallback explicit or revert it. The default registered
   fixture helpers should not silently change authority from shared snapshot
   fixtures to committed seed files.

5. After those fixes, rerun focused tests:

   ```text
   PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs cargo test -p ploke-db --features typed_type_graph corpus_matrix_ -- --nocapture
   PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs cargo test -p ploke-rag --features typed_type_graph corpus_type_shape_matrix -- --nocapture
   PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs cargo test -p ploke-tui --features test_harness,typed_type_graph request_code_context_tool_emits_matrix_type_context -- --nocapture
   ```

