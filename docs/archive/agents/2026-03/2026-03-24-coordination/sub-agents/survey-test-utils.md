# Survey: ploke_test_utils Crate

**Date:** 2026-03-25  
**Task:** Survey ploke_test_utils for fixture loading and database setup utilities  
**Agent:** Sub-agent for M.1 Survey  

---

## Files Touched During Survey

| File | Path | Lines | Description |
|------|------|-------|-------------|
| `lib.rs` | `crates/test-utils/src/lib.rs` | 562 | Main library with database setup utilities |
| `fixture_dbs.rs` | `crates/test-utils/src/fixture_dbs.rs` | 500 | Fixture database loading and management |
| `nodes.rs` | `crates/test-utils/src/nodes.rs` | 181 | Test helper types for paranoid tests |
| `Cargo.toml` | `crates/test-utils/Cargo.toml` | 39 | Crate dependencies and features |

---

## Key Functions for `db load-fixture` Command

### 1. `fresh_backup_fixture_db`

**Full Path:** `ploke_test_utils::fresh_backup_fixture_db`  
**Location:** `fixture_dbs.rs:303-328`

```rust
pub fn fresh_backup_fixture_db(fixture: &'static FixtureDb) -> Result<Database, Error>
```

**Purpose:** Loads a fresh database instance from a backup fixture file. This is the primary function for the `db load-fixture` command.

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture` | `&'static FixtureDb` | The fixture metadata struct with path, import mode, etc. |

**Output/Return:**
- `Result<Database, Error>` - Returns a `ploke_db::Database` on success

**Error Types:**
- `ploke_error::Error` - Wraps `DbError::Cozo` if fixture file missing
- `DbError` from import operations

**Special Considerations:**
- Creates a fresh `Database::init_with_schema()` before importing
- Supports two import modes based on `fixture.import_mode`:
  - `PlainBackup`: Uses `db.import_from_backup()` with relation filtering
  - `BackupWithEmbeddings`: Uses `db.import_backup_with_embeddings()`
- Validates the fixture contract after loading via `validate_backup_fixture_contract()`
- **NOT async** - synchronous operation

**Example Usage:**
```rust
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};

let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
    .expect("Failed to load fixture");
```

---

### 2. `shared_backup_fixture_db`

**Full Path:** `ploke_test_utils::shared_backup_fixture_db`  
**Location:** `fixture_dbs.rs:330-349`

```rust
pub fn shared_backup_fixture_db(fixture: &'static FixtureDb) -> Result<Arc<Database>, Error>
```

**Purpose:** Returns a cached/shared database instance for immutable fixtures to avoid reloading.

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture` | `&'static FixtureDb` | The fixture metadata struct |

**Output/Return:**
- `Result<Arc<Database>, Error>` - Thread-safe reference to shared database

**Special Considerations:**
- Uses a static `Mutex<HashMap<&str, Arc<Database>>>` for caching
- First call loads and caches the fixture
- Subsequent calls return cached `Arc<Database>` clone
- Thread-safe (uses `Arc` and `Mutex`)
- **NOT async** - synchronous operation

**Example Usage:**
```rust
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, shared_backup_fixture_db};

let db = shared_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?;
// Subsequent calls return cached instance
let db2 = shared_backup_fixture_db(&FIXTURE_NODES_CANONICAL)?; // Same underlying DB
```

---

### 3. `backup_db_fixture`

**Full Path:** `ploke_test_utils::backup_db_fixture`  
**Location:** `fixture_dbs.rs:296-301`

```rust
pub fn backup_db_fixture(id: &str) -> Option<&'static FixtureDb>
```

**Purpose:** Look up a fixture by its ID string from the `BACKUP_DB_FIXTURES` registry.

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `id` | `&str` | The fixture ID (e.g., "fixture_nodes_canonical") |

**Output/Return:**
- `Option<&'static FixtureDb>` - Returns `Some(&FixtureDb)` if found, `None` otherwise

**Special Considerations:**
- Searches `BACKUP_DB_FIXTURES` static array
- Used for command-line fixture selection by ID
- **NOT async** - synchronous operation

