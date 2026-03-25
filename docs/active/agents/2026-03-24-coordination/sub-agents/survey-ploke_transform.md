# Survey Report: ploke_transform Crate

**Date:** 2026-03-25  
**Task:** Milestone M.1 - Survey ploke_transform for xtask transform commands  
**Agent:** Sub-agent for ploke_transform survey  
**Branch:** feature/xtask-commands

---

## Summary

Surveyed the `ploke_transform` crate to identify functions for the transform commands described in README section A.2. Found 2 primary public functions:
- `transform_parsed_graph` - transforms a single parsed crate graph to CozoDB
- `transform_parsed_workspace` - transforms a full workspace with multiple crates to CozoDB

Both functions are already used in production code (ploke-tui, xtask, test-utils).

---

## Files Touched During Survey

| File | Purpose |
|------|---------|
| `crates/ingest/ploke-transform/src/lib.rs` | Crate root, module exports |
| `crates/ingest/ploke-transform/src/transform/mod.rs` | Main transform functions |
| `crates/ingest/ploke-transform/src/transform/workspace.rs` | Workspace transform implementation |
| `crates/ingest/ploke-transform/src/transform/crate_context.rs` | Crate context transformation |
| `crates/ingest/ploke-transform/src/error.rs` | Error types |
| `crates/ingest/ploke-transform/src/schema/mod.rs` | Schema creation utilities |
| `crates/ingest/ploke-transform/src/tests.rs` | Test usage examples |
| `crates/ingest/ploke-transform/Cargo.toml` | Dependencies |

---

## Function Documentation

### 1. `transform_parsed_graph`

**Full Path:** `ploke_transform::transform::transform_parsed_graph`

**Signature:**
```rust
#[instrument(skip_all)]
pub fn transform_parsed_graph(
    db: &Db<MemStorage>,
    parsed_graph: ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<(), TransformError>
```

**Location:** `crates/ingest/ploke-transform/src/transform/mod.rs:122`

**Description:**  
Transforms a single parsed crate graph into CozoDB relations. This is the main entry point for transforming individual crate parsing results into the database.

**Input Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `db` | `&Db<MemStorage>` | Reference to initialized CozoDB instance with MemStorage |
| `parsed_graph` | `ParsedCodeGraph` | The parsed code graph containing all AST nodes from a single crate |
| `tree` | `&ModuleTree` | Reference to the module tree for path resolution |

**Output/Return Type:**
- `Result<(), TransformError>` - Returns `Ok(())` on success, or `TransformError` on failure

**Error Type:**
- `TransformError` (defined in `crates/ingest/ploke-transform/src/error.rs`)
  - `TransformError::Internal(InternalError)` - Internal errors
  - `TransformError::Database(String)` - Database operation failures
  - `TransformError::SchemaValidation(String)` - Schema validation failures
  - `TransformError::Transformation(String)` - Data transformation failures
  - Also converts from `cozo::Error` automatically

**Key Types/Structs Needed:**

| Type | Crate | Path | Description |
|------|-------|------|-------------|
| `Db<MemStorage>` | `cozo` | `cozo::Db` | Cozo database instance |
| `ParsedCodeGraph` | `syn_parser` | `syn_parser::parser::graph::ParsedCodeGraph` | Parsed crate graph |
| `ModuleTree` | `syn_parser` | `syn_parser::resolve::module_tree::ModuleTree` | Module resolution tree |
| `TransformError` | `ploke_transform` | `ploke_transform::error::TransformError` | Error type |

**Special Considerations:**
- **Tracing:** Has `#[instrument(skip_all)]` attribute for tracing support
- **Database Setup:** Database must be initialized and schema created via `create_schema_all(&db)` before calling
- **Panics:** Will panic if `parsed_graph.crate_context` is `None` (expects all graphs to have crate context)
- **Sequential:** Transforms components in sequence: types → functions → defined types → traits → impls → modules → consts → statics → macros → imports → relations → crate_context

**Prerequisites:**
1. Database must be created: `Db::new(MemStorage::default())`
2. Database must be initialized: `db.initialize()`
3. Schema must be created: `create_schema_all(&db)`

**Example Usage:**

