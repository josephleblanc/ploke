# Command Matrix for xtask Commands Feature

**Date:** 2026-03-25  
**Milestone:** M.1 - Survey crates for functions to use for commands  
**Branch:** feature/xtask-commands

## Overview

This document tracks the mapping between desired xtask commands (from PRIMARY_TASK_SPEC.md sections A.1-A.4) and the actual functions available in ploke workspace crates.

### Legend: survey vs `xtask` implementation

| Column | Meaning |
|--------|---------|
| **Survey** | M.1 mapping complete: target crate API identified for this command. |
| **`xtask` impl** | Status of the command in the [`xtask`](../../../../xtask) crate (clap + execute path). Not the same as survey completeness. |

**`xtask` impl values:** **Stub** = CLI/types exist but `execute` is `todo!()` or otherwise non-functional; **Not started** = no dedicated command surface in `xtask` yet; **N/A** = not an `xtask` subcommand in the current design; **Partial** / **Done** = reserved for when behavior is implemented (M.4+).

## A.1 Parsing Commands

### Target Functions from syn_parser

| Command | Function | Crate | Module | Survey | `xtask` impl | Notes |
|---------|----------|-------|--------|--------|--------------|-------|
| `parse discovery` | `run_discovery_phase` | syn_parser | `discovery` | Complete | Stub | Discovery phase for workspace/crate analysis |
| `parse phases-resolve` | `try_run_phases_and_resolve` | syn_parser | `lib` | Complete | Stub | Parse and resolve without merging |
| `parse phases-merge` | `try_run_phases_and_merge` | syn_parser | `lib` | Complete | Stub | Parse, resolve, and merge graphs |
| `parse workspace` | `parse_workspace` | syn_parser | `lib` | Complete | Stub | Full workspace parsing |

### Related Types Needed
- `DiscoveryOutput`
- `ParsedCodeGraph`
- `ParserOutput`
- `ModuleTree`
- `SynParserError`

## A.2 Transform to DB Commands

### Target Functions from ploke_transform

| Command | Function | Crate | Module | Survey | `xtask` impl | Notes |
|---------|----------|-------|--------|--------|--------------|-------|
| `transform graph` | `transform_parsed_graph` | ploke_transform | `transform` | Complete | Not started | Transform parsed graph to CozoDB |
| `transform workspace` | `transform_parsed_workspace` | ploke_transform | `transform::workspace` | Complete | Not started | Transform workspace to CozoDB |

### Related Types Needed
- `TransformError`
- `ParsedCodeGraph`
- `ModuleTree`

## A.3 Ingestion Pipeline (up to embeddings)

### Target Functions from ploke_embed

| Command | Function | Crate | Module | Survey | `xtask` impl | Notes |
|---------|----------|-------|--------|--------|--------------|-------|
| `ingest embed` | `EmbeddingProcessor::new` | ploke_embed | `indexer` | Complete | Not started | Process embeddings |
| `ingest index` | `IndexerTask::run` | ploke_embed | `indexer` | Complete | Not started | Run indexing task |

### Environment Variable
- `TEST_OPENROUTER_API_KEY` (different from `OPENROUTER_API_KEY` used elsewhere)

## A.4 Database Commands

### Target Functions from ploke_db

