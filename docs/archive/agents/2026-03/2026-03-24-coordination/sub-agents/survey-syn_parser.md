# Survey: syn_parser Crate

**Date:** 2026-03-25  
**Task:** Milestone M.1 - Survey syn_parser for xtask commands (A.1 Parsing Commands)  
**Agent:** Sub-agent for syn_parser survey  
**Branch:** feature/xtask-commands

---

## Files Touched During Survey

| File | Lines | Description |
|------|-------|-------------|
| `crates/ingest/syn_parser/src/lib.rs` | 804 | Main entry points, high-level API |
| `crates/ingest/syn_parser/src/discovery/mod.rs` | 567 | Discovery phase implementation |
| `crates/ingest/syn_parser/src/discovery/error.rs` | 469 | Discovery-specific error types |
| `crates/ingest/syn_parser/src/discovery/single_crate.rs` | 966 | Single crate context types |
| `crates/ingest/syn_parser/src/discovery/workspace.rs` | 538 | Workspace manifest parsing |
| `crates/ingest/syn_parser/src/error.rs` | 478 | Main error types (SynParserError) |
| `crates/ingest/syn_parser/src/parser/mod.rs` | 14 | Parser module re-exports |
| `crates/ingest/syn_parser/src/parser/graph/mod.rs` | 1000+ | GraphAccess trait and node lookups |
| `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs` | 916 | ParsedCodeGraph implementation |
| `crates/ingest/syn_parser/src/resolve/mod.rs` | 223 | Resolution types and re-exports |
| `crates/ingest/syn_parser/src/resolve/module_tree.rs` | 1000+ | ModuleTree definition |

---

## Function Documentation

### 1. `run_discovery_phase`

**Full Path:** `syn_parser::discovery::run_discovery_phase`

**Signature:**
```rust
#[instrument(err)]
pub fn run_discovery_phase(
    workspace_root: Option<&Path>,
    target_crates: &[PathBuf],
) -> Result<DiscoveryOutput, DiscoveryError>
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace_root` | `Option<&Path>` | Optional workspace root path for workspace-aware discovery |
| `target_crates` | `&[PathBuf]` | Slice of absolute paths to crate root directories |

**Output/Return Type:**
- `Result<DiscoveryOutput, DiscoveryError>`
- `DiscoveryOutput` contains:
  - `crate_contexts: HashMap<PathBuf, CrateContext>` - Context for each crate
  - `workspace: Option<WorkspaceManifestMetadata>` - Workspace metadata if present
  - `warnings: Vec<DiscoveryError>` - Non-fatal errors collected during discovery

**Error Type:** `DiscoveryError` (defined in `discovery/error.rs`)
- `Io { path, source }` - I/O error accessing path
- `TomlParse { path, source }` - Failed to parse Cargo.toml
- `MissingPackageName { path }` - Missing package.name field
- `MissingPackageVersion { path }` - Missing package.version field
- `CratePathNotFound { path }` - Target crate path not found
- `Walkdir { path, source }` - Directory walking error
- `SrcNotFound { path }` - Source directory not found
- And several workspace-related variants

**Key Types/Structs Needed:**
- `DiscoveryOutput` - Output of discovery phase
- `CrateContext` - Context for a single crate (name, version, namespace, files, etc.)
- `DiscoveryError` - Error type for discovery failures
- `PackageInfo`, `PackageVersion` - Cargo.toml package section
- `Dependencies`, `DevDependencies`, `Features` - Cargo.toml sections
- `WorkspaceManifestMetadata`, `WorkspaceMetadataSection` - Workspace info

**Special Considerations:**
- **Synchronous** - Single-threaded discovery phase
- Expects absolute paths for `target_crates`
- Performs file system walking with walkdir
- Parses Cargo.toml files using toml crate
- Generates deterministic UUID v5 namespaces for crates
- Filters out main.rs when lib.rs is present (known limitation)
- Has `#[instrument(err)]` tracing annotation for error tracking

