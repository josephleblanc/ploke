# Test matrix for `xtask` commands

**Date:** 2026-03-25  
**Task spec:** [PRIMARY_TASK_SPEC.md](./PRIMARY_TASK_SPEC.md) section E  
**Branch:** `feature/xtask-commands`

## Canonical location

Per PRIMARY_TASK_SPEC §E.1, this file under `docs/active/agents/2026-03-24-coordination/` is the **primary** test matrix for coordination and agent workflow.

The PRIMARY_TASK_SPEC milestone M.3.2 also mentions `xtask/tests/test_matrix.md`. That path holds a **short pointer** to this document only (no duplicate matrix). If tooling is added under `xtask/tests/`, extend this file and link the new tests here.

---

## Legend (per-test columns)

| Column | Meaning |
|--------|---------|
| **Spec** | PRIMARY_TASK_SPEC section(s) or [design/test-design-requirements.md](./design/test-design-requirements.md). |
| **Verified** | Last `cargo test` date and outcome for the `xtask` crate (`PASS` / `FAIL`). |
| **Gate** | **Behavior-gated** — fails (or panics from `todo!`) until listed implementation exists; passing implies the asserted behavior is present. **Structural** — can pass without full M.4 agent command bodies (types, serde, pure helpers). **Gap-signal** — intentionally passes while a feature is missing (e.g. expects panic); replace with a behavior-gated test when the feature lands. **Weak** — passes but does not prove the full behavior implied by the name/docs (called out in Notes). |
| **Unblocks green** | Minimal commands / modules / behavior that must be correct for this test to pass *as its gate type implies*; `N/A` for Structural or Gap-signal where not applicable. |
| **Notes** | Edge cases, overlap with other tests, or misleading coverage. |

**Weak / gap-signal highlights:** `load_fixture_with_index_flag` (message-only “index” today; docstring states scope; see [`LoadFixture`](../../../../xtask/src/commands/db.rs)), `registry_factory_panics_until_command_construction_implemented` (Gap-signal). `executor_tracks_usage` now asserts `UsageTracker::total_command_count` (still does not parse JSONL contents line-by-line).

---

## Command acceptance index

Maps each agent-facing [`Cli`](../../../../xtask/src/cli.rs) subcommand to the test(s) that define **basic done-ness** for that command: fixture/input, then concrete assertions (not `is_ok()` alone). Negative cases are listed where they are the dedicated acceptance test for that command.

| CLI command | Primary acceptance test(s) | Fixture / input | Expect (summary) |
|-------------|----------------------------|-----------------|------------------|
| `parse discovery` | [`discovery_finds_cargo_toml`](../../../../xtask/tests/parse_commands.rs), [`discovery_error_missing_cargo_toml`](../../../../xtask/tests/parse_commands.rs) | `tests/fixture_crates/fixture_nodes`; invalid dir for error | `crates_found > 0`; error + recovery on bad path |
| `parse phases-resolve` | [`acceptance_parse_phases_resolve_success_fixture_nodes`](../../../../xtask/tests/command_acceptance_parse.rs), [`acceptance_parse_phases_resolve_rejects_missing_path`](../../../../xtask/tests/command_acceptance_parse.rs) | `tests/fixture_crates/fixture_nodes`; missing absolute path | `PhaseResult`: `success`, `nodes_parsed > 0`, `relations_found > 0`; error + §D recovery |
| `parse phases-merge` | [`phases_merge_produces_merged_graph`](../../../../xtask/tests/parse_commands.rs) | `tests/fixture_crates/fixture_nodes` | Same `PhaseResult` shape |
| `parse workspace` | [`workspace_parses_all_crates`](../../../../xtask/tests/parse_commands.rs) | workspace fixture paths in test | Successful workspace parse output |
| `parse stats` | [`stats_returns_accurate_counts`](../../../../xtask/tests/parse_commands.rs) | fixture path in test | `Stats` output with non-zero totals |
| `parse list-modules` | [`list_modules_finds_all_modules`](../../../../xtask/tests/parse_commands.rs) | fixture path in test | Non-empty `ModuleList` |
| `db save` | [`save_creates_valid_backup`](../../../../xtask/tests/db_commands.rs) | isolated `FIXTURE_NODES_CANONICAL` | Backup file created |
| `db load` | [`load_restores_backup_correctly`](../../../../xtask/tests/db_commands.rs) | isolated fixture + backup path | DB restores |
| `db load-fixture` | [`load_fixture_loads_valid_fixture`](../../../../xtask/tests/db_commands.rs), [`load_fixture_rejects_invalid_id`](../../../../xtask/tests/db_commands.rs) | registered fixture ids | Success + path; validation error + recovery |
| `db load-fixture --index` | [`load_fixture_with_index_flag`](../../../../xtask/tests/db_commands.rs) | `fixture_nodes_local_embeddings` | **Weak:** success message mentions index/HNSW (no DB index proof yet) |
| `db count-nodes` | [`count_nodes_returns_nonzero_for_populated_db`](../../../../xtask/tests/db_commands.rs) | isolated `FIXTURE_NODES_CANONICAL` | `total > 0`, sum by kind |
| `db query` | [`query_executes_valid_cozoscript`](../../../../xtask/tests/db_commands.rs), [`query_handles_invalid_syntax`](../../../../xtask/tests/db_commands.rs) | fixture DB | Rows returned; error includes query context |
| `db stats` | [`stats_returns_comprehensive_data`](../../../../xtask/tests/db_commands.rs) | fixture DB | `DatabaseStats` populated |
| `db list-relations` | [`acceptance_db_list_relations_success`](../../../../xtask/tests/command_acceptance_db.rs), [`acceptance_db_list_relations_with_counts`](../../../../xtask/tests/command_acceptance_db.rs) | isolated `FIXTURE_NODES_CANONICAL` | Non-empty `RelationsList`; `--counts` → at least one `row_count: Some` (not all names support count query) |
| `db embedding-status` | [`acceptance_db_embedding_status_success`](../../../../xtask/tests/command_acceptance_db.rs) | isolated `FIXTURE_NODES_CANONICAL` | `total_nodes > 0`; `embedded == total_nodes.saturating_sub(pending)` |
| `db hnsw-build` | [`acceptance_db_hnsw_build_panics_until_implemented`](../../../../xtask/tests/command_acceptance_db.rs) | N/A | **Gap-signal:** `catch_unwind` until `todo!` removed |
| `db hnsw-rebuild` | [`acceptance_db_hnsw_rebuild_panics_until_implemented`](../../../../xtask/tests/command_acceptance_db.rs) | N/A | **Gap-signal** |
| `db bm25-rebuild` | [`acceptance_db_bm25_rebuild_panics_until_implemented`](../../../../xtask/tests/command_acceptance_db.rs) | N/A | **Gap-signal** |
| `help-topic` | (none) | — | Optional future: capture stdout |

