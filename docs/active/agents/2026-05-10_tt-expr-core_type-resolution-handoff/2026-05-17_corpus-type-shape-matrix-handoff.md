# 2026-05-17 Corpus Type Shape Matrix Handoff

- Date: 2026-05-17
- Task title: Corpus-backed TypeNode pipeline coverage
- Task description: Shared source-pinned corpus matrix for modeled `syn_parser` TypeNode structures across DB traversal, RAG type-context expansion, direct TUI `request_code_context`, and ignored live tool execution.
- Related planning files: `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/README.md`, `docs/testing/BACKUP_DB_FIXTURES.md`

## Restart Summary

The shared matrix is implemented in `ploke_test_utils::type_shape_matrix`, the focused DB/RAG/TUI matrix tests are green, `cargo test --workspace` is green, and the ignored live OpenRouter model/tool matrix passed when run manually as of this handoff.

The source-pinned corpus fixture set now includes five plain typed graph backups and five OpenRouter-searchable backups:

- `corpus_semver_type_graph_2026-05-17.sqlite`
- `corpus_semver_openrouter_embeddings_2026-05-17.sqlite`
- `corpus_memchr_type_graph_2026-05-17.sqlite`
- `corpus_memchr_openrouter_embeddings_2026-05-17.sqlite`
- `corpus_generic_array_type_graph_2026-05-17.sqlite`
- `corpus_generic_array_openrouter_embeddings_2026-05-17.sqlite`
- `corpus_chrono_type_graph_2026-05-17.sqlite`
- `corpus_chrono_openrouter_embeddings_2026-05-17.sqlite`
- `corpus_axum_type_graph_2026-05-17.sqlite`
- `corpus_axum_openrouter_embeddings_2026-05-17.sqlite`

The searchable corpus fixtures use OpenRouter embeddings:

- provider: `openrouter`
- model: `mistralai/codestral-embed-2505`
- dims: `1536`
- dtype: `f32`
- import mode: `BackupWithEmbeddings`

Important: `tests/backup_dbs/` is ignored by `.gitignore`. If this branch is committed, add the reviewed seed artifacts explicitly with `git add -f tests/backup_dbs/<filename>`.

## Main Files

- `crates/test-utils/src/type_shape_matrix.rs`
  Shared `TypeShapeCase`, `TypeShapeNoTargetCase`, fixture selectors, owner/target selectors, pipeline coverage flags, and live prompts.
- `crates/test-utils/src/fixture_dbs.rs`
  Registry entries for the five plain corpus backups and five OpenRouter searchable variants.
- `crates/ploke-db/tests/unit/type_graph_queries/matrix.rs`
  Strict DB assertions for exact owner, `TypeUseCoordinate`, relation kind, containment depth, terminal target, and no-target fallback rows.
- `crates/ploke-rag/src/core/mod.rs`
  `RagService` now expands both owner-centered and target-centered seeds, and prioritizes an owner hit's direct terminal targets before truncating expanded hits.
- `crates/ploke-rag/src/core/unit_tests.rs`
  Matrix assertions for `Database::expand_type_context` and `RagService::expand_hits_with_type_context`.
- `crates/ploke-tui/src/tools/request_code_context.rs`
  Direct production tool path test plus ignored live OpenRouter matrix test.
- `crates/ploke-tui/tests/integration/workspace_subset_remove.rs`
  Workspace snapshot tests now write current-schema temp snapshots from the registered workspace fixture before exercising strict production snapshot loading.
- `xtask/tests/command_acceptance_db.rs`
  DB command acceptance tests now write a current-schema temp snapshot from the registered fixture before exercising the generic strict `--db` path.
- `docs/testing/BACKUP_DB_FIXTURES.md`
  Current fixture inventory and regeneration contract.

## Coverage State

Positive traversal rows cover:

