# Cross-Crate Commands for xtask

**Date:** 2026-03-25  
**Task:** M.1.3 - Identify cross-crate commands that combine functionality from multiple crates  
**Agent:** Sub-agent for cross-crate command analysis  
**Branch:** feature/xtask-commands

---

## Overview

This document identifies high-value cross-crate commands that chain multiple operations together for agent convenience. These commands combine functions from `syn_parser`, `ploke_transform`, `ploke_db`, `ploke_embed`, and `ploke_test_utils` to provide streamlined workflows.

---

## Cross-Crate Commands Identified

### 1. `pipeline parse-transform`

**Description:** Parse a crate and immediately transform it to the database in one operation.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `try_run_phases_and_merge` | `lib` |
| ploke_transform | `transform_parsed_graph` | `transform` |
| ploke_transform | `create_schema_all` | `schema` |
| ploke_db | `Database::init_with_schema` | `database` |

**Workflow/Order of Operations:**
1. Initialize database: `Database::init_with_schema()`
2. Run discovery phase: `run_discovery_phase()` (internal to step 3)
3. Parse and merge: `try_run_phases_and_merge(crate_path)`
4. Extract merged graph and module tree from `ParserOutput`
5. Create schema: `create_schema_all(&db)`
6. Transform to DB: `transform_parsed_graph(&db, merged_graph, &tree)`

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `crate_path` | `PathBuf` | Path to crate root directory |
| `db_path` | `Option<PathBuf>` | Optional path for persistent database |

**Output:** 
- Success: Database with transformed graph
- Errors: `SynParserError` | `TransformError` | `DbError`

**Error Handling:**
- Stop on first error (fail-fast)
- Provide detailed context about which stage failed
- Cleanup partial database on failure

---

### 2. `pipeline full-ingest`

**Description:** Full pipeline from parsing through embedding generation.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `try_run_phases_and_merge` | `lib` |
| ploke_transform | `transform_parsed_graph` | `transform` |
| ploke_db | `Database::init_with_schema` | `database` |
| ploke_db | `setup_multi_embedding` | `database` |
| ploke_embed | `IndexerTask::new` | `indexer` |
| ploke_embed | `IndexerTask::run` | `indexer` |
| ploke_embed | `EmbeddingRuntime::with_default_set` | `runtime` |

**Workflow/Order of Operations:**
1. Initialize database: `Database::init_with_schema()`
2. Setup multi-embedding: `db.setup_multi_embedding()`
3. Parse and merge crate: `try_run_phases_and_merge(crate_path)`
4. Extract merged graph and module tree
5. Transform to DB: `transform_parsed_graph(&db, merged_graph, &tree)`
6. Create embedding processor (mock or real based on flags)
7. Create embedding runtime: `EmbeddingRuntime::with_default_set(processor)`
8. Create indexer task: `IndexerTask::new(...)`
9. Run indexing: `task.run(progress_tx, control_rx).await`
10. Build HNSW index: `create_index_primary(&db)`

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `crate_path` | `PathBuf` | Path to crate root directory |
| `embedding_backend` | `String` | "mock", "local", "openrouter" |
| `batch_size` | `Option<usize>` | Override batch size for indexing |

**Output:**
- Success: Database with code graph and embeddings
- Progress updates via stdout
- Errors: Any from pipeline stages

**Error Handling:**
- Stop on first error
- Report progress percentage at each stage
- Allow cancellation during embedding phase

**Environment Variables:**
- `TEST_OPENROUTER_API_KEY` - Required when using OpenRouter backend

---

### 3. `pipeline workspace`

**Description:** Parse an entire workspace and transform all crates to database.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `parse_workspace` | `lib` |
| ploke_transform | `transform_parsed_workspace` | `transform::workspace` |
| ploke_db | `Database::init_with_schema` | `database` |