---

## Scope and status summary (by file)

| Test source | PRIMARY_TASK_SPEC areas | Role | Verified |
|-------------|-------------------------|------|----------|
| [cli_invariant_tests.rs](../../../../xtask/tests/cli_invariant_tests.rs) | C.1, C.2, C.3 | `xtask::cli::Cli` clap help and errors | 2026-03-25 PASS |
| [context_tests.rs](../../../../xtask/tests/context_tests.rs) | A.1–A.4 prep, architecture | `CommandContext`, DB pool, fixtures | 2026-03-25 PASS |
| [db_commands.rs](../../../../xtask/tests/db_commands.rs) | A.4, E, D | `db` commands via executor + fixtures | 2026-03-25 PASS |
| [error_tests.rs](../../../../xtask/tests/error_tests.rs) | D, C (indirect) | `XtaskError`, recovery, `ploke_error` mapping | 2026-03-25 PASS |
| [executor_tests.rs](../../../../xtask/tests/executor_tests.rs) | M.2/M.3/M.4 infra, C (indirect) | Executor, registry, `MaybeAsync` | 2026-03-25 PASS |
| [parse_commands.rs](../../../../xtask/tests/parse_commands.rs) | A.1, E, D | `parse` commands via `syn_parser` | 2026-03-25 PASS |
| [command_acceptance_parse.rs](../../../../xtask/tests/command_acceptance_parse.rs) | A.1, E, D | Command-level acceptance: `phases-resolve` | 2026-03-25 PASS |
| [command_acceptance_db.rs](../../../../xtask/tests/command_acceptance_db.rs) | A.4, E, D | Command-level acceptance: `list-relations`, `embedding-status`, HNSW/BM25 gap-signal | 2026-03-25 PASS |
| Crate unit tests under [`xtask/src/`](../../../../xtask/src/) | E, D, infra | Faster checks on types, errors, usage, harness | 2026-03-25 PASS |

**Design reference:** [design/test-design-requirements.md](./design/test-design-requirements.md)

---

## Integration tests: [`cli_invariant_tests.rs`](../../../../xtask/tests/cli_invariant_tests.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `cli_root_help_shows_parse_and_db` | C.3 | 2026-03-25 PASS | Behavior-gated | `Cli` clap: top-level help lists `parse` and `db` | Does not exercise legacy [`main.rs`](../../../../xtask/src/main.rs) dispatch. |
| `cli_parse_help_lists_subcommands` | C.3 | 2026-03-25 PASS | Behavior-gated | `parse` subcommand help text | Asserts `discovery` substring. |
| `cli_db_help_lists_subcommands` | C.3 | 2026-03-25 PASS | Behavior-gated | `db` subcommand help text | Asserts `count` substring. |
| `cli_parse_discovery_help_documents_path_target` | C.2, C.3 | 2026-03-25 PASS | Behavior-gated | `parse discovery` args/help mention path target | Loose match on `PATH` / `path`. |
| `cli_unknown_subcommand_produces_clap_error` | C.1 | 2026-03-25 PASS | Behavior-gated | clap error output for unknown argv | Non-empty feedback only. |