**Example Usage:**
```rust
use syn_parser::discovery::run_discovery_phase;
use std::path::PathBuf;

let crate_path = PathBuf::from("/path/to/crate");
let discovery = run_discovery_phase(None, &[crate_path])?;

for (path, context) in discovery.iter_crate_contexts() {
    println!("Crate: {} at {:?}", context.name, path);
    println!("Files: {:?}", context.files);
}
```

---

### 2. `try_run_phases_and_resolve`

**Full Path:** `syn_parser::try_run_phases_and_resolve`

**Signature:**
```rust
#[instrument()]
pub fn try_run_phases_and_resolve(
    target_crate_dir: &Path,
) -> Result<Vec<ParsedCodeGraph>, SynParserError>
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `target_crate_dir` | `&Path` | Path to the crate root directory (must contain Cargo.toml) |

**Output/Return Type:**
- `Result<Vec<ParsedCodeGraph>, SynParserError>`
- Returns a vector of `ParsedCodeGraph` - one per parsed file
- Each `ParsedCodeGraph` contains:
  - `file_path: PathBuf` - Absolute path of parsed file
  - `crate_namespace: Uuid` - UUID namespace for the crate
  - `graph: CodeGraph` - The actual code graph with nodes and relations
  - `crate_context: Option<CrateContext>` - Crate context (critical for merging)

**Error Type:** `SynParserError` (defined in `error.rs`)
- `MultipleErrors(Vec<SynParserError>)` - Multiple errors occurred
- `PartialParsing { successes, errors }` - Some files succeeded, some failed
- `ParsedGraphError(ParsedGraphError)` - Error in parsed graph
- `ComplexDiscovery { name, path, source_string }` - Discovery phase error
- And many more variants for different failure modes

**Key Types/Structs Needed:**
- `ParsedCodeGraph` - The main output type containing parsed code
- `CodeGraph` - Contains all nodes (functions, types, modules, etc.) and relations
- `CrateContext` - Context information for the crate
- `SynParserError` - Main error type
- `DiscoveryOutput` - Intermediate type from discovery phase

**Special Considerations:**
- **Synchronous** but internally uses Rayon for parallel parsing
- Runs the full pipeline: discovery → parallel parsing → returns individual graphs (not merged)
- Validates that at least one graph has crate_context (required for merging)
- Returns partial success as `SynParserError::PartialParsing` if some files fail
- Uses `analyze_files_parallel` internally (Rayon-based)
- Has `#[instrument()]` tracing annotation
- Target must be a single crate (not a workspace root)

**Example Usage:**
```rust
use syn_parser::try_run_phases_and_resolve;
use std::path::Path;

let crate_path = Path::new("/path/to/crate");
match try_run_phases_and_resolve(crate_path) {
    Ok(graphs) => {
        for graph in graphs {
            println!("Parsed: {:?}", graph.file_path);
            println!("Functions: {}", graph.functions().len());
        }
    }
    Err(SynParserError::PartialParsing { successes, errors }) => {
        println!("Partial success: {} files parsed, {} errors", 
                 successes.0.len(), errors.len());
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

---

### 3. `try_run_phases_and_merge`

**Full Path:** `syn_parser::try_run_phases_and_merge`

**Signature:**
```rust
pub fn try_run_phases_and_merge(target_crate: &Path) -> Result<ParserOutput, SynParserError>
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `target_crate` | `&Path` | Path to the crate root directory |

**Output/Return Type:**
- `Result<ParserOutput, SynParserError>`
- `ParserOutput` contains:
  - `merged_graph: Option<ParsedCodeGraph>` - The merged code graph
  - `module_tree: Option<ModuleTree>` - The constructed module tree

**Error Type:** `SynParserError` (same as above)
- Additional error possibilities from merging and tree building
- `InternalState(String)` - If module tree building fails