From `crates/test-utils/src/lib.rs`:
```rust
use cozo::{Db, MemStorage};
use ploke_transform::transform::transform_parsed_graph;
use ploke_transform::schema::create_schema_all;
use syn_parser::parser::ParsedCodeGraph;

// Initialize database
let db = Db::new(MemStorage::default()).expect("Failed to create database");
db.initialize().expect("Failed to initialize database");

// Create schema
create_schema_all(&db)?;

// Parse and get merged graph + tree (from syn_parser)
let (merged, tree) = parse_and_build_tree("crate_name")?;

// Transform into database
transform_parsed_graph(&db, merged, &tree)?;
```

From `crates/ploke-tui/src/parser.rs`:
```rust
use ploke_transform::transform::transform_parsed_graph;

let mut parser_output = try_run_phases_and_merge(&resolved.focused_root)?;
let merged = parser_output.extract_merged_graph().ok_or_else(|| {
    SynParserError::InternalState("Missing parsed code graph".to_string())
})?;
let tree = parser_output
    .extract_module_tree()
    .ok_or_else(|| SynParserError::InternalState("Missing module tree".to_string()))?;
    
transform_parsed_graph(&db, merged, &tree).map_err(|err| {
    SynParserError::InternalState(format!("Failed to transform parsed graph: {err}"))
})?;
```

---

### 2. `transform_parsed_workspace`

**Full Path:** `ploke_transform::transform::transform_parsed_workspace`

**Signature:**
```rust
#[instrument(skip_all, fields(crate_count = parsed_workspace.crates.len()))]
pub fn transform_parsed_workspace(
    db: &Db<MemStorage>,
    parsed_workspace: ParsedWorkspace,
) -> Result<(), TransformError>
```

**Location:** `crates/ingest/ploke-transform/src/transform/workspace.rs:16`

**Description:**  
Transforms workspace metadata and all parsed crate graphs within a workspace into CozoDB. First inserts workspace metadata, then iterates through each crate and calls `transform_parsed_graph` for each.

**Input Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `db` | `&Db<MemStorage>` | Reference to initialized CozoDB instance with MemStorage |
| `parsed_workspace` | `ParsedWorkspace` | The parsed workspace containing metadata and all crate graphs |

**Output/Return Type:**
- `Result<(), TransformError>` - Returns `Ok(())` on success, or `TransformError` on failure

**Error Type:**
- `TransformError` (same as above)
- Specific error messages for missing merged graph or module tree:
  - `"ParsedWorkspace crate was missing its merged graph"`
  - `"ParsedWorkspace crate was missing its module tree"`

**Key Types/Structs Needed:**

| Type | Crate | Path | Description |
|------|-------|------|-------------|
| `Db<MemStorage>` | `cozo` | `cozo::Db` | Cozo database instance |
| `ParsedWorkspace` | `syn_parser` | `syn_parser::ParsedWorkspace` | Parsed workspace with multiple crates |
| `ParsedCrate` | `syn_parser` | `syn_parser::ParsedCrate` | Individual parsed crate within workspace |
| `ParserOutput` | `syn_parser` | `syn_parser::parser::pipeline::ParserOutput` | Output from parser pipeline |
| `WorkspaceMetadataSection` | `syn_parser` | `syn_parser::discovery::workspace::WorkspaceMetadataSection` | Workspace metadata |
| `TransformError` | `ploke_transform` | `ploke_transform::error::TransformError` | Error type |

**Special Considerations:**
- **Tracing:** Has `#[instrument(skip_all, fields(crate_count = ...))]` for tracing with crate count field
- **Database Setup:** Same prerequisites as `transform_parsed_graph`
- **Internal Call:** Calls `transform_parsed_graph` internally for each crate
- **Workspace Metadata:** Also inserts workspace metadata via `transform_workspace_metadata`
- **Crate Count:** Tracing includes the number of crates being transformed

**Prerequisites:**
1. Same database setup as `transform_parsed_graph`
2. Workspace must be parsed using `syn_parser::parse_workspace()`

**Example Usage:**

From `crates/ploke-tui/src/parser.rs`:
```rust
use ploke_transform::transform::transform_parsed_workspace;
use syn_parser::parse_workspace;

let parsed_workspace = parse_workspace(&resolved.workspace_root, None)?;
transform_parsed_workspace(&db, parsed_workspace).map_err(|err| {
    SynParserError::InternalState(format!("Failed to transform parsed workspace: {err}"))
})?;
```

From `crates/test-utils/src/lib.rs`:
```rust
use syn_parser::parse_workspace;
use ploke_transform::transform::transform_parsed_workspace;

let parsed_workspace = parse_workspace(&fixture_workspace_root, None)?;
// ... database setup ...
transform_parsed_workspace(&db, parsed_workspace)?;
```

