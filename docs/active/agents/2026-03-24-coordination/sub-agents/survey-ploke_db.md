# Survey: ploke_db Crate for xtask Commands

**Date:** 2026-03-25  
**Sub-Agent:** ploke_db Survey  
**Task:** Document all functions relevant to database commands (A.4) from the README

---

## Files Touched During Survey

| File | Lines | Description |
|------|-------|-------------|
| `crates/ploke-db/src/database.rs` | ~4578 | Main database implementation |
| `crates/ploke-db/src/index/hnsw.rs` | ~330 | HNSW index functions |
| `crates/ploke-db/src/bm25_index/mod.rs` | ~1000 | BM25 indexer implementation |
| `crates/ploke-db/src/bm25_index/bm25_service.rs` | ~367 | BM25 service actor |
| `crates/ploke-db/src/error.rs` | ~172 | Error types |
| `crates/ploke-db/src/lib.rs` | ~49 | Public exports |
| `crates/ploke-db/src/multi_embedding/db_ext.rs` | ~600+ | Embedding extension trait |
| `crates/test-utils/src/fixture_dbs.rs` | ~500 | Fixture database utilities |
| `crates/test-utils/src/lib.rs` | ~562 | Test utilities |

---

## Function Documentation

### 1. Database::backup_db (save_db)

**Note:** This function is NOT directly implemented in `ploke_db`. Instead, it uses the underlying CozoDB's `backup_db` method via `Db<MemStorage>`.

**Signature:**
```rust
// From cozo::Db<MemStorage>
pub fn backup_db(&self, path: impl AsRef<Path>) -> Result<(), cozo::Error>
```

**Input Parameters:**
- `path`: Path to save the backup file (SQLite format)

**Output/Return Type:**
- `Result<(), cozo::Error>`

**Error Types:**
- `cozo::Error` - Underlying database error

**Special Considerations:**
- This is a method on the internal `cozo::Db<MemStorage>`, accessed via `Database.deref()`
- The backup is in SQLite format
- HNSW indices are NOT retained in backups (must be rebuilt after restore)

**Example Usage:**
```rust
let db = Database::init_with_schema()?;
let backup_path = PathBuf::from("backup.sqlite");
db.db.backup_db(&backup_path)?; // Note: db.db is the internal Cozo DB
```

---

### 2. Database::restore_backup (load_db)

**Note:** This function is NOT directly implemented in `ploke_db`. Instead, it uses the underlying CozoDB's `restore_backup` method.

**Signature:**
```rust
// From cozo::Db<MemStorage>
pub fn restore_backup(&self, path: impl AsRef<Path>) -> Result<(), cozo::Error>
```

**Input Parameters:**
- `path`: Path to the backup file to restore

**Output/Return Type:**
- `Result<(), cozo::Error>`

**Error Types:**
- `cozo::Error` - Underlying database error

**Special Considerations:**
- Database must be empty before restore (clear relations first)
- HNSW indices must be rebuilt after restore

**Example Usage:**
```rust
let db = cozo::Db::new(MemStorage::default())?;
db.restore_backup(&backup_path)?;
let db = Database::new(db);
```

---

### 3. fresh_backup_fixture_db (from test-utils)

**Full Path:** `ploke_test_utils::fresh_backup_fixture_db`

**Signature:**
```rust
pub fn fresh_backup_fixture_db(fixture: &'static FixtureDb) -> Result<Database, Error>
```

**Input Parameters:**
- `fixture`: `&'static FixtureDb` - A static fixture database definition

**Output/Return Type:**
- `Result<Database, Error>` - Returns a fresh database with the fixture loaded

**Error Types:**
- `Error::from(DbError::Cozo(...))` - If fixture file is missing
- `ploke_error::Error` - For other errors

**Key Types/Structs Needed:**
- `FixtureDb` - Struct containing fixture metadata
  - `id: &'static str`
  - `rel_path: &'static str` - Relative path to backup file
  - `import_mode: FixtureImportMode` - PlainBackup or BackupWithEmbeddings
  - `requires_primary_index: bool`