**Workflow/Order of Operations:**
1. Initialize database: `Database::init_with_schema()`
2. Parse workspace: `parse_workspace(workspace_path, selected_crates)`
3. Transform workspace: `transform_parsed_workspace(&db, parsed_workspace)`

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace_path` | `PathBuf` | Path to workspace root |
| `crates` | `Option<Vec<String>>` | Specific crates to parse (optional) |
| `db_path` | `Option<PathBuf>` | Optional database path |

**Output:**
- Success: Database with all workspace crates
- Summary of parsed crates

**Error Handling:**
- `MultipleErrors` aggregation from syn_parser
- Report per-crate success/failure
- Continue on individual crate failures when possible

---

### 4. `validate parse-integrity`

**Description:** Parse code and validate graph integrity without database operations.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `try_run_phases_and_merge` | `lib` |
| syn_parser | `ParsedCodeGraph` methods | `parser::graph` |

**Workflow/Order of Operations:**
1. Parse crate: `try_run_phases_and_merge(crate_path)`
2. Extract merged graph
3. Validate graph integrity:
   - Check for orphaned nodes
   - Verify relation consistency
   - Validate module tree structure
4. Output validation report

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `crate_path` | `PathBuf` | Path to crate root |
| `verbose` | `bool` | Show detailed validation output |

**Output:**
- Validation report with:
  - Node counts by type
  - Relation counts
  - Warnings for potential issues
  - Error count

---

### 5. `validate db-health`

**Description:** Comprehensive database health check with diagnostics.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| ploke_db | `count_pending_embeddings` | `database` |
| ploke_db | `count_unembedded_nonfiles` | `database` |
| ploke_db | `count_unembedded_files` | `database` |
| ploke_db | `count_relations` | `database` |
| ploke_db | `relations_vec` | `database` |

**Workflow/Order of Operations:**
1. Connect to database
2. Count relations in database
3. Count total nodes
4. Count embedding status:
   - Nodes with embeddings
   - Nodes pending embeddings
   - File nodes vs code nodes
5. Check HNSW index status
6. Check BM25 index status
7. Output health report

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `db_path` | `Option<PathBuf>` | Database path (or in-memory) |
| `detailed` | `bool` | Show relation-level details |

**Output:**
- Health report including:
  - Total relations count
  - Total nodes count
  - Embedding coverage percentage
  - Index status (HNSW, BM25)
  - Recommended actions

---

### 6. `validate end-to-end`

**Description:** Validate the entire pipeline from parse to database integrity.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `try_run_phases_and_merge` | `lib` |
| ploke_transform | `transform_parsed_graph` | `transform` |
| ploke_db | Various query functions | `database` |

**Workflow/Order of Operations:**
1. Parse crate: `try_run_phases_and_merge()`
2. Extract graph statistics (nodes, relations)
3. Transform to temporary database
4. Query database statistics
5. Compare parse stats with DB stats
6. Validate data consistency
7. Report any discrepancies

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `crate_path` | `PathBuf` | Path to crate |
| `verify_counts` | `bool` | Verify node/relation counts match |

**Output:**
- Comparison report:
  - Parsed nodes vs DB nodes
  - Parsed relations vs DB relations
  - Validation status (PASS/FAIL)

---

### 7. `diagnostic db-report`

**Description:** Generate comprehensive database diagnostic report.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| ploke_db | `run_script` | `database` (via Deref) |
| ploke_db | `count_edges_by_kind` | `helpers` |
| ploke_db | Various count functions | `database` |

**Workflow/Order of Operations:**
1. Connect to database
2. Query all relation names
3. Count records per relation
4. Count edges by kind
5. Query embedding statistics
6. Generate JSON or formatted report

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `db_path` | `Option<PathBuf>` | Database path |
| `output_format` | `String` | "json" or "table" |
| `output_file` | `Option<PathBuf>` | Write to file instead of stdout |

**Output:**
- Comprehensive report with:
  - All relations and row counts
  - Edge type distribution
  - Embedding statistics
  - Index information

---

### 8. `diagnostic embedding-status`

**Description:** Check embedding generation status across the pipeline.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| ploke_db | `count_pending_embeddings` | `database` |
| ploke_db | `count_complete_embeddings` | `multi_embedding::db_ext` |
| ploke_db | `active_embedding_set` | `database` |

**Workflow/Order of Operations:**
1. Get active embedding set
2. Count nodes needing embeddings
3. Count nodes with complete embeddings
4. Calculate coverage percentage
5. Report status

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `db_path` | `Option<PathBuf>` | Database path |

**Output:**
- Embedding status:
  - Active embedding set info
  - Nodes pending: N
  - Nodes complete: N
  - Coverage: N%

---

### 9. `setup test-env`

**Description:** Setup a complete test environment with fixtures.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| ploke_test_utils | `fresh_backup_fixture_db` | `fixture_dbs` |
| ploke_test_utils | `backup_db_fixture` | `fixture_dbs` |
| ploke_db | `create_index_primary` | `index::hnsw` |

**Workflow/Order of Operations:**
1. Lookup fixture by ID: `backup_db_fixture(fixture_id)`
2. Load fixture: `fresh_backup_fixture_db(fixture)`
3. Validate fixture contract
4. If requested, create HNSW index: `create_index_primary(&db)`
5. Output database path and status

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture_id` | `String` | Fixture identifier (e.g., "fixture_nodes_canonical") |
| `create_index` | `bool` | Create HNSW index after loading |
| `output_path` | `Option<PathBuf>` | Save database to path |