- named return
- named generic argument
- qualified projection
- reference
- slice
- array
- tuple
- function pointer
- raw pointer
- trait object
- impl trait
- trait bound
- associated type bound
- trait super
- generic parameter bound
- where bound
- where subject
- where generic parameter bound

Explicit no-target / fallback rows cover:

- `never` as an owner/root with no fabricated terminal target
- nested macro type in axum `Token![,]`
- nested parser paren behavior in axum `Option<&(dyn StdError + 'static)>`
- `inferred` and `unknown` as absent fallback relations in the registered corpus backups

Pipeline scope:

- DB matrix asserts every positive row with exact `type_use_id`, role, coordinate, relation kind, depth, and terminal.
- DB no-target matrix walks containment paths and filters by final no-target relation so fan-out containers like `Punctuated<syn::Variant, Token![,]>` stay strict without assuming each edge has one child.
- RAG matrix asserts structured `TypeContextInfo` for rows marked `RagApi`, and materialized `RagService` hit expansion for rows marked `TuiTool`.
- Direct TUI tool test uses `TestRuntime`, production `process_tool`, observes `ToolCallCompleted`, parses `RequestCodeContextResult`, and asserts `ConciseContext.type_context`.
- Ignored live test runs the production LLM manager/tool-dispatch path and asserts tool payload events rather than final model wording.

## Verification

Final focused verification was run by a Spark test-runner subagent with:

```sh
PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs \
  cargo test -p ploke-db corpus_matrix_ -- --nocapture

PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs \
  cargo test -p ploke-rag corpus_type_shape_matrix -- --nocapture

PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs \
  cargo test -p ploke-tui --features test_harness request_code_context_tool_emits_matrix_type_context -- --nocapture
```

All three passed:

- `ploke-db`: `3 passed; 0 failed`
- `ploke-rag`: `1 passed; 0 failed`
- `ploke-tui`: `1 passed; 0 failed`

Additional verification:

```sh
PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs \
  cargo test -p ploke-db -- --nocapture

PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs \
  cargo test -p xtask --test command_acceptance_db -- --nocapture

PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs \
  cargo test --workspace

PLOKE_DB_SNAPSHOT_FIXTURE_DIR=/home/brasides/code/agent-dir/ploke/tests/backup_dbs \
  cargo test -p ploke-tui --features test_harness \
    live_request_code_context_matrix_uses_production_tool_payload -- --ignored --nocapture
```

These passed:

- `ploke-db`: `115 passed; 0 failed; 14 ignored`
- `xtask` command acceptance: `3 passed; 0 failed`
- workspace: passed with no failed test targets; some tests remain ignored
- ignored live OpenRouter matrix: `1 passed; 0 failed; 0 ignored; 216 filtered out`; it did not skip for missing `OPENROUTER_API_KEY`

`cargo fmt --all` was also run after the final code changes.

## Notes For Next Agent

Do not demote matrix rows or broaden selectors to make corpus tests pass. If a matrix row fails, inspect the owner selector, source-pinned fixture membership, materialization boundary, or production search seed.

Do not make backup loading permissive. If a fixture is stale, regenerate or repair it through the registry-backed fixture commands instead of tolerating schema drift.

The live matrix test may spend provider credits. It should stay ignored and explicitly gated by `OPENROUTER_API_KEY`; run it manually when live provider verification is intended rather than including it in default verification.

## Cold Restart Checklist

1. Re-read this file, `docs/testing/BACKUP_DB_FIXTURES.md`, and `crates/test-utils/src/type_shape_matrix.rs`.
2. Confirm the ten `2026-05-17` corpus SQLite backups exist in the active fixture directory or in `tests/backup_dbs/`.
3. Run the focused DB/RAG/TUI matrix commands above before touching broader parser/transform code.
4. If broader confidence is needed after new changes, rerun `cargo test --workspace` through a Spark test-runner subagent.
5. Treat the many existing dirty files as branch work in progress; do not revert unrelated changes.