---

## Integration tests: [`context_tests.rs`](../../../../xtask/tests/context_tests.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `context_can_be_created` | Architecture | 2026-03-25 PASS | Behavior-gated | `CommandContext::new` | |
| `context_implements_default` | Architecture | 2026-03-25 PASS | Structural | `Default` for `CommandContext` | |
| `context_lazy_initializes_database_pool` | A.4 prep | 2026-03-25 PASS | Behavior-gated | `CommandContext::database_pool`, `ploke_db` | |
| `context_provides_in_memory_database` | A.4 prep | 2026-03-25 PASS | Behavior-gated | In-memory DB path on context | |
| `context_lazy_initializes_embedding_runtime` | A.3 prep | 2026-03-25 PASS | Behavior-gated | `embedding_runtime()` wiring | |
| `context_provides_io_manager` | Architecture | 2026-03-25 PASS | Behavior-gated | IO manager accessor | |
| `context_detects_workspace_root` | A.1–A.4 | 2026-03-25 PASS | Behavior-gated | Workspace discovery from crate root | |
| `context_caches_workspace_root` | A.1–A.4 | 2026-03-25 PASS | Behavior-gated | Stable `workspace_root()` | |
| `context_provides_temp_dir` | Architecture | 2026-03-25 PASS | Behavior-gated | Temp dir per context | |
| `context_validates_resources` | Architecture | 2026-03-25 PASS | Behavior-gated | `validate_resources` | |
| `context_handles_resource_errors` | D | 2026-03-25 PASS | Behavior-gated | Resource validation error paths | |
| `context_rejects_missing_backup_file_path` | D, A.4 | 2026-03-25 PASS | Behavior-gated | Backup path validation + recovery | |
| `contexts_have_independent_temp_dirs` | Architecture | 2026-03-25 PASS | Behavior-gated | Isolation between contexts | |
| `contexts_share_workspace_root` | Architecture | 2026-03-25 PASS | Behavior-gated | Shared workspace assumption | |
| `context_is_thread_safe` | Architecture | 2026-03-25 PASS | Behavior-gated | `Send`/`Sync` or cross-thread use | |
| `context_creation_error_handling` | D | 2026-03-25 PASS | Behavior-gated | Construction error surfaces | |
| `context_handles_double_initialization` | Architecture | 2026-03-25 PASS | Behavior-gated | Idempotent or safe double init | |

---

## Integration tests: [`db_commands.rs`](../../../../xtask/tests/db_commands.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `count_nodes_returns_nonzero_for_populated_db` | A.4, E | 2026-03-25 PASS | Behavior-gated | `db count-nodes`, fixture copy, cozo queries | Uses `FIXTURE_NODES_CANONICAL`. |
| `count_nodes_with_kind_filter` | A.4 | 2026-03-25 PASS | Behavior-gated | `CountNodes` + kind filter | |
| `count_nodes_with_pending_flag` | A.4 | 2026-03-25 PASS | Behavior-gated | `CountNodes --pending` | |
| `query_executes_valid_cozoscript` | A.4 | 2026-03-25 PASS | Behavior-gated | `db query` + valid script | |
| `query_handles_invalid_syntax` | A.4, B.1, D | 2026-03-25 PASS | Behavior-gated | `db query` error path + query context | |
| `query_with_parameters` | A.4 | 2026-03-25 PASS | Behavior-gated | `db query` param binding | |
| `stats_returns_comprehensive_data` | A.4 | 2026-03-25 PASS | Behavior-gated | `db stats` | |
| `stats_with_category_filter` | A.4 | 2026-03-25 PASS | Behavior-gated | `db stats` category | |
| `save_creates_valid_backup` | A.4 | 2026-03-25 PASS | Behavior-gated | `db save` + `ploke_db::Database` backup | |
| `load_restores_backup_correctly` | A.4 | 2026-03-25 PASS | Behavior-gated | `db load` | |
| `load_fixture_loads_valid_fixture` | A.4 | 2026-03-25 PASS | Behavior-gated | `db load-fixture` + registered fixture | |
| `load_fixture_rejects_invalid_id` | A.4, D | 2026-03-25 PASS | Behavior-gated | Fixture id validation + recovery | |
| `load_fixture_with_index_flag` | A.4 | 2026-03-25 PASS | **Weak** | `LoadFixture` success message path | Docstring matches scope: message reflects `--index`; no DB index assertion until implementation. |
| `count_nodes_with_db_path` | A.4 | 2026-03-25 PASS | Behavior-gated | `CountNodes --db` | |
| `query_with_mutable_flag` | A.4 | 2026-03-25 PASS | Behavior-gated | `db query --mutable` | |
| `backup_restore_roundtrip` | A.4 | 2026-03-25 PASS | Behavior-gated | `save` + `load` consistency | |