**Key Types/Structs Needed:**
- `ParserOutput` - Wrapper for merged results
- `ParsedCodeGraph` - The merged graph
- `ModuleTree` - The module tree structure
- `SynParserError` - Error type
- `GraphAccess` trait - For accessing graph data

**Special Considerations:**
- **Synchronous**
- Runs full pipeline: discovery → parse → merge → build tree
- Calls `try_run_phases_and_resolve` internally, then merges graphs
- Calls `build_tree_and_prune` which constructs `ModuleTree` and prunes unlinked modules
- Returns `ParserOutput` which has `extract_merged_graph()` and `extract_module_tree()` methods
- Uses tracing spans internally for `build_tree_and_prune` with relation/module counts

**Example Usage:**
```rust
use syn_parser::try_run_phases_and_merge;
use std::path::Path;

let crate_path = Path::new("/path/to/crate");
let mut output = try_run_phases_and_merge(crate_path)?;

if let Some(graph) = output.extract_merged_graph() {
    println!("Merged graph has {} functions", graph.functions().len());
    println!("Modules: {}", graph.modules().len());
}

if let Some(tree) = output.extract_module_tree() {
    println!("Module tree root: {:?}", tree.root());
}
```

---

### 4. `parse_workspace`

**Full Path:** `syn_parser::parse_workspace`

**Signature:**
```rust
#[instrument(skip_all, fields(workspace = %target_workspace_dir.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("No filename")
        .to_string()
))]
pub fn parse_workspace(
    target_workspace_dir: &Path,
    selected_crates: Option<&[&Path]>,
) -> Result<ParsedWorkspace, SynParserError>
```

**Input Parameters:**
| Parameter | Type | Description |
|-----------|------|-------------|
| `target_workspace_dir` | `&Path` | Path to the workspace root directory |
| `selected_crates` | `Option<&[&Path]>` | Optional slice of specific crate paths to parse (relative or absolute) |

**Output/Return Type:**
- `Result<ParsedWorkspace, SynParserError>`
- `ParsedWorkspace` contains:
  - `workspace: WorkspaceMetadataSection` - Workspace metadata
  - `crates: Vec<ParsedCrate>` - Parsed results for each crate
- `ParsedCrate` contains:
  - `crate_context: CrateContext` - Context for the crate
  - `parser_output: ParserOutput` - The parse results (graph + tree)

**Error Type:** `SynParserError`
- `ComplexDiscovery { name, path, source_string }` - Manifest parsing error
- `WorkspaceSectionMissing { workspace_path }` - No [workspace] section found
- `WorkspaceSelectionMismatch { ... }` - Selected crates not in workspace
- `MultipleErrors(Vec<SynParserError>)` - Aggregate of member parse failures

**Key Types/Structs Needed:**
- `ParsedWorkspace` - Output for workspace parsing
- `ParsedCrate` - Output for single crate within workspace
- `WorkspaceMetadataSection` - Workspace metadata
- `ParserOutput` - Individual crate parse results
- `CrateContext` - Crate context
- `SynParserError` - Error type

**Special Considerations:**
- **Synchronous**
- Parses workspace Cargo.toml to get member list
- Validates selected crates against workspace members
- Normalizes relative paths to absolute paths
- Calls `try_run_phases_and_merge` for each workspace member
- Aggregates errors across all crates with `MultipleErrors`
- Has `#[instrument(skip_all, fields(...))]` tracing annotation with workspace name
- If `selected_crates` is `None`, parses all workspace members
- If `selected_crates` is `Some(&[])` (empty), returns success with no crates

**Example Usage:**
```rust
use syn_parser::parse_workspace;
use std::path::Path;

let workspace_path = Path::new("/path/to/workspace");

// Parse all members
let parsed = parse_workspace(workspace_path, None)?;

// Parse specific members
let selected = &[Path::new("crate_a"), Path::new("crate_b")];
let parsed = parse_workspace(workspace_path, Some(selected))?;

println!("Workspace: {:?}", parsed.workspace.path);
for parsed_crate in &parsed.crates {
    println!("Crate: {} at {:?}", 
             parsed_crate.crate_context.name,
             parsed_crate.crate_context.root_path);
}
```