**Output:**
- Loaded database
- Fixture metadata
- Validation status

**Available Fixtures:**
- `fixture_nodes_canonical` - Standard fixture
- `fixture_nodes_local_embeddings` - With local embeddings
- `ploke_db_primary` - ploke-db crate backup
- `ws_fixture_01_canonical` - Workspace fixture

---

### 10. `setup dev-workspace`

**Description:** Setup a development workspace with full pipeline processing.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `parse_workspace` | `lib` |
| ploke_transform | `transform_parsed_workspace` | `transform::workspace` |
| ploke_db | `Database::init_with_schema` | `database` |
| ploke_db | `setup_multi_embedding` | `database` |

**Workflow/Order of Operations:**
1. Parse workspace: `parse_workspace(path, None)`
2. Initialize database with multi-embedding support
3. Transform workspace to DB
4. Setup BM25 service
5. Report workspace statistics

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace_path` | `PathBuf` | Workspace root directory |
| `db_path` | `Option<PathBuf>` | Database save path |

**Output:**
- Database with workspace code graph
- Parse statistics per crate
- Ready for embedding/indexing

---

### 11. `workflow reindex`

**Description:** Reindex a workspace (parse, transform, and regenerate embeddings).

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `parse_workspace` | `lib` |
| ploke_transform | `transform_parsed_workspace` | `transform::workspace` |
| ploke_db | Database initialization | `database` |
| ploke_embed | `IndexerTask` | `indexer` |

**Workflow/Order of Operations:**
1. Clear existing database or create new
2. Parse workspace fresh
3. Transform to database
4. Setup multi-embedding
5. Run indexer task
6. Build HNSW index
7. Rebuild BM25 index

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace_path` | `PathBuf` | Workspace root |
| `embedding_backend` | `String` | Backend to use |
| `preserve_db` | `bool` | Backup existing DB before reindex |

**Output:**
- New database with fresh index
- Reindex statistics
- Time elapsed per stage

---

### 12. `workflow regenerate-fixture`

**Description:** Regenerate a fixture database from source.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| ploke_test_utils | `setup_db_full_multi_embedding` | `lib` |
| ploke_test_utils | `FixtureAutomation` | `fixture_dbs` |
| ploke_db | `backup_db` (via cozo) | `database` |