| Command | Function | Crate | Module | Survey | `xtask` impl | Notes |
|---------|----------|-------|--------|--------|--------------|-------|
| `db save` | `backup_db` (cozo) | ploke_db | `database` | Complete | Stub | Via `db.db.backup_db()` - see survey notes |
| `db load` | `restore_backup` (cozo) | ploke_db | `database` | Complete | Stub | Via `db.db.restore_backup()` - see survey notes |
| `db load-fixture` | `fresh_backup_fixture_db` | ploke_test_utils | `fixture_dbs` | Complete | Stub | Loads fixtures like `FIXTURE_NODES_CANONICAL` |
| `db count-nodes` | `count_pending_embeddings` | ploke_db | `database` | Complete | Stub | Counts nodes needing embeddings + others |
| `db hnsw-build` | `create_index_primary` | ploke_db | `index::hnsw` | Complete | Stub | Creates HNSW index for active embedding set |
| `db hnsw-rebuild` | `create_index_primary_with_index` | ploke_db | `index::hnsw` | Complete | Stub | Rebuilds HNSW (currently same as build) |
| `db bm25-rebuild` | `Bm25Indexer::rebuild_from_db` | ploke_db | `bm25_index` | Complete | Stub | Rebuilds BM25 from DB source of truth |
| `db query` | `run_script` (cozo) | ploke_db | `database` | Complete | Stub | Via Deref to inner CozoDB |

### Related Types Needed
- `Database`
- `DbError`
- `NodeType`
- `QueryResult`

## A.5 Headless TUI Commands

### Target Types/Functions from ploke_tui

| Command | Function/Type | Crate | Module | Survey | `xtask` impl | Notes |
|---------|---------------|-------|--------|--------|--------------|-------|
| `tui headless` | `App` with `TestBackend` | ploke_tui | `app` | Complete | Not started | Run TUI in headless mode |
| `tui input` | Simulate user input | ploke_tui | - | Complete | Not started | Send input to TUI |
| `tui key` | Simulate keypress | ploke_tui | - | Complete | Not started | Send key codes |

## A.6 Tool Call Commands

### Target Functions from ploke_tui

| Command | Function | Crate | Module | Survey | `xtask` impl | Notes |
|---------|----------|-------|--------|--------|--------------|-------|
| `tool ns-read` | `NsRead` | ploke_tui | `tools::ns_read` | Complete | Not started | Namespace read tool |
| `tool code-lookup` | `CodeItemLookup` | ploke_tui | `tools::code_item_lookup` | Complete | Not started | Code item lookup |

---

## Sub-Agent Survey Assignments

### Crate Survey Checklist

- [x] **syn_parser** - Parsing pipeline functions (A.1)
- [x] **ploke_transform** - Transform functions (A.2)
- [x] **ploke_embed** - Embedding/indexing functions (A.3)
- [x] **ploke_db** - Database operations (A.4)
- [x] **ploke_tui** - TUI and tool functions (A.5-A.6)
- [x] **ploke_test_utils** - Fixture loading utilities

## Survey Output Template

For each crate, sub-agents should document:

1. **Functions found** with full paths and signatures
2. **Input parameters** with types
3. **Output/Return types**
4. **Error types**
5. **Key types/structs** needed
6. **Special considerations** (async, complex setup, etc.)
7. **Example usage** from existing code/tests

---

## Cross-Crate Commands (M.1.3)

This section documents high-value commands that chain operations across multiple crates for agent convenience.

**`xtask` impl (entire section):** Not started — no `pipeline` / `validate` / `workflow` subcommands in `xtask` yet (M.4+). Survey and dependency graphs below remain the source of truth for design.

### Pipeline Commands

| Command | Crates | Functions | Input | Output | Priority |
|---------|--------|-----------|-------|--------|----------|
| `pipeline parse-transform` | syn_parser, ploke_transform, ploke_db | `try_run_phases_and_merge` → `transform_parsed_graph` | `crate_path: PathBuf` | Database with code graph | P1 |
| `pipeline full-ingest` | syn_parser, ploke_transform, ploke_db, ploke_embed | Parse + transform + `IndexerTask::run` | `crate_path`, `backend: String` | Database + embeddings | P3 |
| `pipeline workspace` | syn_parser, ploke_transform, ploke_db | `parse_workspace` → `transform_parsed_workspace` | `workspace_path: PathBuf` | Database with workspace | P2 |

**Workflow: `pipeline parse-transform`**
```
1. Database::init_with_schema()
2. try_run_phases_and_merge(crate_path)
3. extract_merged_graph() + extract_module_tree()
4. create_schema_all(&db)
5. transform_parsed_graph(&db, graph, &tree)
```