---

## Integration tests: [`error_tests.rs`](../../../../xtask/tests/error_tests.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `error_new_creates_generic_error` | D | 2026-03-25 PASS | Structural | `XtaskError` variant shape | Overlaps [`error.rs`](../../../../xtask/src/error.rs) unit tests. |
| `error_internal_creates_internal_error` | D | 2026-03-25 PASS | Structural | Internal variant | |
| `error_validation_creates_validation_builder` | D | 2026-03-25 PASS | Structural | Validation builder | |
| `error_validation_with_recovery` | D | 2026-03-25 PASS | Structural | Recovery on validation | |
| `error_from_string` | D | 2026-03-25 PASS | Structural | `From<String>` | |
| `error_from_str` | D | 2026-03-25 PASS | Structural | `From<&str>` | |
| `error_from_io_error` | D | 2026-03-25 PASS | Structural | `From<io::Error>` | |
| `error_from_json_error` | D | 2026-03-25 PASS | Structural | `From<serde_json::Error>` | |
| `error_with_context_wraps_error` | D | 2026-03-25 PASS | Structural | `with_context` | |
| `error_with_context_chain` | D | 2026-03-25 PASS | Structural | Chained context | |
| `error_recovery_suggestion` | D | 2026-03-25 PASS | Structural | `recovery_suggestion()` | |
| `recovery_hint_creation` | D | 2026-03-25 PASS | Structural | `RecoveryHint` | |
| `recovery_hint_format` | D | 2026-03-25 PASS | Structural | Hint display | |
| `error_is_validation` | D | 2026-03-25 PASS | Structural | `is_validation()` | |
| `error_is_io` | D | 2026-03-25 PASS | Structural | `is_io()` | |
| `error_is_internal` | D | 2026-03-25 PASS | Structural | `is_internal()` | |
| `error_display_formats` | D | 2026-03-25 PASS | Structural | `Display` | |
| `error_code_as_str` | D | 2026-03-25 PASS | Structural | `ErrorCode` | Duplicates unit coverage. |
| `error_code_format_pattern` | D | 2026-03-25 PASS | Structural | Code format | |
| `error_trait_implementation` | D | 2026-03-25 PASS | Structural | `std::error::Error` | |
| `result_type_ok` | D | 2026-03-25 PASS | Structural | `xtask::error::Result` alias | |
| `result_type_err` | D | 2026-03-25 PASS | Structural | Result alias | |
| `result_error_handling_patterns` | D | 2026-03-25 PASS | Structural | `?` / chaining | |
| `error_from_ploke_error_maps_for_database_domain` | D | 2026-03-25 PASS | Behavior-gated | `From<ploke_error::Error>` + DB domain display/recovery | Integration boundary. |
| `error_report_contains_all_info` | D | 2026-03-25 PASS | Structural | Formatted report strings | |

---

## Integration tests: [`executor_tests.rs`](../../../../xtask/tests/executor_tests.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `executor_runs_sync_command` | M.3/M.4 | 2026-03-25 PASS | Behavior-gated | `CommandExecutor::execute` sync path | |
| `executor_runs_async_command` | M.3/M.4 | 2026-03-25 PASS | Behavior-gated | Executor async dispatch (`TestAsyncCommand`) | Does not exercise `MaybeAsync::block` pending path. |
| `executor_validates_prerequisites` | M.3/M.4 | 2026-03-25 PASS | Behavior-gated | Prereq validation + `database_pool()` | |
| `executor_tracks_usage` | B.4 (intent) | 2026-03-25 PASS | Behavior-gated | `CommandExecutor` + `UsageTracker::total_command_count` | Asserts tracker sees ≥5 runs; does not read JSONL lines or `generate_stats` from file. |
| `executor_handles_command_failure` | M.4 | 2026-03-25 PASS | Behavior-gated | Error propagation from `Command::execute` | |
| `executor_works_without_async_runtime` | M.4 | 2026-03-25 PASS | Behavior-gated | `ExecutorConfig.enable_async = false` | |
| `executor_provides_context_access` | M.4 | 2026-03-25 PASS | Behavior-gated | `executor.context()` | |
| `registry_registers_commands` | M.4 | 2026-03-25 PASS | Behavior-gated | `CommandRegistry::register` metadata | Factory still stubbed. |
| `registry_organizes_by_category` | M.4 | 2026-03-25 PASS | Behavior-gated | Category buckets | |
| `registry_generates_help` | C.3 (registry) | 2026-03-25 PASS | Behavior-gated | `generate_help()` | |
| `registry_returns_none_for_unknown_command` | M.4 | 2026-03-25 PASS | Structural | Empty registry lookup | |
| `registry_factory_panics_until_command_construction_implemented` | M.3/M.4 | 2026-03-25 PASS | **Gap-signal** | N/A (pass means factory still panics) | Replace when `CommandRegistry` builds commands from args ([`executor.rs`](../../../../xtask/src/executor.rs) `todo!`). |
| `maybe_async_ready_holds_value` | M.3 | 2026-03-25 PASS | Structural | `MaybeAsync::Ready` | |
| `maybe_async_from_value` | M.3 | 2026-03-25 PASS | Structural | `From` into `Ready` | |
| `maybe_async_into_future_ready` | M.3 | 2026-03-25 PASS | Structural | `into_future` on `Ready` | |
| `maybe_async_block_ready` | M.3 | 2026-03-25 PASS | Structural | `block()` on `Ready` only | Pending branch still `todo!`. |
| `resource_requirements_default` | M.3 | 2026-03-25 PASS | Structural | `ResourceRequirements` defaults | |
| `resource_requirements_custom` | M.3 | 2026-03-25 PASS | Structural | Custom reqs on test command | |
| `command_category_as_str` | M.3 | 2026-03-25 PASS | Structural | `CommandCategory::as_str` | Duplicates lib unit tests. |
| `command_category_all_count` | M.3 | 2026-03-25 PASS | Structural | Category count | |