**Special Considerations:**
- Initializes database with schema before importing
- Handles two import modes: `PlainBackup` and `BackupWithEmbeddings`
- Validates fixture contract after loading (checks embeddings if expected)
- Creates HNSW index if `requires_primary_index` is true

**Example Usage:**
```rust
use ploke_test_utils::{fresh_backup_fixture_db, FIXTURE_NODES_CANONICAL};

let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?;
```

**Available Fixtures:**
- `FIXTURE_NODES_CANONICAL` - Standard fixture_nodes backup
- `FIXTURE_NODES_LOCAL_EMBEDDINGS` - With local embeddings
- `WS_FIXTURE_01_CANONICAL` - Workspace fixture with multiple crates
- `PLOKE_DB_PRIMARY` - ploke-db crate backup

---

### 4. create_index_primary (HNSW indexing)

**Full Path:** `ploke_db::index::hnsw::create_index_primary`

**Signature:**
```rust
pub fn create_index_primary(db: &Database) -> Result<(), DbError>
```

**Input Parameters:**
- `db: &Database` - Database instance

**Output/Return Type:**
- `Result<(), DbError>`

**Error Types:**
- `DbError::Cozo(String)` - CozoDB errors
- `DbError::ActiveSetPoisoned` - RwLock poisoned

**Key Types/Structs Needed:**
- `Database` - Main database struct
- `EmbeddingSet` - Active embedding set
- `EmbeddingExt` trait - For embedding operations
- `HnswExt` trait - For HNSW operations

**Special Considerations:**
- Uses the database's `active_embedding_set` field
- Creates embedding relations and HNSW indices
- Calls `create_index_for_set` internally
- Not instrumented with tracing

**Example Usage:**
```rust
use ploke_db::create_index_primary;

let db = Database::init_with_schema()?;
// ... load data ...
create_index_primary(&db)?;
```

---

### 5. create_index_primary_with_index (HNSW rebuild)

**Full Path:** `ploke_db::index::hnsw::create_index_primary_with_index`

**Signature:**
```rust
pub fn create_index_primary_with_index(db: &Database) -> Result<(), DbError>
```

**Input Parameters:**
- `db: &Database` - Database instance

**Output/Return Type:**
- `Result<(), DbError>`

**Error Types:**
- `DbError::Cozo(String)` - CozoDB errors
- `DbError::ActiveSetPoisoned` - RwLock poisoned

**Special Considerations:**
- Same as `create_index_primary` - both functions do the same thing currently
- TODO: Replace with a Database method (see code comment)
- Not instrumented with tracing

**Example Usage:**
```rust
use ploke_db::create_index_primary_with_index;

let db = Database::init_with_schema()?;
// ... load data ...
create_index_primary_with_index(&db)?;
```

**Test Example:**
```rust
#[tokio::test]
async fn test_hnsw_init_from_backup() -> Result<(), Error> {
    let db = Database::init_with_schema()?;
    let prior_rels_vec = db.relations_vec()?;
    db.import_from_backup(&target_file, &prior_rels_vec)?;
    
    super::create_index_primary_with_index(&db)?;  // Create HNSW index
    
    let k = 20;
    let ef = 40;
    hnsw_all_types(&db, k, ef)?;  // Use the index
    Ok(())
}
```

---

### 6. BM25 Index Rebuild Functions

#### Bm25Indexer::rebuild_from_db

**Full Path:** `ploke_db::bm25_index::Bm25Indexer::rebuild_from_db`

**Signature:**
```rust
pub fn rebuild_from_db(db: &Database) -> Result<Self, DbError>
```

**Input Parameters:**
- `db: &Database` - Database instance

**Output/Return Type:**
- `Result<Bm25Indexer, DbError>`

**Error Types:**
- `DbError::Cozo(String)` - CozoDB errors
- `DbError::QueryExecution(String)` - Query execution errors

**Key Types/Structs Needed:**
- `Bm25Indexer` - In-memory BM25 indexer
- `DocMeta` - Document metadata struct
- `CodeTokenizer` - Code-aware tokenizer