**Example Usage:**
```rust
use ploke_test_utils::backup_db_fixture;

if let Some(fixture) = backup_db_fixture("fixture_nodes_canonical") {
    let db = fresh_backup_fixture_db(fixture)?;
}
```

---

### 4. `all_backup_db_fixtures`

**Full Path:** `ploke_test_utils::all_backup_db_fixtures`  
**Location:** `fixture_dbs.rs:285-287`

```rust
pub fn all_backup_db_fixtures() -> &'static [&'static FixtureDb]
```

**Purpose:** Returns all registered backup fixtures (active, legacy, and orphaned).

**Output/Return:**
- `&'static [&'static FixtureDb]` - Slice of all fixture references

**Example Usage:**
```rust
use ploke_test_utils::all_backup_db_fixtures;

for fixture in all_backup_db_fixtures() {
    println!("Fixture: {} at {:?}", fixture.id, fixture.path());
}
```

---

### 5. `active_backup_db_fixtures`

**Full Path:** `ploke_test_utils::active_backup_db_fixtures`  
**Location:** `fixture_dbs.rs:289-294`

```rust
pub fn active_backup_db_fixtures() -> impl Iterator<Item = &'static FixtureDb>
```

**Purpose:** Returns an iterator over only active (non-legacy, non-orphaned) fixtures.

**Output/Return:**
- `impl Iterator<Item = &'static FixtureDb>` - Iterator of active fixtures

**Example Usage:**
```rust
use ploke_test_utils::active_backup_db_fixtures;

for fixture in active_backup_db_fixtures() {
    // Only processes active fixtures
    validate_fixture(fixture)?;
}
```

---

### 6. `validate_backup_fixture_contract`

**Full Path:** `ploke_test_utils::validate_backup_fixture_contract`  
**Location:** `fixture_dbs.rs:351-375`

```rust
pub fn validate_backup_fixture_contract(fixture: &FixtureDb, db: &Database) -> Result<(), Error>
```

**Purpose:** Validates that a loaded fixture meets its contract (embeddings, indexes, etc.).

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture` | `&FixtureDb` | Fixture metadata with expectations |
| `db` | `&Database` | The loaded database to validate |

**Output/Return:**
- `Result<(), Error>` - Ok if validation passes

**Validation Checks:**
- If `fixture.embedding` is specified:
  - Sets active embedding set if `active_set_expected`
  - Verifies embeddings exist if `vectors_present`
- If `fixture.requires_primary_index`:
  - Calls `create_index_primary(db)` to ensure HNSW index exists

**Example Usage:**
```rust
use ploke_test_utils::{validate_backup_fixture_contract, FIXTURE_NODES_LOCAL_EMBEDDINGS};