---

## Types and Structs Summary

### Primary Output Types

| Type | Module | Description |
|------|--------|-------------|
| `ParserOutput` | `lib.rs` | Main output with merged graph and module tree |
| `ParsedCodeGraph` | `parser::graph` | Single file's parsed code graph |
| `ParsedWorkspace` | `lib.rs` | Workspace parsing output |
| `ParsedCrate` | `lib.rs` | Single crate within workspace output |
| `DiscoveryOutput` | `discovery` | Discovery phase output |

### Graph Types

| Type | Module | Description |
|------|--------|-------------|
| `CodeGraph` | `parser::graph` | Contains all nodes and relations |
| `ModuleTree` | `resolve::module_tree` | Module hierarchy and resolution |
| `GraphAccess` | `parser::graph` | Trait for accessing graph data |

### Error Types

| Type | Module | Description |
|------|--------|-------------|
| `SynParserError` | `error` | Main error enum |
| `DiscoveryError` | `discovery::error` | Discovery phase errors |
| `ParsedGraphError` | `parser::graph` | Graph-specific errors |
| `ModuleTreeError` | `resolve` | Module tree errors |

### Context Types

| Type | Module | Description |
|------|--------|-------------|
| `CrateContext` | `discovery` | Crate metadata (name, version, files, deps) |
| `WorkspaceMetadataSection` | `discovery::workspace` | Workspace [workspace] table |

---

## Tracing Instrumentation Status

| Function | Tracing Status | Notes |
|----------|---------------|-------|
| `run_discovery_phase` | ✅ `#[instrument(err)]` | Errors are traced |
| `try_run_phases_and_resolve` | ✅ `#[instrument()]` | Entry traced |
| `try_run_phases_and_merge` | ⚠️ No direct annotation | Calls traced functions internally |
| `parse_workspace` | ✅ `#[instrument(skip_all, fields(...))]` | Workspace name in fields |
| `build_tree_and_prune` | ✅ Internal spans | Uses `info_span!` for tree building and pruning |

---

## Issues Encountered

1. **No async functions** - All parsing functions are synchronous. This is by design for the parsing pipeline.

2. **Complex error hierarchy** - Multiple error types (`DiscoveryError`, `SynParserError`, `ParsedGraphError`, `ModuleTreeError`) with conversions between them.

3. **Path handling** - Functions expect absolute paths but may work with relative paths in some cases. `parse_workspace` normalizes paths.

4. **Partial success handling** - `try_run_phases_and_resolve` returns `PartialParsing` error variant that contains successes, which is an unusual pattern.

5. **CrateContext requirement** - Merging graphs requires at least one graph to have `crate_context` set.

---

## Notes for Architecture Design (M.2)

1. **Command Structure Recommendations:**
   - `parse discovery <path>` → `run_discovery_phase`
   - `parse phases-resolve <path>` → `try_run_phases_and_resolve`
   - `parse phases-merge <path>` → `try_run_phases_and_merge`
   - `parse workspace <path> [--crate <name>...]` → `parse_workspace`

2. **Error Handling:**
   - All functions return `Result<T, SynParserError>` (or `DiscoveryError` for discovery)
   - Need to map to xtask-friendly error messages
   - `PartialParsing` needs special handling to extract partial results

3. **Output Formats:**
   - Consider JSON output for programmatic consumption
   - `ParserOutput` can be serialized (has `Deserialize` on `ParsedCodeGraph`)
   - `ModuleTree` has serde support for some types

4. **Performance Considerations:**
   - Parallel parsing uses Rayon internally
   - No async runtime needed
   - Memory-intensive for large workspaces (holds all graphs)

5. **Tracing Integration:**
   - Functions already have `#[instrument]` attributes
   - Tracing output can be enabled for debugging
   - Consider exposing tracing subscriber configuration in xtask