**Special Considerations:**
- Scans all primary node relations for (id, name, tracking_hash)
- Computes avgdl (average document length) from corpus
- Builds new embedder with fitted avgdl
- Uses identifier name doubled as lightweight snippet representation
- Not async

**Example Usage:**
```rust
use ploke_db::bm25_index::Bm25Indexer;

let idx = Bm25Indexer::rebuild_from_db(&db)?;
let results = idx.search("main", 5, RetrievalScope::LoadedWorkspace);
```

#### bm25_service::start_rebuilt

**Full Path:** `ploke_db::bm25_index::bm25_service::start_rebuilt`

**Signature:**
```rust
pub fn start_rebuilt(db: Arc<Database>) -> Result<mpsc::Sender<Bm25Cmd>, DbError>
```

**Input Parameters:**
- `db: Arc<Database>` - Database instance (Arc for sharing)

**Output/Return Type:**
- `Result<mpsc::Sender<Bm25Cmd>, DbError>` - Channel sender for BM25 commands

**Error Types:**
- `DbError::Cozo(String)` - CozoDB errors

**Special Considerations:**
- Starts BM25 actor with rebuilt index from database
- Uses `Bm25Indexer::rebuild_from_db` internally
- Returns mpsc channel for sending commands
- Async actor runs in tokio task

**Example Usage:**
```rust
use ploke_db::bm25_index::bm25_service;

let tx = bm25_service::start_rebuilt(Arc::new(db))?;
// Now you can issue search commands
```

#### Bm25Cmd::Rebuild

**Command enum for actor:**
```rust
pub enum Bm25Cmd {
    Rebuild,
    // ... other variants
}
```

---

### 7. Database::run_script (arbitrary query)

**Full Path:** `ploke_db::Database::run_script`

**Signature:**
```rust
// Inherited from cozo::Db<MemStorage> via Deref
pub fn run_script(
    &self,
    script: &str,
    params: BTreeMap<String, DataValue>,
    mutability: ScriptMutability,
) -> Result<NamedRows, cozo::Error>
```

**Input Parameters:**
- `script: &str` - CozoScript query string
- `params: BTreeMap<String, DataValue>` - Query parameters
- `mutability: ScriptMutability` - Immutable or Mutable

**Output/Return Type:**
- `Result<NamedRows, cozo::Error>`

**Error Types:**
- `cozo::Error` - CozoDB errors

**Key Types/Structs Needed:**
- `DataValue` - Cozo data value enum
- `ScriptMutability` - Enum: `Immutable` or `Mutable`
- `NamedRows` - Query result with headers and rows

**Special Considerations:**
- Inherited from `cozo::Db<MemStorage>` via `Deref` impl
- Use `BTreeMap::new()` for empty params
- For immutable queries, prefer `raw_query` wrapper

**Example Usage:**
```rust
use cozo::{ScriptMutability, DataValue};
use std::collections::BTreeMap;

// Simple immutable query
let result = db.run_script(
    "?[count(id)] := *function { id }",
    BTreeMap::new(),
    ScriptMutability::Immutable
)?;

// With parameters
let mut params = BTreeMap::new();
params.insert("name".to_string(), DataValue::from("foo"));
let result = db.run_script(
    "?[id] := *function { id, name }, name = $name",
    params,
    ScriptMutability::Immutable
)?;
```

**Helper Methods:**
- `raw_query(&self, script: &str) -> Result<QueryResult, DbError>` - Immutable queries
- `raw_query_mut(&self, script: &str) -> Result<QueryResult, DbError>` - Mutable queries

---

### 8. Node Counting Functions

#### count_pending_embeddings

**Full Path:** `ploke_db::Database::count_pending_embeddings`

**Signature:**
```rust
pub fn count_pending_embeddings(&self) -> Result<usize, DbError>
```

**Description:** Counts all nodes that need embeddings (primary nodes without embeddings).

**Error Types:**
- `DbError::ActiveSetPoisoned`
- `DbError::Cozo(String)`

---

#### count_unembedded_nonfiles