let db = fresh_backup_fixture_db(&FIXTURE_NODES_LOCAL_EMBEDDINGS)?;
validate_backup_fixture_contract(&FIXTURE_NODES_LOCAL_EMBEDDINGS, &db)?;
```

---

## Database Setup Utilities (for Fixture Regeneration)

### 7. `setup_db_full`

**Full Path:** `ploke_test_utils::setup_db_full`  
**Location:** `lib.rs:118-157`

```rust
#[cfg(feature = "test_setup")]
pub fn setup_db_full(fixture: &'static str) -> Result<cozo::Db<MemStorage>, ploke_error::Error>
```

**Purpose:** Full database setup from a fixture crate (parsing + transform).

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture` | `&'static str` | Name of fixture crate (e.g., "fixture_nodes") |

**Process:**
1. Creates `cozo::Db<MemStorage>` (in-memory database)
2. Initializes database
3. Creates schema via `ploke_transform::schema::create_schema_all()`
4. Parses fixture using `test_run_phases_and_collect()`
5. Merges parsed graphs
6. Builds module tree
7. Transforms graph to database

**Output/Return:**
- `Result<cozo::Db<MemStorage>, ploke_error::Error>`

**Example Usage:**
```rust
use ploke_test_utils::setup_db_full;

let cozo_db = setup_db_full("fixture_nodes")?;
```

---

### 8. `setup_db_full_multi_embedding`

**Full Path:** `ploke_test_utils::setup_db_full_multi_embedding`  
**Location:** `lib.rs:230-282`

```rust
pub fn setup_db_full_multi_embedding(
    fixture: &'static str
) -> Result<cozo::Db<MemStorage>, ploke_error::Error>
```

**Purpose:** Full database setup plus multi-embedding schema creation.

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture` | `&'static str` | Name of fixture crate |

**Process:**
1. Runs full `setup_db_full()` process
2. Creates `embedding_set` relation
3. Inserts default `EmbeddingSet`
4. Creates default vector embedding relation

**Output/Return:**
- `Result<cozo::Db<MemStorage>, ploke_error::Error>`

**Example Usage:**
```rust
use ploke_test_utils::setup_db_full_multi_embedding;

let cozo_db = setup_db_full_multi_embedding("fixture_nodes")?;
```

---

### 9. `setup_db_full_crate`

**Full Path:** `ploke_test_utils::setup_db_full_crate`  
**Location:** `lib.rs:328-359`

```rust
#[cfg(feature = "test_setup")]
pub fn setup_db_full_crate(
    crate_name: &'static str
) -> Result<cozo::Db<MemStorage>, ploke_error::Error>
```

**Purpose:** Setup database from a workspace crate (e.g., "ploke-db").

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `crate_name` | `&'static str` | Name of crate in workspace (e.g., "ploke-db") |

**Process:**
1. Creates in-memory CozoDB
2. Initializes and creates schema
3. Parses crate from `workspace_root().join("crates").join(crate_name)`
4. Merges graphs and builds tree via `parse_and_build_tree()`
5. Transforms to database
6. Creates multi-embedding schema

**Output/Return:**
- `Result<cozo::Db<MemStorage>, ploke_error::Error>`

**Example Usage:**
```rust
use ploke_test_utils::setup_db_full_crate;

let cozo_db = setup_db_full_crate("ploke-db")?;
```

---

### 10. `setup_db_full_workspace_fixture`

**Full Path:** `ploke_test_utils::setup_db_full_workspace_fixture`  
**Location:** `lib.rs:285-322`

```rust
#[cfg(feature = "test_setup")]
pub fn setup_db_full_workspace_fixture(
    fixture: &'static str
) -> Result<cozo::Db<MemStorage>, ploke_error::Error>
```

**Purpose:** Setup database from a workspace fixture (multi-member workspace).

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture` | `&'static str` | Name of workspace fixture |

**Process:**
1. Creates in-memory CozoDB
2. Initializes and creates schema
3. Parses workspace from `tests/fixture_workspace/{fixture}`
4. Transforms workspace (uses `transform_parsed_workspace`)
5. Creates multi-embedding schema

**Output/Return:**
- `Result<cozo::Db<MemStorage>, ploke_error::Error>`

**Example Usage:**
```rust
use ploke_test_utils::setup_db_full_workspace_fixture;

let cozo_db = setup_db_full_workspace_fixture("ws_fixture_01")?;
```

---

### 11. `setup_db_create_multi_embeddings_with_hnsw`

**Full Path:** `ploke_test_utils::setup_db_create_multi_embeddings_with_hnsw`  
**Location:** `lib.rs:219-228`

```rust
pub fn setup_db_create_multi_embeddings_with_hnsw(
    fixture: &'static str
) -> Result<cozo::Db<cozo::MemStorage>, ploke_error::Error>
```

**Purpose:** Setup database with multi-embedding schema AND HNSW index.

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture` | `&'static str` | Name of fixture crate |

**Process:**
1. Calls `setup_db_full_multi_embedding(fixture)`
2. Creates HNSW embedding index via `db.create_embedding_index(&embedding_set)`

**Output/Return:**
- `Result<cozo::Db<MemStorage>, ploke_error::Error>`

---

### 12. `test_run_phases_and_collect`

**Full Path:** `ploke_test_utils::test_run_phases_and_collect`  
**Location:** `lib.rs:41-61`

```rust
pub fn test_run_phases_and_collect(fixture_name: &str) -> Vec<ParsedCodeGraph>
```

**Purpose:** Run discovery and parsing phases, collecting successful results.

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `fixture_name` | `&str` | Name of fixture crate |

**Output/Return:**
- `Vec<ParsedCodeGraph>` - Panics on any parsing error

**Special Considerations:**
- **Panics** on discovery or parsing failure
- For tests that expect all files to parse successfully

---

## Key Types and Structs

### `FixtureDb`

**Path:** `ploke_test_utils::FixtureDb`  
**Location:** `fixture_dbs.rs:82-136`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureDb {
    pub id: &'static str,                    // e.g., "fixture_nodes_canonical"
    pub rel_path: &'static str,              // Path relative to workspace root
    pub parsed_targets: &'static [&'static str], // Source paths used to create fixture
    pub status: FixtureStatus,               // Active, Legacy, Orphaned
    pub creation: FixtureCreationStrategy,   // How to regenerate
    pub default_access: FixtureAccess,       // ImmutableShared or FreshMutable
    pub import_mode: FixtureImportMode,      // PlainBackup or BackupWithEmbeddings
    pub requires_primary_index: bool,        // Whether to create HNSW index
    pub bm25_index_expected: bool,           // Whether BM25 index expected
    pub embedding: Option<FixtureEmbeddingExpectation>, // Embedding metadata
    pub last_updated: &'static str,          // Date string
    pub notes: &'static str,                 // Human-readable description
}
```

**Key Methods:**
- `path()` -> `PathBuf` - Returns absolute path to fixture file
- `filename()` -> `&'static str` - Returns filename portion
- `expected_embedding_set()` -> `Option<EmbeddingSet>` - Returns expected embedding set
- `output_stem()` -> `&'static str` - Returns output filename stem