**Workflow: `pipeline full-ingest`**
```
1. Database::init_with_schema()
2. db.setup_multi_embedding()
3. try_run_phases_and_merge(crate_path)
4. transform_parsed_graph(&db, graph, &tree)
5. EmbeddingRuntime::with_default_set(processor)
6. IndexerTask::new(...).run(progress_tx, control_rx).await
7. create_index_primary(&db)
```

### Validation Commands

| Command | Crates | Functions | Purpose | Priority |
|---------|--------|-----------|---------|----------|
| `validate parse-integrity` | syn_parser | `try_run_phases_and_merge` + graph validation | Check graph consistency | P2 |
| `validate db-health` | ploke_db | `count_*`, `relations_vec`, queries | Database diagnostics | P2 |
| `validate end-to-end` | syn_parser, ploke_transform, ploke_db | Parse → transform → compare stats | Verify pipeline integrity | P3 |

**Workflow: `validate db-health`**
```
1. Count all relations
2. Count nodes by type
3. Count pending/complete embeddings
4. Check HNSW index status
5. Check BM25 index status
6. Output health report
```

### Diagnostic Commands

| Command | Crates | Functions | Output | Priority |
|---------|--------|-----------|--------|----------|
| `diagnostic db-report` | ploke_db | `run_script`, `count_edges_by_kind` | Comprehensive report (JSON/table) | P2 |
| `diagnostic embedding-status` | ploke_db | `count_pending_embeddings`, `count_complete_embeddings` | Embedding coverage stats | P3 |
| `compare parse-transform` | syn_parser, ploke_transform, ploke_db | Parse stats vs DB stats comparison | Side-by-side comparison | P4 |
| `debug graph-inspect` | syn_parser | `try_run_phases_and_resolve`, `GraphAccess` | Detailed graph output | P4 |

### Setup Commands

| Command | Crates | Functions | Purpose | Priority |
|---------|--------|-----------|---------|----------|
| `setup test-env` | ploke_test_utils, ploke_db | `fresh_backup_fixture_db`, `create_index_primary` | Load fixture database | P1 |
| `setup dev-workspace` | syn_parser, ploke_transform, ploke_db | `parse_workspace` → `transform_parsed_workspace` | Setup workspace for dev | P2 |

**Available Fixtures for `setup test-env`:**
| Fixture ID | Description | Import Mode |
|------------|-------------|-------------|
| `fixture_nodes_canonical` | Standard test fixture | PlainBackup |
| `fixture_nodes_local_embeddings` | With local embeddings | BackupWithEmbeddings |
| `ploke_db_primary` | ploke-db crate backup | PlainBackup |
| `ws_fixture_01_canonical` | Workspace fixture | PlainBackup |

### Workflow Commands

| Command | Crates | Functions | Purpose | Priority |
|---------|--------|-----------|---------|----------|
| `workflow reindex` | syn_parser, ploke_transform, ploke_db, ploke_embed | Full pipeline rebuild | Regenerate workspace index | P3 |
| `workflow regenerate-fixture` | ploke_test_utils, syn_parser, ploke_transform, ploke_db | `setup_db_full_multi_embedding` | Regenerate fixture databases | P4 |

---

## Cross-Crate Command Dependencies

```
┌─────────────────────────────────────────────────────────────┐
│                     DEPENDENCY GRAPH                        │
└─────────────────────────────────────────────────────────────┘

pipeline parse-transform
  ├── parse: try_run_phases_and_merge (syn_parser)
  └── transform: transform_parsed_graph (ploke_transform)

pipeline full-ingest
  ├── pipeline parse-transform (above)
  ├── setup_multi_embedding (ploke_db)
  └── embed: IndexerTask::run (ploke_embed)

pipeline workspace
  ├── parse_workspace (syn_parser)
  └── transform_parsed_workspace (ploke_transform)

workflow reindex
  ├── pipeline workspace (or parse-transform)
  └── pipeline full-ingest (embeddings)

validate end-to-end
  ├── pipeline parse-transform
  └── query comparison (ploke_db)

setup test-env
  ├── fresh_backup_fixture_db (ploke_test_utils)
  └── create_index_primary (ploke_db) [optional]
```