---

## Integration tests: [`parse_commands.rs`](../../../../xtask/tests/parse_commands.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `discovery_finds_cargo_toml` | A.1, E | 2026-03-25 PASS | Behavior-gated | `parse discovery` + `syn_parser` discovery | Fixture under workspace. |
| `discovery_error_missing_cargo_toml` | A.1, D | 2026-03-25 PASS | Behavior-gated | Discovery error + recovery | |
| `phases_merge_produces_merged_graph` | A.1, E | 2026-03-25 PASS | Behavior-gated | `parse phases-merge` | |
| `phases_merge_with_tree_output` | A.1 | 2026-03-25 PASS | Behavior-gated | `phases-merge --tree` | |
| `workspace_parses_all_crates` | A.1 | 2026-03-25 PASS | Behavior-gated | `parse workspace` | |
| `workspace_selective_crate_parsing` | A.1 | 2026-03-25 PASS | Behavior-gated | `workspace` crate filter | |
| `workspace_continue_on_error` | A.1 | 2026-03-25 PASS | Behavior-gated | `continue_on_error` path | |
| `stats_returns_accurate_counts` | A.1 | 2026-03-25 PASS | Behavior-gated | `parse stats` | |
| `stats_with_node_type_filter` | A.1 | 2026-03-25 PASS | Behavior-gated | `parse stats` filter | |
| `list_modules_finds_all_modules` | A.1 | 2026-03-25 PASS | Behavior-gated | `parse list-modules` | |
| `list_modules_with_full_path` | A.1 | 2026-03-25 PASS | Behavior-gated | `--full-path` | |
| `parse_command_invalid_path_error` | A.1, D | 2026-03-25 PASS | Behavior-gated | Invalid path + recovery suggestion | Permissive substring match on message. |
| `parse_output_serialization` | E | 2026-03-25 PASS | Structural | `ParseOutput` serde shape | Static sample. |
| `node_type_filter_variants` | E | 2026-03-25 PASS | Structural | `NodeTypeFilter` enum count | |
| `module_info_creation` | E | 2026-03-25 PASS | Structural | `ModuleInfo` struct + serde | |
| `discovery_command_trait` | E | 2026-03-25 PASS | Structural | `Command` metadata for `Discovery` | |
| `phases_merge_command_trait` | E | 2026-03-25 PASS | Structural | `PhasesMerge` trait surface | |
| `workspace_command_trait` | E | 2026-03-25 PASS | Structural | `Workspace` trait surface | |
| `stats_command_trait` | E | 2026-03-25 PASS | Structural | `Stats` trait surface | |
| `list_modules_command_trait` | E | 2026-03-25 PASS | Structural | `ListModules` trait surface | |

---

## Integration tests: [`command_acceptance_parse.rs`](../../../../xtask/tests/command_acceptance_parse.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `acceptance_parse_phases_resolve_success_fixture_nodes` | A.1, E | 2026-03-25 PASS | Behavior-gated | `parse phases-resolve` + `try_run_phases_and_resolve` | Pinned `tests/fixture_crates/fixture_nodes`. |
| `acceptance_parse_phases_resolve_rejects_missing_path` | A.1, D | 2026-03-25 PASS | Behavior-gated | Path validation + recovery | |

---

## Integration tests: [`command_acceptance_db.rs`](../../../../xtask/tests/command_acceptance_db.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `acceptance_db_list_relations_success` | A.4, E | 2026-03-25 PASS | Behavior-gated | `db list-relations` + fixture DB | |
| `acceptance_db_list_relations_with_counts` | A.4 | 2026-03-25 PASS | Behavior-gated | `--counts` → at least one `row_count: Some` | Some relations omit count when generic count query fails (`.ok()`). |
| `acceptance_db_embedding_status_success` | A.4 | 2026-03-25 PASS | Behavior-gated | `db embedding-status` + current count semantics | |
| `acceptance_db_hnsw_build_panics_until_implemented` | A.4 | 2026-03-25 PASS | Gap-signal | N/A | Replace with Ok + DB checks when `HnswBuild` implemented. |
| `acceptance_db_hnsw_rebuild_panics_until_implemented` | A.4 | 2026-03-25 PASS | Gap-signal | N/A | |
| `acceptance_db_bm25_rebuild_panics_until_implemented` | A.4 | 2026-03-25 PASS | Gap-signal | N/A | |