---

### `FixtureAccess`

**Path:** `ploke_test_utils::FixtureAccess`  
**Location:** `fixture_dbs.rs:19-23`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureAccess {
    ImmutableShared,  // Can be cached/shared across tests
    FreshMutable,     // Needs fresh instance for mutations
}
```

---

### `FixtureImportMode`

**Path:** `ploke_test_utils::FixtureImportMode`  
**Location:** `fixture_dbs.rs:25-29`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureImportMode {
    PlainBackup,           // Standard backup import
    BackupWithEmbeddings,  // Import with embedding vectors
}
```

---

### `FixtureStatus`

**Path:** `ploke_test_utils::FixtureStatus`  
**Location:** `fixture_dbs.rs:31-36`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureStatus {
    Active,   // Currently used in tests
    Legacy,   // Old schema, may still have consumers
    Orphaned, // No known consumers
}
```

---

### `FixtureCreationStrategy`

**Path:** `ploke_test_utils::FixtureCreationStrategy`  
**Location:** `fixture_dbs.rs:38-42`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureCreationStrategy {
    Automated(FixtureAutomation),   // Regenerated via code
    Manual(FixtureManualRecreation), // Manual steps required
}
```

---

### `FixtureAutomation`

**Path:** `ploke_test_utils::FixtureAutomation`  
**Location:** `fixture_dbs.rs:44-62`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureAutomation {
    FixtureCrateMultiEmbedding {
        fixture_name: &'static str,
        output_stem: &'static str,
    },
    FixtureCrateLocalEmbeddings {
        fixture_name: &'static str,
        output_stem: &'static str,
    },
    WorkspaceFixture {
        fixture_name: &'static str,
        output_stem: &'static str,
    },
    WorkspaceCrate {
        crate_name: &'static str,
        output_stem: &'static str,
    },
}
```

---

### `FixtureEmbeddingExpectation`

**Path:** `ploke_test_utils::FixtureEmbeddingExpectation`  
**Location:** `fixture_dbs.rs:71-79`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixtureEmbeddingExpectation {
    pub provider: &'static str,      // e.g., "local"
    pub model: &'static str,         // e.g., "sentence-transformers/all-MiniLM-L6-v2"
    pub dims: u32,                   // e.g., 384
    pub dtype: &'static str,         // e.g., "f32"
    pub vectors_present: bool,       // Whether vectors are in backup
    pub active_set_expected: bool,   // Whether set should be active
}
```