**Workflow/Order of Operations:**
1. Parse fixture creation strategy
2. Setup database: `setup_db_full_multi_embedding(fixture_name)`
3. Optionally generate embeddings
4. Save to backup file: `db.db.backup_db(path)`
5. Update fixture metadata

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture_name` | `String` | Fixture to regenerate |
| `with_embeddings` | `bool` | Generate embeddings before saving |
| `output_path` | `Option<PathBuf>` | Override output path |

**Output:**
- New fixture database file
- Regeneration report

---

### 13. `compare parse-transform`

**Description:** Compare outputs between parse and transform stages.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `try_run_phases_and_merge` | `lib` |
| ploke_transform | `transform_parsed_graph` | `transform` |
| ploke_db | `run_script` | `database` |

**Workflow/Order of Operations:**
1. Parse crate
2. Extract parse statistics
3. Transform to temp database
4. Query database for transformed data
5. Generate comparison report

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `crate_path` | `PathBuf` | Crate to analyze |

**Output:**
- Side-by-side comparison:
  - Functions: parsed N, in DB N
  - Types: parsed N, in DB N
  - Relations: parsed N, in DB N

---

### 14. `debug graph-inspect`

**Description:** Inspect a parsed code graph with detailed output.

**Combines:**
| Crate | Function | Module |
|-------|----------|--------|
| syn_parser | `try_run_phases_and_resolve` | `lib` |
| syn_parser | `GraphAccess` trait | `parser::graph` |

**Workflow/Order of Operations:**
1. Parse without merging: `try_run_phases_and_resolve()`
2. Iterate through parsed graphs
3. Output detailed node information
4. Show relation mappings

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `crate_path` | `PathBuf` | Crate to inspect |
| `node_type` | `Option<String>` | Filter by node type |
| `output_format` | `String` | "tree", "list", or "json" |

**Output:**
- Detailed graph inspection
- Node listings with paths
- Relation mappings

---

## Required Crate Combinations Summary

| Command | Crates Required | Complexity |
|---------|-----------------|------------|
| `pipeline parse-transform` | syn_parser, ploke_transform, ploke_db | Medium |
| `pipeline full-ingest` | syn_parser, ploke_transform, ploke_db, ploke_embed | High |
| `pipeline workspace` | syn_parser, ploke_transform, ploke_db | Medium |
| `validate parse-integrity` | syn_parser | Low |
| `validate db-health` | ploke_db | Low |
| `validate end-to-end` | syn_parser, ploke_transform, ploke_db | Medium |
| `diagnostic db-report` | ploke_db | Low |
| `diagnostic embedding-status` | ploke_db | Low |
| `setup test-env` | ploke_test_utils, ploke_db | Low |
| `setup dev-workspace` | syn_parser, ploke_transform, ploke_db | Medium |
| `workflow reindex` | syn_parser, ploke_transform, ploke_db, ploke_embed | High |
| `workflow regenerate-fixture` | ploke_test_utils, syn_parser, ploke_transform, ploke_db | High |
| `compare parse-transform` | syn_parser, ploke_transform, ploke_db | Medium |
| `debug graph-inspect` | syn_parser | Low |

---

## Command Dependencies and Ordering

### Dependency Graph

```
parse-transform
├── parse (syn_parser)
└── transform (ploke_transform)

full-ingest
├── parse-transform
│   ├── parse
│   └── transform
└── embed (ploke_embed)
    └── index

workspace
├── parse_workspace (syn_parser)
└── transform_workspace (ploke_transform)

reindex
├── workspace (or parse-transform)
└── full-ingest (embeddings)

regenerate-fixture
├── setup_db_full (test-utils)
│   ├── parse-transform
│   └── multi-embedding schema
└── backup (cozo)
```

### Execution Order Rules

1. **Parse must precede Transform** - Cannot transform without parsed graph
2. **Transform must precede Embed** - Cannot index without database
3. **Database init must precede all DB operations** - Schema required
4. **Multi-embedding setup must precede IndexerTask** - Relations must exist
5. **HNSW requires embeddings** - Index built from embedding vectors

---

## Error Handling Patterns

### Pattern 1: Fail-Fast (Pipeline Commands)
```rust
let parsed = try_run_phases_and_merge(path)?;
let merged = parsed.extract_merged_graph().ok_or(...)?;
transform_parsed_graph(&db, merged, &tree)?;
// Any error stops the pipeline
```

### Pattern 2: Aggregate Errors (Validation Commands)
```rust
let mut errors = Vec::new();
if let Err(e) = check_thing_1() { errors.push(e); }
if let Err(e) = check_thing_2() { errors.push(e); }
// Report all errors at end
```

### Pattern 3: Cleanup on Failure
```rust
let result = try {
    let db = setup_db()?;
    transform(&db, graph)?;
    Ok(db)
};
if result.is_err() {
    // Cleanup temporary resources
}
```

---

## Recommended Implementation Priority

| Priority | Command | Rationale |
|----------|---------|-----------|
| 1 | `pipeline parse-transform` | Core workflow, most used |
| 1 | `setup test-env` | Essential for testing |
| 2 | `validate db-health` | Common diagnostic need |
| 2 | `diagnostic db-report` | Debugging aid |
| 3 | `pipeline full-ingest` | Complete workflow |
| 3 | `validate end-to-end` | Integration testing |
| 4 | `workflow reindex` | Maintenance operation |
| 4 | `workflow regenerate-fixture` | Fixture management |
| 5 | `compare parse-transform` | Advanced debugging |
| 5 | `debug graph-inspect` | Deep inspection |

---

## Notes for M.2 Architecture Design

1. **Shared Setup Code**: All pipeline commands need similar database initialization - extract to common function
2. **Progress Reporting**: Long-running commands (full-ingest, reindex) need progress output
3. **Cancellation Support**: Embedding commands should support graceful cancellation
4. **Output Formats**: Consider structured output (JSON) for programmatic consumption
5. **Temporary Resources**: Commands that create temp DBs should cleanup on failure