---

## Unit tests: [`xtask/src/cli.rs`](../../../../xtask/src/cli.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_cli_default_format` | E | 2026-03-25 PASS | Structural | Cli default `OutputFormat` | |
| `test_cli_json_format` | E | 2026-03-25 PASS | Structural | `--format json` parse | |
| `test_cli_verbose_count` | E | 2026-03-25 PASS | Structural | `-v` counting | |
| `test_cli_quiet` | E | 2026-03-25 PASS | Structural | `-q` | |

---

## Unit tests: [`xtask/src/commands/db.rs`](../../../../xtask/src/commands/db.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_count_nodes_fields` | E | 2026-03-25 PASS | Structural | `CountNodes` clap/defaults | |
| `test_query_params` | E | 2026-03-25 PASS | Structural | `Query` args | |
| `test_db_output_serialization` | E | 2026-03-25 PASS | Structural | `DbOutput` serde | |
| `test_parse_key_val` | E | 2026-03-25 PASS | Structural | `parse_key_val` helper | |
| `test_parse_key_val_invalid` | E | 2026-03-25 PASS | Behavior-gated | Invalid key=val parse errors | |
| `test_embedding_set_info` | A.4 | 2026-03-25 PASS | Structural | Embedding set metadata type | |

---

## Unit tests: [`xtask/src/commands/mod.rs`](../../../../xtask/src/commands/mod.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_output_format_default` | E | 2026-03-25 PASS | Structural | `OutputFormat` default | |
| `test_output_format_json` | E | 2026-03-25 PASS | Structural | JSON variant | |
| `test_output_format_human` | E | 2026-03-25 PASS | Structural | Human variant | |
| `test_output_format_table_not_implemented` | E | 2026-03-25 PASS | Behavior-gated | `format_table` returns error until implemented | |

---

## Unit tests: [`xtask/src/commands/parse.rs`](../../../../xtask/src/commands/parse.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_discovery_default_path` | A.1 | 2026-03-25 PASS | Structural | Default path on `Discovery` | |
| `test_phases_merge_fields` | A.1 | 2026-03-25 PASS | Structural | `PhasesMerge` fields | |
| `test_parse_output_serialization` | E | 2026-03-25 PASS | Structural | Parse output JSON | Overlaps integration `parse_output_serialization`. |
| `test_module_info` | E | 2026-03-25 PASS | Structural | `ModuleInfo` | |

---

## Unit tests: [`xtask/src/context.rs`](../../../../xtask/src/context.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_context_new` | Architecture | 2026-03-25 PASS | Behavior-gated | `CommandContext::new` | Mirrors integration themes. |
| `test_context_default` | Architecture | 2026-03-25 PASS | Structural | Default context | |
| `test_io_manager` | Architecture | 2026-03-25 PASS | Behavior-gated | IO manager init | |
| `test_temp_dir` | Architecture | 2026-03-25 PASS | Behavior-gated | Temp dir | |
| `test_validate_resources` | Architecture | 2026-03-25 PASS | Behavior-gated | Validation | |
| `test_database_pool_new` | A.4 prep | 2026-03-25 PASS | Behavior-gated | DB pool | |
| `test_io_manager_handle` | Architecture | 2026-03-25 PASS | Behavior-gated | Handle access | |
| `test_context_loads_canonical_fixture` | A.4 | 2026-03-25 PASS | Behavior-gated | Fixture load via context | |
| `test_embedding_runtime_manager` | A.3 prep | 2026-03-25 PASS | Behavior-gated | Embedding runtime | |

---

## Unit tests: [`xtask/src/error.rs`](../../../../xtask/src/error.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_new_error` | D | 2026-03-25 PASS | Structural | `XtaskError::new` | See `error_tests.rs` overlap. |
| `test_internal_error` | D | 2026-03-25 PASS | Structural | Internal | |
| `test_validation_builder` | D | 2026-03-25 PASS | Structural | Validation builder | |
| `test_validation_without_recovery` | D | 2026-03-25 PASS | Structural | No recovery | |
| `test_recovery_suggestion` | D | 2026-03-25 PASS | Structural | Suggestions | |
| `test_with_context` | D | 2026-03-25 PASS | Structural | Context wrap | |
| `test_from_string` | D | 2026-03-25 PASS | Structural | Conversions | |
| `test_from_str` | D | 2026-03-25 PASS | Structural | | |
| `test_io_error_conversion` | D | 2026-03-25 PASS | Structural | | |
| `test_error_code_as_str` | D | 2026-03-25 PASS | Structural | | |
| `test_recovery_hint_format` | D | 2026-03-25 PASS | Structural | | |
| `test_display` | D | 2026-03-25 PASS | Structural | | |
| `test_is_validation` | D | 2026-03-25 PASS | Structural | | |
| `test_is_io` | D | 2026-03-25 PASS | Structural | | |
| `test_source` | D | 2026-03-25 PASS | Structural | | |
| `test_result_type_alias` | D | 2026-03-25 PASS | Structural | | |