From test in `workspace.rs`:
```rust
#[test]
fn transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_01");
    let parsed_workspace = parse_workspace(&fixture_workspace_root, None)?;

    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    transform_parsed_workspace(&db, parsed_workspace)?;
    
    // Query to verify workspace metadata was persisted...
    Ok(())
}
```

---

## Supporting Functions

### `create_schema_all`

**Full Path:** `ploke_transform::schema::create_schema_all`

**Signature:**
```rust
pub fn create_schema_all(db: &Db<MemStorage>) -> Result<(), crate::error::TransformError>
```

**Location:** `crates/ingest/ploke-transform/src/schema/mod.rs:86`

**Description:** Creates all database schemas (relations) required for the transform. Must be called once before any transform operations.

**Example Usage:**
```rust
use ploke_transform::schema::create_schema_all;

create_schema_all(&db)?;
```

---

## Input Type Details

### `ParsedCodeGraph` (from syn_parser)

```rust
pub struct ParsedCodeGraph {
    pub file_path: PathBuf,
    pub crate_namespace: Uuid,
    pub graph: CodeGraph,
    pub crate_context: Option<CrateContext>,
}
```

Key method:
- `build_tree_and_prune() -> Result<ModuleTree, ModuleTreeError>` - Builds module tree from the graph

### `ParsedWorkspace` (from syn_parser)

```rust
pub struct ParsedWorkspace {
    pub workspace: WorkspaceMetadataSection,
    pub crates: Vec<ParsedCrate>,
}

pub struct ParsedCrate {
    pub crate_context: CrateContext,
    pub parser_output: ParserOutput,
}
```

`ParserOutput` provides:
- `extract_merged_graph() -> Option<ParsedCodeGraph>`
- `extract_module_tree() -> Option<ModuleTree>`

---

## Tracing Instrumentation Status

| Function | Tracing | Notes |
|----------|---------|-------|
| `transform_parsed_graph` | ✅ `#[instrument(skip_all)]` | Logs trace for each transform step |
| `transform_parsed_workspace` | ✅ `#[instrument(skip_all, fields(crate_count))]` | Includes crate count in span |
| `transform_crate_context` | ✅ `#[instrument(skip_all)]` | Internal helper |
| `transform_workspace_metadata` | ❌ Not instrumented | Internal helper |

The transform functions use `tracing::trace!` with `LogStyle` helpers for detailed step logging:
- `"types".log_step()`
- `"functions".log_step()`
- etc.

---

## Dependencies

From `Cargo.toml`:
```toml
[dependencies]
cozo = { workspace = true }
syn_parser = { path = "../syn_parser" }
serde = { workspace = true }
serde_json = { workspace = true }
itertools = { workspace = true }
ploke-core = { path = "../../ploke-core" }
ploke-common = { path = "../../common" }
uuid = { workspace = true }
tracing = { workspace = true }
ploke-error = { path = "../../ploke-error" }
thiserror = { workspace = true }
```

---

## Issues Encountered

None. The crate is well-structured and the functions are clearly documented and instrumented.

---

## Notes for Architecture Design (M.2)

1. **Command Mapping:**
   - `transform graph` → `transform_parsed_graph`
   - `transform workspace` → `transform_parsed_workspace`

2. **Error Handling:** Both functions return `TransformError`, which can be converted to `ploke_error::Error` for broader error handling.

3. **Database State:** Commands must ensure database is initialized and schema created before transform.

4. **Input Source:** Commands need to either:
   - Accept a path to parse on-the-fly, OR
   - Accept pre-parsed data from a previous command in a pipeline

5. **Tracing:** Both functions already have good tracing coverage via `#[instrument]` attributes.

6. **Synchronous:** Both functions are **synchronous** (not async), which simplifies command implementation.

---

## References

- Main source: `crates/ingest/ploke-transform/src/transform/mod.rs`
- Workspace source: `crates/ingest/ploke-transform/src/transform/workspace.rs`
- Error definitions: `crates/ingest/ploke-transform/src/error.rs`
- Schema definitions: `crates/ingest/ploke-transform/src/schema/mod.rs`
- Production usage: `crates/ploke-tui/src/parser.rs`
- Test utilities: `crates/test-utils/src/lib.rs`
- Benchmarks: `crates/ingest/ploke-transform/benches/transform_pipeline.rs`