**Full Path:** `ploke_db::Database::count_unembedded_nonfiles`

**Signature:**
```rust
pub fn count_unembedded_nonfiles(&self) -> Result<usize, DbError>
```

**Description:** Counts non-file nodes that need embeddings.

---

#### count_unembedded_files

**Full Path:** `ploke_db::Database::count_unembedded_files`

**Signature:**
```rust
pub fn count_unembedded_files(&self) -> Result<usize, DbError>
```

**Description:** Counts file (module) nodes that need embeddings.

---

#### count_common_nodes

**Full Path:** `ploke_db::multi_embedding::db_ext::EmbeddingExt::count_common_nodes`

**Signature:**
```rust
fn count_common_nodes(&self) -> Result<usize, PlokeError>;
```

**Description:** Counts common embedding nodes (primary code items).

**Implemented on:** `cozo::Db<MemStorage>`

---

#### count_complete_embeddings

**Full Path:** `ploke_db::multi_embedding::db_ext::EmbeddingExt::count_complete_embeddings`

**Signature:**
```rust
fn count_complete_embeddings(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError>;
```

**Description:** Counts nodes that have complete embeddings for the given set.

---

#### count_embeddings_for_set

**Full Path:** `ploke_db::multi_embedding::db_ext::EmbeddingExt::count_embeddings_for_set`

**Signature:**
```rust
fn count_embeddings_for_set(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError>;
```

**Description:** Counts total embeddings stored for a specific embedding set.

---

#### count_relations

**Full Path:** `ploke_db::Database::count_relations`

**Signature:**
```rust
pub async fn count_relations(&self) -> Result<usize, PlokeError>
```

**Description:** Counts total number of relations in the database.

**Special Considerations:**
- Async function
- Includes both system and user-defined relations

---

#### count_edges_by_kind

**Full Path:** `ploke_db::helpers::count_edges_by_kind`

**Signature:**
```rust
pub fn count_edges_by_kind(db: &Database, relation_kind: &str) -> Result<usize, DbError>
```

**Description:** Counts edges by their relation kind.

---

## Types and Structs Identified

### Core Database Types

| Type | Module | Description |
|------|--------|-------------|
| `Database` | `database` | Main database struct wrapping CozoDB |
| `DbError` | `error` | Error enum for ploke_db |
| `QueryResult` | `result` | Wrapper around Cozo NamedRows |
| `NodeType` | `query::builder` | Enum of node types (Function, Struct, etc.) |

### Embedding Types

| Type | Module | Description |
|------|--------|-------------|
| `EmbeddingSet` | `ploke_core::embeddings` | Identifier for embedding model/dims |
| `TypedEmbedData` | `database` | Embedding data with node type |
| `EmbeddingData` | `ploke_core` | Metadata for embedding processing |

### BM25 Types

| Type | Module | Description |
|------|--------|-------------|
| `Bm25Indexer` | `bm25_index` | In-memory BM25 indexer |
| `DocMeta` | `bm25_index` | Document metadata |
| `DocData` | `bm25_index` | Document data with snippet |
| `Bm25Cmd` | `bm25_index::bm25_service` | Command enum for BM25 actor |
| `Bm25Status` | `bm25_index::bm25_service` | Status enum |

### Fixture Types

| Type | Module | Description |
|------|--------|-------------|
| `FixtureDb` | `test-utils::fixture_dbs` | Fixture database definition |
| `FixtureImportMode` | `test-utils::fixture_dbs` | PlainBackup / BackupWithEmbeddings |
| `FixtureAccess` | `test-utils::fixture_dbs` | ImmutableShared / FreshMutable |

### CozoDB Types (External)

| Type | Crate | Description |
|------|-------|-------------|
| `Db<MemStorage>` | `cozo` | In-memory CozoDB instance |
| `DataValue` | `cozo` | Cozo data value enum |
| `NamedRows` | `cozo` | Query result |
| `ScriptMutability` | `cozo` | Immutable/Mutable |

---

## Issues Encountered