---

## Unit tests: [`xtask/src/executor.rs`](../../../../xtask/src/executor.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_command_category_as_str` | M.3 | 2026-03-25 PASS | Structural | | |
| `test_command_category_all` | M.3 | 2026-03-25 PASS | Structural | | |
| `test_maybe_async_ready` | M.3 | 2026-03-25 PASS | Structural | | |
| `test_registry_new` | M.3 | 2026-03-25 PASS | Structural | | |
| `test_registry_generate_help` | M.3 | 2026-03-25 PASS | Structural | | |
| `test_resource_requirements_default` | M.3 | 2026-03-25 PASS | Structural | | |
| `test_executor_config_default` | M.3 | 2026-03-25 PASS | Structural | | |

---

## Unit tests: [`xtask/src/lib.rs`](../../../../xtask/src/lib.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_version` | E | 2026-03-25 PASS | Structural | Crate version const | |
| `test_workspace_root` | A.1–A.4 | 2026-03-25 PASS | Behavior-gated | `workspace_root()` | |
| `test_display_relative` | E | 2026-03-25 PASS | Structural | Path display helper | |

---

## Unit tests: [`xtask/src/test_harness.rs`](../../../../xtask/src/test_harness.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_test_result_passed` | E | 2026-03-25 PASS | Structural | Harness types | |
| `test_test_result_failed` | E | 2026-03-25 PASS | Structural | | |
| `test_test_case_builder` | E | 2026-03-25 PASS | Structural | | |
| `test_expected_result_variants` | E | 2026-03-25 PASS | Structural | | |
| `test_test_outcome_helpers` | E | 2026-03-25 PASS | Structural | | |
| `test_fixture_handle` | E | 2026-03-25 PASS | Structural | | |
| `test_harness_new` | E | 2026-03-25 PASS | Structural | `CommandTestHarness` | |
| `test_assertions` | E | 2026-03-25 PASS | Structural | | |
| `test_expect_helpers` | E | 2026-03-25 PASS | Structural | `expect_command_ok` helpers | |
| `test_string_diff_creation` | E | 2026-03-25 PASS | Structural | Diff types | |
| `test_diff_type_variants` | E | 2026-03-25 PASS | Structural | | |

---

## Unit tests: [`xtask/src/usage.rs`](../../../../xtask/src/usage.rs)

| Test | Spec | Verified | Gate | Unblocks green | Notes |
|------|------|----------|------|----------------|-------|
| `test_usage_tracker_new` | B.4 | 2026-03-25 PASS | Behavior-gated | `UsageTracker::new` | |
| `test_record_start_completion` | B.4 | 2026-03-25 PASS | Behavior-gated | In-memory record flow | |
| `test_flush_buffer` | B.4 | 2026-03-25 PASS | Behavior-gated | Flush to log path | |
| `test_generate_stats` | B.4 | 2026-03-25 PASS | Behavior-gated | Stats aggregation | |
| `test_should_show_suggestion` | B.5 | 2026-03-25 PASS | Behavior-gated | Suggestion counter threshold | Experimental spec item. |
| `test_usage_record_serialization` | B.4 | 2026-03-25 PASS | Structural | Serde of records | |
| `test_format_usage_summary` | B.4 | 2026-03-25 PASS | Structural | Summary formatting | |
| `test_default_usage_log_path` | B.4 | 2026-03-25 PASS | Structural | Default path helper | |
| `test_feedback_file_path` | B.5 | 2026-03-25 PASS | Structural | Feedback path | |
| `test_usage_start_creation` | B.4 | 2026-03-25 PASS | Structural | Start record | |

---

## Planned tests (not yet written) — spec gaps