---

## Error Handling by Command Category

| Category | Pattern | Behavior |
|----------|---------|----------|
| Pipeline | Fail-fast | Stop on first error, cleanup partial state |
| Validation | Aggregate | Collect all errors, report summary |
| Setup | Idempotent | Safe to re-run, skip if exists |
| Workflow | Checkpoint | Save progress, allow resume |

---

## Additional Functions Identified (M.1.2)

Functions discovered during additional survey that would be valuable for agent diagnostics:

### Database Statistics & Node Counting

| Suggested Command | Function | Crate | Module | Description |
|-------------------|----------|-------|--------|-------------|
| `db stats embeddings` | `count_complete_embeddings` | ploke_db | `multi_embedding::db_ext` | Count nodes with complete embeddings |
| `db stats embeddings --set` | `count_embeddings_for_set` | ploke_db | `multi_embedding::db_ext` | Count embeddings for specific set |
| `db stats nodes` | `count_common_nodes` | ploke_db | `multi_embedding::db_ext` | Count embeddable primary nodes |
| `db stats edges` | `count_edges_by_kind` | ploke_db | `helpers` | Count edges by relation kind |

### Index Status Functions

| Suggested Command | Function | Crate | Module | Description |
|-------------------|----------|-------|--------|-------------|
| `db hnsw-status` | `is_hnsw_index_registered` | ploke_db | `multi_embedding::hnsw_ext` | Check HNSW index existence |
| `db check-schema` | `is_embedding_set_registered` | ploke_db | `multi_embedding::db_ext` | Verify embedding schema exists |
| `db list-embedding-sets` | `list_embedding_sets` | ploke_db | `database` | List all embedding sets |
| `db bm25-status` | `Bm25Status` / `doc_count` | ploke_db | `bm25_index` | BM25 index status and doc count |

### Namespace & Workspace Queries

| Suggested Command | Function | Crate | Module | Description |
|-------------------|----------|-------|--------|-------------|
| `db list-crates` | `list_crate_context_rows` | ploke_db | `database` | List all loaded crates |
| `db namespace-info` | `collect_namespace_inventory` | ploke_db | `database` | Get namespace details |
| `db crate-files` | `get_crate_files` | ploke_db | `database` | Get files for a crate |
| `db path-info` | `get_path_info` | ploke_db | `database` | Query path information |

### Database Introspection

| Suggested Command | Function | Crate | Module | Description |
|-------------------|----------|-------|--------|-------------|
| `db list-relations` | `relations_vec` | ploke_db | `database` | List all database relations |
| `db list-relations --no-hnsw` | `relations_vec_no_hnsw` | ploke_db | `database` | List relations excluding HNSW |
| `db list-tracked-relations` | `rel_names_with_tracking_hash` | ploke_db | `database` | Relations with tracking_hash |
| `db list-files` | `get_file_data` | ploke_db | `database` | List all file data |
| `db list-embedded-nodes` | `list_primary_nodes` | ploke_db | `helpers` | List embedded primary nodes |

### Integrity & Validation

| Suggested Command | Function | Crate | Module | Description |
|-------------------|----------|-------|--------|-------------|
| `db validate-import` | `validate_namespace_import_conflicts` | ploke_db | `database` | Pre-flight import validation |
| `db verify-embedding-set` | `restore_embedding_set` | ploke_db | `database` | Verify/restore embedding set |
| `db active-embedding-set` | `with_active_set` | ploke_db | `database` | Show current embedding config |

### Reference Document