### 1. Missing Direct backup_db/restore_backup Methods
- **Issue:** `Database` struct doesn't directly implement `backup_db` or `restore_backup`
- **Workaround:** Access via `db.db.backup_db()` using `Deref` to inner `cozo::Db<MemStorage>`
- **Recommendation:** Consider wrapping these for consistency

### 2. create_index_primary vs create_index_primary_with_index
- **Issue:** Both functions do the same thing currently
- **Code Comment:** "TODO:ploke-db 2025-12-16 - Replace this function with a Database method"
- **Recommendation:** Consolidate or differentiate

### 3. Async vs Sync Functions
- **Issue:** Mix of async and sync functions
  - `count_relations()` is async
  - `create_index_primary()` is sync
  - `fresh_backup_fixture_db()` is sync
- **Recommendation:** Document clearly which are async

### 4. Tracing Instrumentation
- **Issue:** Most functions NOT instrumented with `#[instrument]`
- **Exceptions:**
  - `search_similar` in `hnsw.rs` has `#[instrument(skip_all, fields(query_result))]`
  - `search_similar_args` in `hnsw.rs` has instrumentation
- **Recommendation:** Add tracing instrumentation for xtask commands

### 5. Error Type Inconsistencies
- **Issue:** Mix of `DbError` and `ploke_error::Error`
- Some functions return `DbError`, others convert to `ploke_error::Error`
- **Recommendation:** Document which error type each function returns

---

## Notes on Tracing Instrumentation Status

| Function | Instrumented | Notes |
|----------|--------------|-------|
| `create_index_primary` | ❌ | No tracing |
| `create_index_primary_with_index` | ❌ | No tracing |
| `create_index_for_set` | ❌ | No tracing |
| `fresh_backup_fixture_db` | ❌ | No tracing (in test-utils) |
| `Bm25Indexer::rebuild_from_db` | ❌ | No tracing |
| `bm25_service::start_rebuilt` | ❌ | Internal tracing only |
| `run_script` | ❌ | Inherited from CozoDB |
| `count_pending_embeddings` | ❌ | No tracing |
| `count_unembedded_nonfiles` | ❌ | No tracing |
| `count_unembedded_files` | ❌ | No tracing |
| `search_similar` | ✅ | `#[instrument(skip_all, fields(query_result))]` |
| `search_similar_args` | ✅ | `#[instrument(skip_all, fields(query_result))]` |

---

## Summary Table for Architecture Design

| Command | Function | Path | Async | Error Type | Tracing |
|---------|----------|------|-------|------------|---------|
| `db save` | `backup_db` (cozo) | `db.db.backup_db()` | ❌ | `cozo::Error` | ❌ |
| `db load` | `restore_backup` (cozo) | `db.db.restore_backup()` | ❌ | `cozo::Error` | ❌ |
| `db load-fixture` | `fresh_backup_fixture_db` | `test-utils::fixture_dbs` | ❌ | `ploke_error::Error` | ❌ |
| `db count-nodes` | `count_pending_embeddings` | `Database::count_pending_embeddings` | ❌ | `DbError` | ❌ |
| `db hnsw-build` | `create_index_primary` | `ploke_db::index::hnsw` | ❌ | `DbError` | ❌ |
| `db hnsw-rebuild` | `create_index_primary_with_index` | `ploke_db::index::hnsw` | ❌ | `DbError` | ❌ |
| `db bm25-rebuild` | `Bm25Indexer::rebuild_from_db` | `ploke_db::bm25_index` | ❌ | `DbError` | ❌ |
| `db query` | `run_script` | `Database` (via Deref) | ❌ | `cozo::Error` | ❌ |

---

## Recommendations for M.2 Architecture

1. **Wrap CozoDB backup/restore** - Create `Database::backup_db()` and `Database::restore_backup()` wrappers
2. **Add unified error handling** - Consider a single error type for xtask commands
3. **Add tracing instrumentation** - Add `#[instrument]` to all command functions
4. **Create async variants** - For consistency, consider async versions of sync functions
5. **Document fixture management** - Clear docs on when to use which fixture