---

## Fixture Constants

| Constant | ID | Path | Status | Import Mode |
|----------|-----|------|--------|-------------|
| `FIXTURE_NODES_CANONICAL` | fixture_nodes_canonical | tests/backup_dbs/fixture_nodes_canonical_2026-03-20.sqlite | Active | PlainBackup |
| `FIXTURE_NODES_LOCAL_EMBEDDINGS` | fixture_nodes_local_embeddings | tests/backup_dbs/fixture_nodes_local_embeddings_2026-03-20.sqlite | Active | BackupWithEmbeddings |
| `FIXTURE_NODES_MULTI_EMBEDDING_SCHEMA_V1` | fixture_nodes_multi_embedding_schema_v1_legacy | tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988... | Legacy | PlainBackup |
| `PLOKE_DB_PRIMARY` | ploke_db_primary | tests/backup_dbs/ploke_db_primary_2026-03-22.sqlite | Active | PlainBackup |
| `WS_FIXTURE_01_CANONICAL` | ws_fixture_01_canonical | tests/backup_dbs/ws_fixture_01_canonical_2026-03-21.sqlite | Active | PlainBackup |
| `PLOKE_DB_ORPHANED` | ploke_db_orphaned | tests/backup_dbs/ploke-db_af8e3a20-728d-5967-8523-da8a5ccdae45 | Orphaned | PlainBackup |

---

## Re-exports in lib.rs

All key items are re-exported from `lib.rs` for convenient access:

```rust
pub use fixture_dbs::{
    backup_db_fixture, fresh_backup_fixture_db, shared_backup_fixture_db,
    validate_backup_fixture_contract, FixtureAutomation, FixtureCreationStrategy, FixtureDb,
    FixtureEmbeddingExpectation, FixtureImportMode, FixtureManualRecreation, FixtureStatus,
    BACKUP_DB_FIXTURES, FIXTURE_NODES_CANONICAL, FIXTURE_NODES_LOCAL_EMBEDDINGS,
    FIXTURE_NODES_MULTI_EMBEDDING_SCHEMA_V1, PLOKE_DB_ORPHANED, PLOKE_DB_PRIMARY,
    WS_FIXTURE_01_CANONICAL,
};
```

---

## Example Usage from xtask

From `xtask/src/main.rs`:

```rust
use ploke_test_utils::{
    FIXTURE_NODES_LOCAL_EMBEDDINGS, FixtureAutomation, FixtureCreationStrategy, FixtureDb,
    FixtureImportMode, FixtureManualRecreation, backup_db_fixture, fresh_backup_fixture_db,
    setup_db_full_crate, setup_db_full_multi_embedding, setup_db_full_workspace_fixture,
    validate_backup_fixture_contract,
};

// Loading a fixture by ID
let fixture = backup_db_fixture("fixture_nodes_canonical")
    .ok_or_else(|| format!("Unknown fixture: {}", fixture_id))?;
let db = fresh_backup_fixture_db(fixture)?;

// Regenerating a fixture
let cozo_db = setup_db_full_multi_embedding("fixture_nodes")?;
// ... save to file ...

// Validating after load
validate_backup_fixture_contract(fixture, &db)?;
```

---

## Summary for `db load-fixture` Command

| Function | Use Case |
|----------|----------|
| `backup_db_fixture(id)` | Look up fixture by user-provided ID |
| `fresh_backup_fixture_db(fixture)` | Load fixture into fresh database |
| `shared_backup_fixture_db(fixture)` | Load with caching (for tests) |
| `all_backup_db_fixtures()` | List all available fixtures |
| `active_backup_db_fixtures()` | List only active fixtures |
| `validate_backup_fixture_contract()` | Verify loaded fixture integrity |

---

## Issues Encountered

None - crate is well-documented and follows clear patterns.

---

## Dependencies

Key dependencies for context:
- `ploke_db::Database` - Database wrapper type
- `cozo::Db<MemStorage>` - Underlying CozoDB in-memory storage
- `ploke_error::Error` - Unified error type
- `ploke_transform` - For schema creation and graph transformation
- `syn_parser` - For parsing fixture crates