See full details in: [`additional-ploke_db.md`](./sub-agents/additional-ploke_db.md)

---

## Additional Functions Identified (M.1.2)

Additional diagnostic/inspection functions identified in syn_parser that would be valuable for agent troubleshooting:

### Graph Validation & Debug

| Command | Function | Module | Diagnostic Value |
|---------|----------|--------|------------------|
| `parse validate-relations` | `validate_unique_rels` | `parser::graph` | Check for duplicate relations |
| `parse debug-relations` | `debug_relationships` | `parser::graph` | Print detailed relation info |
| `parse list-items` | `debug_print_all_visible` | `parser::graph` | List all parsed items |

### Node Lookup & Query

| Command | Function | Module | Diagnostic Value |
|---------|----------|--------|------------------|
| `parse find-node` | `find_node_unique` | `parser::graph` | Find node by ID |
| `parse find-module` | `find_module_by_path_checked` | `parser::graph` | Find module by path |
| `parse get-*` | `get_*_checked` | `parser::graph` | Typed node getters |

### Metadata & Dependencies

| Command | Function | Module | Diagnostic Value |
|---------|----------|--------|------------------|
| `parse crate-info` | `CrateContext` fields | `discovery` | Show crate metadata |
| `parse dependencies` | `dependency_names` | `parser::graph` | List dependencies |
| `parse features` | `Features::keys/values` | `discovery` | List Cargo features |
| `parse list-files` | `CrateContext::files` | `discovery` | List source files |

### Discovery Output

| Command | Function | Module | Diagnostic Value |
|---------|----------|--------|------------------|
| `parse discovery-list` | `iter_crate_contexts` | `discovery` | List discovered crates |
| `parse warnings` | `DiscoveryOutput::warnings` | `discovery` | Show discovery warnings |
| `parse check-warnings` | `has_warnings` | `discovery` | Quick warning check |

### Module Tree Inspection

| Command | Function | Module | Diagnostic Value |
|---------|----------|--------|------------------|
| `parse tree-root` | `ModuleTree::root` | `resolve::module_tree` | Show tree root |
| `parse tree-modules` | `ModuleTree::modules` | `resolve::module_tree` | List modules |
| `parse tree-relations` | `ModuleTree::tree_relations` | `resolve::module_tree` | Tree relations |
| `parse pending-imports` | `ModuleTree::pending_imports` | `resolve::module_tree` | Unresolved imports |
| `parse pending-exports` | `ModuleTree::pending_exports` | `resolve::module_tree` | Unresolved exports |

### Workspace Discovery

| Command | Function | Module | Diagnostic Value |
|---------|----------|--------|------------------|
| `parse locate-workspace` | `locate_workspace_manifest` | `discovery::workspace` | Find workspace root |
| `parse manifest-info` | `try_parse_manifest` | `discovery::workspace` | Parse manifest |
| `parse workspace-version` | `resolve_workspace_version` | `discovery::workspace` | Resolve version |

### Statistics Commands

| Command | Functions | Description |
|---------|-----------|-------------|
| `parse stats` | All getter methods | Count of all node types |
| `parse list-functions` | `functions()` | List all functions |
| `parse list-types` | `defined_types()` | List all type defs |
| `parse list-modules` | `modules()` | List all modules |

---

## Updates Log

| Date | Agent | Update |
|------|-------|--------|
| 2026-03-25 | Main Agent | Initial matrix structure created |
| 2026-03-25 | Sub-Agent M.1.2 | Added 22 additional diagnostic functions for ploke_db (see additional-ploke_db.md) |
| 2026-03-25 | Sub-agent M.1.2 | Added additional syn_parser functions section |
| 2026-03-25 | Sub-agent M.1.3 | Comprehensive cross-crate commands section added (14 commands across 5 categories) |
| 2026-03-25 | Doc alignment | Added survey vs `xtask` impl legend and columns for A.1–A.6; cross-crate section marked not started in `xtask` |