| Planned scenario (suggested name) | Spec | Gate (when written) | Unblocks green | Notes |
|-----------------------------------|------|---------------------|----------------|-------|
| `transform_parsed_graph_command_ok` | A.2 | Behavior-gated | `xtask` command wrapping `ploke_transform::transform_parsed_graph` + tracing | Depends on parse output path. |
| `ingest_embeddings_uses_test_openrouter_env_only` | A.3 | Behavior-gated | Ingest/embed pipeline command; **no** CLI key override; reads `TEST_OPENROUTER_API_KEY` | Per spec comments. |
| `db_hnsw_build_executes` | A.4 | Behavior-gated | `db hnsw-build` impl (replaces `todo!` in [`db.rs`](../../../../xtask/src/commands/db.rs)) | Gap-signal today: [`acceptance_db_hnsw_build_panics_until_implemented`](../../../../xtask/tests/command_acceptance_db.rs). |
| `db_hnsw_rebuild_executes` | A.4 | Behavior-gated | `db hnsw-rebuild` | Gap-signal: [`acceptance_db_hnsw_rebuild_panics_until_implemented`](../../../../xtask/tests/command_acceptance_db.rs). |
| `db_bm25_rebuild_executes` | A.4 | Behavior-gated | `db bm25-rebuild` | Gap-signal: [`acceptance_db_bm25_rebuild_panics_until_implemented`](../../../../xtask/tests/command_acceptance_db.rs). |
| `load_fixture_index_builds_real_hnsw` | A.4 | Behavior-gated | `LoadFixture --index` performs real index ops OR CLI/docs aligned with behavior | Tighten current Weak test. |
| `maybe_async_block_on_pending` | M.4 | Behavior-gated | `MaybeAsync::block` + runtime (`todo!` removal) | |
| `registry_factory_builds_parse_or_db_command` | M.4 | Behavior-gated | `CommandRegistry` arg parsing / factories | Replaces gap-signal panic test. |
| `executor_usage_persisted_to_log` | B.4 | Behavior-gated | Executor + `UsageTracker` JSONL file contents after runs | Optional: parse log file lines; `executor_tracks_usage` already checks `total_command_count`. |
| `cli_tracing_log_hint_in_output` | B.2 | Behavior-gated | Tracing subscriber + stdout hint with log path | |
| `headless_tui_runs_app_test_backend` | A.5 | Behavior-gated | `ploke-tui` headless module + `TestBackend` | M.5. |
| `headless_tui_sends_keys_and_timeout` | A.5 | Behavior-gated | Input simulation + keycodes | |
| `tool_bypass_runs_single_tool_json` | A.6 | Behavior-gated | Direct tool dispatch in `ploke-tui` | M.5. |
| `main_binary_dispatches_agent_cli` | C.1–C.3 | Behavior-gated | Unify [`main.rs`](../../../../xtask/src/main.rs) with [`Cli::execute`](../../../../xtask/src/cli.rs) | Matches `cli_invariant_tests` note. |
| `help_staleness_prompt_or_metadata` | B.3 | Behavior-gated | Help “last updated” / review prompt | Spec experimental. |

---

## Hypothesis template (for new rows)

When adding tests, keep PRIMARY_TASK_SPEC E.3 discipline:

1. **To prove:** …  
2. **Why useful:** …  
3. **When this would not prove correctness:** …  

---

## Test runs (PRIMARY_TASK_SPEC §E.2)

| Date | Command | Result |
|------|---------|--------|
| 2026-03-25 | `cargo test -p xtask` | **PASS** (all integration + lib unit tests + doctests ran; 7 doctests ignored) |
| 2026-03-25 | `cargo test -p xtask --lib` | **PASS** |
| 2026-03-25 | `cargo test -p xtask --no-run` | **PASS** (compile tests only) |
| 2026-03-25 | `cargo test -p ploke-db --lib --no-run` | **PASS** (fixture-related) |
| 2026-03-25 | `cargo test -p xtask` | **PASS** after command acceptance tests + `executor_tracks_usage` assert |

## Updates log

| Date | Change |
|------|--------|
| 2026-03-25 | Initial population; canonical path + pointer to `xtask/tests/test_matrix.md` |
| 2026-03-25 | Matrix updated for fail-until-impl policy (`parse_commands`, `db_commands`) |
| 2026-03-25 | §E.2 run log; `cli_invariant_tests`; real `ploke_db` context; parse/db command bodies; fixture isolation helper; §D recovery + `ploke_error` mapping tests |
| 2026-03-25 | **Per-test inventory** for all integration tests and all `#[test]` in `xtask/src/**`; **Gate** column (Behavior-gated / Structural / Gap-signal / Weak); **Unblocks green** column; **Legend**; **Planned tests** for A.2, A.3, A.4 index subcommands, M.4 executor/registry/async block, A.5–A.6, §B, CLI/binary unify; refreshed §E.2 (`cargo test -p xtask`, `--lib`). |
| 2026-03-25 | **Command acceptance index** + [`command_acceptance_parse.rs`](../../../../xtask/tests/command_acceptance_parse.rs) / [`command_acceptance_db.rs`](../../../../xtask/tests/command_acceptance_db.rs); `executor_tracks_usage` asserts `UsageTracker::total_command_count`; `load_fixture_with_index_flag` doc aligned to message-only behavior. |

---

## Retired per-file prose (superseded by tables above)

The previous short “Per-file notes” bullets are folded into the per-test tables and the summary table. Refer to [design/test-design-requirements.md](./design/test-design-requirements.md) for proof-oriented command expectations.
