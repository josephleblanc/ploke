# Additional ploke_db Functions for xtask Commands

**Date:** 2026-03-25  
**Task:** M.1.2 - Identify additional key functions for agent diagnostics  
**Sub-Agent:** ploke_db Additional Survey

---

## Overview

This document identifies additional functions in the `ploke_db` crate that would be valuable for agent diagnostics but were not included in the original A.4 list from the README. These functions provide capabilities for database introspection, index status checking, workspace metadata queries, and integrity validation.

---

## Additional Functions Identified

### 1. Database Statistics & Node Counting

#### `Database::count_complete_embeddings`
**Location:** `ploke_db::multi_embedding::db_ext::EmbeddingExt` (line 183)

**Signature:**
```rust
fn count_complete_embeddings(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError>;
```

**Diagnostic Value:**
- Counts nodes that have complete embeddings for the active embedding set
- Useful for verifying embedding coverage after ingestion
- Can be compared with `count_pending_embeddings` to calculate completion percentage

**Suggested Command:** `db stats embeddings`

**Input/Output:**
- Input: Database handle (uses active embedding set internally)
- Output: `usize` count of nodes with complete embeddings

---

#### `Database::count_embeddings_for_set`
**Location:** `ploke_db::multi_embedding::db_ext::EmbeddingExt` (line 186)

**Signature:**
```rust
fn count_embeddings_for_set(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError>;
```

**Diagnostic Value:**
- Counts total embeddings stored for a specific embedding set
- Useful when multiple embedding sets exist in the database
- Helps verify multi-embedding setup integrity

**Suggested Command:** `db stats embeddings --set <set_id>`

---

#### `Database::count_common_nodes`
**Location:** `ploke_db::multi_embedding::db_ext::EmbeddingExt` (line 190)

**Signature:**
```rust
fn count_common_nodes(&self) -> Result<usize, PlokeError>;
```

**Diagnostic Value:**
- Counts common embedding nodes (primary code items that can be embedded)
- Provides baseline for understanding total embeddable content
- Useful for capacity planning and progress tracking

**Suggested Command:** `db stats nodes --embeddable`

---

#### `helpers::count_edges_by_kind`
**Location:** `ploke_db::helpers` (line 288)

**Signature:**
```rust
pub fn count_edges_by_kind(db: &Database, relation_kind: &str) -> Result<usize, DbError>
```

**Diagnostic Value:**
- Counts syntax edges by their relation kind (Contains, ResolvesToDefinition, etc.)
- Useful for understanding graph structure and connectivity
- Can identify orphaned nodes or verify relation integrity

**Suggested Command:** `db stats edges --kind <kind>`

---

### 2. Index Status Functions

#### `HnswExt::is_hnsw_index_registered`
**Location:** `ploke_db::multi_embedding::hnsw_ext` (line 71, 463)

**Signature:**
```rust
fn is_hnsw_index_registered(&self, embedding_set: &EmbeddingSet) -> Result<bool, DbError>;
```

**Diagnostic Value:**
- Checks if HNSW index exists for a given embedding set
- Critical for verifying semantic search readiness
- Can detect when index needs rebuilding after backup restore

**Suggested Command:** `db hnsw-status`

---

#### `EmbeddingExt::is_embedding_set_registered`
**Location:** `ploke_db::multi_embedding::db_ext` (line 101, 335)

**Signature:**
```rust
fn is_embedding_set_registered(&self) -> Result<bool, DbError>;
```

**Diagnostic Value:**
- Checks if the `embedding_set` metadata relation exists
- Fundamental check for multi-embedding system health
- Should be true after proper database initialization

**Suggested Command:** `db check-schema`

---

#### `EmbeddingExt::is_relation_registered`
**Location:** `ploke_db::multi_embedding::db_ext` (line 91, 306)

**Signature:**
```rust
fn is_relation_registered(&self, relation_name: &EmbRelName) -> Result<bool, DbError>;
```

**Diagnostic Value:**
- Generic relation existence check
- Used internally but valuable for debugging schema issues
- Can verify specific embedding vector relations

**Suggested Command:** `db check-relation <name>`

---

#### `Database::list_embedding_sets`
**Location:** `ploke_db::database` (line 2093)

**Signature:**
```rust
pub fn list_embedding_sets(&self) -> Result<Vec<EmbeddingSet>, DbError>;
```

**Diagnostic Value:**
- Lists all embedding sets registered in the database
- Essential for multi-model embedding scenarios
- Shows provider, model, dimensions for each set

**Suggested Command:** `db list-embedding-sets`

---

#### `Database::list_embedding_vector_relations`
**Location:** `ploke_db::database` (line 1003)

**Signature:**
```rust
fn list_embedding_vector_relations(&self) -> Result<Vec<String>, DbError>;
```

**Diagnostic Value:**
- Lists all vector storage relations
- Maps to actual database relation names
- Useful for low-level debugging

**Suggested Command:** `db list-vector-relations`

---

### 3. Namespace & Workspace Queries

#### `Database::list_crate_context_rows`
**Location:** `ploke_db::database` (line 2202)

**Signature:**
```rust
pub fn list_crate_context_rows(&self) -> Result<Vec<CrateContextRow>, DbError>;
```

**Diagnostic Value:**
- Lists all crates loaded in the database
- Shows namespace, root_path for each crate
- Foundation for workspace introspection

**Suggested Command:** `db list-crates`

**Output Type:**
```rust
pub struct CrateContextRow {
    pub id: Uuid,
    pub name: String,
    pub namespace: Uuid,
    pub root_path: String,
}
```

---

#### `Database::collect_namespace_inventory`
**Location:** `ploke_db::database` (line 2241)

**Signature:**
```rust
pub fn collect_namespace_inventory(&self, namespace: Uuid) -> Result<NamespaceInventory, DbError>;
```

**Diagnostic Value:**
- Comprehensive namespace analysis
- Lists all file modules and descendant nodes
- Useful for understanding loaded codebase structure

**Suggested Command:** `db namespace-info <namespace>`

**Output Type:**
```rust
pub struct NamespaceInventory {
    pub crate_context: CrateContextRow,
    pub file_module_owner_ids: BTreeSet<Uuid>,
    pub descendant_ids: BTreeSet<Uuid>,
}
```

---

#### `Database::get_crate_files`
**Location:** `ploke_db::database` (line 1381)

**Signature:**
```rust
pub fn get_crate_files(&self, crate_name: &str) -> Result<Vec<FileData>, PlokeError>;
```

**Diagnostic Value:**
- Gets all file data for a specific crate
- Includes tracking hashes for change detection
- Useful for file-based operations and validation

**Suggested Command:** `db crate-files <crate_name>`

---

#### `Database::get_path_info`
**Location:** `ploke_db::database` (line 2613)

**Signature:**
```rust
pub fn get_path_info(&self, path: &str) -> Result<QueryResult, PlokeError>;
```

**Diagnostic Value:**
- Queries database for a specific file path
- Returns module information for that path
- Useful for file-to-module resolution

**Suggested Command:** `db path-info <path>`

---

### 4. Database Introspection

#### `Database::iter_relations` / `Database::relations_vec`
**Location:** `ploke_db::database` (line 2156, 2165)

**Signature:**
```rust
pub fn iter_relations(&self) -> Result<impl IntoIterator<Item = String>, PlokeError>;
pub fn relations_vec(&self) -> Result<Vec<String>, PlokeError>;
```

**Diagnostic Value:**
- Lists all relations in the database
- Essential for understanding database schema
- Used internally for backup/restore operations

**Suggested Command:** `db list-relations`

---

#### `Database::relations_vec_no_hnsw`
**Location:** `ploke_db::database` (line 2170)

**Signature:**
```rust
pub fn relations_vec_no_hnsw(&self) -> Result<Vec<String>, PlokeError>;
```

**Diagnostic Value:**
- Lists relations excluding HNSW indices
- Useful for backup operations (HNSW not preserved)
- Shows actual data relations vs. search indices

**Suggested Command:** `db list-relations --no-hnsw`

---

#### `Database::rel_names_with_tracking_hash`
**Location:** `ploke_db::database` (line 1208)

**Signature:**
```rust
pub fn rel_names_with_tracking_hash(&self) -> Result<Vec<String>, DbError>;
```

**Diagnostic Value:**
- Lists relations that have tracking_hash column
- Important for change detection features
- Shows which nodes support hash-based validation

**Suggested Command:** `db list-tracked-relations`

---

#### `Database::get_file_data`
**Location:** `ploke_db::database` (line 2763)

**Signature:**
```rust
pub fn get_file_data(&self) -> Result<Vec<FileData>, PlokeError>;
```

**Diagnostic Value:**
- Gets all file data across all crates
- Includes tracking hashes and namespaces
- Foundation for file-based operations

**Suggested Command:** `db list-files`

---

#### `helpers::list_primary_nodes`
**Location:** `ploke_db::helpers` (line 170)

**Signature:**
```rust
pub fn list_primary_nodes(db: &Database) -> Result<Vec<PrimaryNodeRow>, DbError>;
```

**Diagnostic Value:**
- Lists all primary nodes that have embeddings
- Shows relation type, name, file path, module path
- Useful for browsing embedded content

**Suggested Command:** `db list-embedded-nodes`

**Output Type:**
```rust
pub struct PrimaryNodeRow {
    pub relation: String,
    pub name: String,
    pub file_path: PathBuf,
    pub module_path: Vec<String>,
}
```

---

### 5. BM25 Index Status

#### `Bm25Indexer::doc_count` / `Bm25Indexer::is_empty`
**Location:** `ploke_db::bm25_index` (lines 707-714)

**Signature:**
```rust
pub fn doc_count(&self) -> usize;
pub fn is_empty(&self) -> bool;
```

**Diagnostic Value:**
- Reports number of documents in BM25 index
- Essential for text search readiness check
- `is_empty()` quick check for index status

**Suggested Command:** `db bm25-status`

---

#### `Bm25Status` (via bm25_service)
**Location:** `ploke_db::bm25_index::bm25_service` (line 11)

**Variants:**
```rust
pub enum Bm25Status {
    Uninitialized,
    Building,
    Ready { docs: usize },
    Empty,
    Error(String),
}
```

**Diagnostic Value:**
- Full lifecycle status of BM25 actor
- Distinguishes between empty and uninitialized states
- Reports errors if index build failed

**Suggested Command:** `db bm25-status --verbose`

---

### 6. Integrity & Validation

#### `Database::validate_namespace_import_conflicts`
**Location:** `ploke_db::database` (line 727)

**Signature:**
```rust
fn validate_namespace_import_conflicts(&self, artifact: &NamespaceExportArtifact) -> Result<(), NamespaceImportError>;
```

**Diagnostic Value:**
- Pre-flight check for namespace import operations
- Detects duplicate namespaces, crate names, root paths
- Prevents database corruption from conflicting imports

**Suggested Command:** `db validate-import <artifact_path>`

**Output Type:**
```rust
pub struct NamespaceImportConflictReport {
    pub duplicate_namespace: Option<Uuid>,
    pub duplicate_crate_name: Option<String>,
    pub duplicate_root_path: Option<String>,
    pub workspace_root_mismatch: Option<(String, String)>,
}
```

---

#### `Database::restore_embedding_set`
**Location:** `ploke_db::database` (line 1967)

**Signature:**
```rust
pub fn restore_embedding_set(&self, crate_name: &str) -> Result<Option<(EmbeddingSet, RestoredEmbeddingSet)>, DbError>;
```

**Diagnostic Value:**
- Validates and restores active embedding set after backup
- Checks metadata consistency
- Falls back to first populated set if metadata missing

**Suggested Command:** `db verify-embedding-set [crate_name]`

---

#### `Database::with_active_set`
**Location:** `ploke_db::database` (line 1179)

**Signature:**
```rust
pub fn with_active_set<R>(&self, f: impl FnOnce(&EmbeddingSet) -> R) -> Result<R, DbError>;
```

**Diagnostic Value:**
- Safe accessor for active embedding set
- Reports poisoning errors if RwLock is corrupted
- Shows current model/provider configuration

**Suggested Command:** `db active-embedding-set`

---

## Summary Table

| Category | Function | Suggested Command | Priority |
|----------|----------|-------------------|----------|
| **Stats** | `count_complete_embeddings` | `db stats embeddings` | High |
| **Stats** | `count_embeddings_for_set` | `db stats embeddings --set` | Medium |
| **Stats** | `count_common_nodes` | `db stats nodes` | Medium |
| **Stats** | `count_edges_by_kind` | `db stats edges` | Low |
| **Index** | `is_hnsw_index_registered` | `db hnsw-status` | High |
| **Index** | `is_embedding_set_registered` | `db check-schema` | High |
| **Index** | `list_embedding_sets` | `db list-embedding-sets` | High |
| **Index** | `Bm25Indexer::doc_count` | `db bm25-status` | High |
| **Namespace** | `list_crate_context_rows` | `db list-crates` | High |
| **Namespace** | `collect_namespace_inventory` | `db namespace-info` | Medium |
| **Namespace** | `get_crate_files` | `db crate-files` | Medium |
| **Introspection** | `relations_vec` | `db list-relations` | High |
| **Introspection** | `list_primary_nodes` | `db list-embedded-nodes` | Medium |
| **Introspection** | `get_file_data` | `db list-files` | Medium |
| **Validation** | `validate_namespace_import_conflicts` | `db validate-import` | Low |
| **Validation** | `restore_embedding_set` | `db verify-embedding-set` | Medium |

---

## Integration Notes

### Error Handling
Most functions return `Result<T, DbError>` or `Result<T, PlokeError>`. The xtask commands should:
1. Convert these to user-friendly error messages
2. Distinguish between "not found" and actual errors
3. Provide context about which diagnostic check failed

### Async Considerations
- `count_relations()` is async
- Most other diagnostic functions are synchronous
- The BM25 service requires async actor communication

### Database Access
All functions require a `Database` instance. For xtask:
1. Initialize database via `Database::init_with_schema()` or similar
2. Handle the case where database doesn't exist yet
3. Provide `--database <path>` option for custom locations

### Output Formatting
Suggested output formats:
- **Table**: For list commands (crates, relations, embedding sets)
- **JSON**: For machine-readable output (`--json` flag)
- **Human-readable**: Default format with counts and status indicators

---

## Relationship to Original A.4 Commands

These additional functions complement the original A.4 commands:

| Original Command | Additional Complements |
|------------------|------------------------|
| `db count-nodes` | `count_complete_embeddings`, `count_common_nodes` |
| `db hnsw-build` | `is_hnsw_index_registered`, `hnsw-status` |
| `db bm25-rebuild` | `bm25-status`, `Bm25Indexer::doc_count` |
| `db load-fixture` | `validate_namespace_import_conflicts` |
| `db query` | `relations_vec`, `list_primary_nodes` |
