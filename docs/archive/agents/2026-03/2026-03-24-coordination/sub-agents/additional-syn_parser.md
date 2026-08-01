# Additional syn_parser Functions for Agent Diagnostics

**Date:** 2026-03-25  
**Task:** Milestone M.1.2 - Identify additional key functions for xtask commands  
**Agent:** Sub-agent for syn_parser deep-dive  
**Branch:** feature/xtask-commands

---

## Summary

This document identifies additional functions in the `syn_parser` crate beyond the main parsing pipeline functions (documented in `survey-syn_parser.md`). These functions provide diagnostic, inspection, and metadata capabilities that would be valuable for agent troubleshooting and analysis.

---

## 1. Graph Validation and Debug Functions

### 1.1 `validate_unique_rels`

**Location:** `syn_parser::parser::graph::GraphAccess::validate_unique_rels`

**Signature:**
```rust
fn validate_unique_rels(&self) -> bool
```

**Diagnostic Value:**
- Validates that all relations in the code graph are unique
- Detects duplicate relations that could indicate parsing errors
- Used internally during graph merging for validation

**Suggested Command:** `parse validate-relations <path>`

**Input:** `&self` (anything implementing `GraphAccess`)
**Output:** `bool` - true if all relations are unique

---

### 1.2 `debug_relationships`

**Location:** `syn_parser::parser::graph::GraphAccess::debug_relationships`

**Signature:**
```rust
fn debug_relationships(&self)
```

**Diagnostic Value:**
- Prints detailed debug information about all relations
- Shows unique vs total relation counts
- Lists duplicate relations with source/target IDs
- Essential for debugging relation issues

**Suggested Command:** `parse debug-relations <path>`

**Input:** `&self` (anything implementing `GraphAccess`)
**Output:** Logs to debug output

---

### 1.3 `debug_print_all_visible`

**Location:** `syn_parser::parser::graph::GraphAccess::debug_print_all_visible`

**Signature:**
```rust
fn debug_print_all_visible(&self)
```

**Diagnostic Value:**
- Prints all visible items in the graph sorted by ID
- Shows functions, types, traits, modules, consts, statics, macros
- Useful for understanding what's been parsed

**Suggested Command:** `parse list-items <path>`

**Input:** `&self` (anything implementing `GraphAccess`)
**Output:** Prints to stdout

---

## 2. Node Lookup and Query Functions

### 2.1 `find_node_unique`

**Location:** `syn_parser::parser::graph::GraphAccess::find_node_unique`

**Signature:**
```rust
fn find_node_unique(&self, item_id: AnyNodeId) -> Result<&dyn GraphNode, SynParserError>
```

**Diagnostic Value:**
- Finds a node by its ID across all collections
- Returns error if not found or if duplicates exist
- Primary node lookup method with validation

**Suggested Command:** `parse find-node <path> <node_id>`

**Input:** 
- `item_id: AnyNodeId` - The node ID to find
**Output:** `Result<&dyn GraphNode, SynParserError>`

---

### 2.2 `find_module_by_path_checked`

**Location:** `syn_parser::parser::graph::GraphAccess::find_module_by_path_checked`

**Signature:**
```rust
fn find_module_by_path_checked(&self, path: &[String]) -> Result<&ModuleNode, SynParserError>
```

**Diagnostic Value:**
- Finds a module by its full definition path (e.g., `["crate", "module", "submodule"]`)
- Excludes declaration nodes, finds only definitions
- Returns error if not found or duplicates exist

**Suggested Command:** `parse find-module <path> <module_path>`

**Input:**
- `path: &[String]` - Module path segments
**Output:** `Result<&ModuleNode, SynParserError>`

---

### 2.3 `find_any_node_checked`

**Location:** `syn_parser::parser::graph::GraphAccess::find_any_node_checked`

**Signature:**
```rust
fn find_any_node_checked(&self, item_id: AnyNodeId) -> Result<&dyn GraphNode, SynParserError>
```

**Diagnostic Value:**
- Similar to `find_node_unique` but returns `NotFound` error instead of `Option`
- Useful for diagnostic error messages

**Suggested Command:** (internal use, exposed via `find-node`)

---

## 3. Dependency and Metadata Functions

### 3.1 `dependency_names`

**Location:** `syn_parser::parser::graph::ParsedCodeGraph::dependency_names`

**Signature:**
```rust
pub fn dependency_names(&self) -> HashSet<String>
```

**Diagnostic Value:**
- Returns all dependency names from Cargo.toml
- Helps agents understand crate dependencies
- Returns empty set if no crate context available

**Suggested Command:** `parse dependencies <path>`

**Input:** `&self` (ParsedCodeGraph with crate_context)
**Output:** `HashSet<String>`

---

### 3.2 `iter_dependency_names`

**Location:** `syn_parser::parser::graph::ParsedCodeGraph::iter_dependency_names`

**Signature:**
```rust
pub fn iter_dependency_names(&self) -> impl Iterator<Item = &str> + '_
```

**Diagnostic Value:**
- Zero-copy iterator over dependency names
- More efficient than `dependency_names` for large dep lists
- Non-allocating alternative

**Suggested Command:** (used internally by `dependencies`)

---

### 3.3 `CrateContext` Accessors

**Location:** `syn_parser::discovery::CrateContext`

**Key Fields:**
- `name: String` - Crate name
- `version: String` - Crate version
- `namespace: Uuid` - Generated crate namespace
- `root_path: PathBuf` - Crate root directory
- `files: Vec<PathBuf>` - All discovered source files
- `features: Features` - Cargo.toml features
- `dependencies: Dependencies` - Cargo.toml dependencies
- `dev_dependencies: DevDependencies` - Dev dependencies

**Diagnostic Value:**
- Complete crate metadata from discovery phase
- Essential for understanding crate structure

**Suggested Commands:**
- `parse crate-info <path>` - Show crate metadata
- `parse list-files <path>` - List all source files
- `parse features <path>` - List features

---

## 4. Discovery Output Functions

### 4.1 `DiscoveryOutput::get_crate_context`

**Location:** `syn_parser::discovery::DiscoveryOutput::get_crate_context`

**Signature:**
```rust
pub fn get_crate_context(&self, crate_root_path: &Path) -> Option<&CrateContext>
```

**Diagnostic Value:**
- Retrieve context for a specific crate by path
- Used after discovery to inspect crate details

**Suggested Command:** (internal use)

---

### 4.2 `DiscoveryOutput::iter_crate_contexts`

**Location:** `syn_parser::discovery::DiscoveryOutput::iter_crate_contexts`

**Signature:**
```rust
pub fn iter_crate_contexts(&self) -> impl Iterator<Item = (&PathBuf, &CrateContext)> + '_
```

**Diagnostic Value:**
- Iterate over all discovered crates
- Useful for workspace-wide analysis

**Suggested Command:** `parse discovery-list <path>`

---

### 4.3 `DiscoveryOutput::has_warnings`

**Location:** `syn_parser::discovery::DiscoveryOutput::has_warnings`

**Signature:**
```rust
pub fn has_warnings(&self) -> bool
```

**Diagnostic Value:**
- Quick check if discovery encountered non-fatal issues
- Can be used to prompt agent to check warnings

**Suggested Command:** `parse check-warnings <path>`

---

### 4.4 `DiscoveryOutput::warnings`

**Location:** `syn_parser::discovery::DiscoveryOutput::warnings`

**Signature:**
```rust
pub fn warnings(&self) -> &[DiscoveryError]
```

**Diagnostic Value:**
- Get all non-fatal warnings from discovery
- Helps diagnose parsing issues

**Suggested Command:** `parse warnings <path>`

---

## 5. Module Tree Inspection Functions

### 5.1 `ModuleTree::root`

**Location:** `syn_parser::resolve::module_tree::ModuleTree::root`

**Signature:**
```rust
pub fn root(&self) -> ModuleNodeId
```

**Diagnostic Value:**
- Get the root module ID
- Starting point for tree traversal

**Suggested Command:** `parse tree-root <path>`

---

### 5.2 `ModuleTree::modules`

**Location:** `syn_parser::resolve::module_tree::ModuleTree::modules`

**Signature:**
```rust
pub fn modules(&self) -> &HashMap<ModuleNodeId, ModuleNode>
```

**Diagnostic Value:**
- Access all modules in the tree
- Full module hierarchy inspection

**Suggested Command:** `parse tree-modules <path>`

---

### 5.3 `ModuleTree::path_index`

**Location:** `syn_parser::resolve::module_tree::ModuleTree::path_index`

**Signature:**
```rust
pub fn path_index(&self) -> &HashMap<NodePath, AnyNodeId>
```

**Diagnostic Value:**
- Maps canonical paths to node IDs
- Useful for path-based lookups

**Suggested Command:** `parse path-index <path>`

---

### 5.4 `ModuleTree::tree_relations`

**Location:** `syn_parser::resolve::module_tree::ModuleTree::tree_relations`

**Signature:**
```rust
pub fn tree_relations(&self) -> &[TreeRelation]
```

**Diagnostic Value:**
- Access all module tree relations
- Shows module hierarchy and links

**Suggested Command:** `parse tree-relations <path>`

---

### 5.5 `ModuleTree::pending_imports`

**Location:** `syn_parser::resolve::module_tree::ModuleTree::pending_imports`

**Signature:**
```rust
pub fn pending_imports(&self) -> &[PendingImport]
```

**Diagnostic Value:**
- Shows imports that need resolution
- Useful for debugging import issues

**Suggested Command:** `parse pending-imports <path>`

---

### 5.6 `ModuleTree::pending_exports`

**Location:** `syn_parser::resolve::module_tree::ModuleTree::pending_exports`

**Signature:**
```rust
pub fn pending_exports(&self) -> &[PendingExport]
```

**Diagnostic Value:**
- Shows re-exports that need resolution
- Useful for debugging `pub use` issues

**Suggested Command:** `parse pending-exports <path>`

---

### 5.7 `ModuleTree::get_module_checked`

**Location:** `syn_parser::resolve::module_tree::ModuleTree::get_module_checked`

**Signature:**
```rust
pub fn get_module_checked(&self, module_id: &ModuleNodeId) -> Result<&ModuleNode, ModuleTreeError>
```

**Diagnostic Value:**
- Safe module retrieval with error handling
- Validates module exists in tree

**Suggested Command:** (internal use)

---

## 6. Workspace Discovery Functions

### 6.1 `locate_workspace_manifest`

**Location:** `syn_parser::discovery::workspace::locate_workspace_manifest`

**Signature:**
```rust
pub fn locate_workspace_manifest(crate_root: &Path) -> Result<(PathBuf, WorkspaceManifestMetadata), DiscoveryError>
```

**Diagnostic Value:**
- Finds workspace manifest by searching upward from crate
- Useful for understanding workspace structure

**Suggested Command:** `parse locate-workspace <path>`

---

### 6.2 `try_parse_manifest`

**Location:** `syn_parser::discovery::workspace::try_parse_manifest`

**Signature:**
```rust
pub fn try_parse_manifest(target_dir: &Path) -> Result<WorkspaceManifestMetadata, DiscoveryError>
```

**Diagnostic Value:**
- Parses workspace Cargo.toml
- Returns workspace metadata

**Suggested Command:** `parse manifest-info <path>`

---

### 6.3 `resolve_workspace_version`

**Location:** `syn_parser::discovery::workspace::resolve_workspace_version`

**Signature:**
```rust
pub fn resolve_workspace_version(crate_root: &Path) -> Result<String, DiscoveryError>
```

**Diagnostic Value:**
- Resolves workspace-inherited version
- Handles `version.workspace = true` syntax

**Suggested Command:** `parse workspace-version <path>`

---

## 7. Graph Access Getters

### 7.1 Primary Getters (GraphAccess trait)

All available on types implementing `GraphAccess` (e.g., `ParsedCodeGraph`, `CodeGraph`):

| Function | Return Type | Description |
|----------|-------------|-------------|
| `functions()` | `&[FunctionNode]` | All standalone functions |
| `defined_types()` | `&[TypeDefNode]` | All type definitions (structs, enums, unions, type aliases) |
| `type_graph()` | `&[TypeNode]` | Type nodes for semantic analysis |
| `impls()` | `&[ImplNode]` | All impl blocks |
| `traits()` | `&[TraitNode]` | All trait definitions |
| `relations()` | `&[SyntacticRelation]` | All syntactic relations |
| `modules()` | `&[ModuleNode]` | All module nodes |
| `consts()` | `&[ConstNode]` | All const items |
| `statics()` | `&[StaticNode]` | All static items |
| `macros()` | `&[MacroNode]` | All macro definitions |
| `use_statements()` | `&[ImportNode]` | All use/import statements |
| `unresolved_nodes()` | `&[UnresolvedNode]` | Unresolved items |

**Suggested Commands:**
- `parse stats <path>` - Show counts of all node types
- `parse list-functions <path>` - List all functions
- `parse list-types <path>` - List all type definitions
- `parse list-modules <path>` - List all modules

---

## 8. Typed Node Getters

### 8.1 Checked Getters (return `Result`)

| Function | Location | Description |
|----------|----------|-------------|
| `get_function_checked` | `GraphAccess` | Get function by ID with validation |
| `get_module_checked` | `GraphAccess` | Get module by ID with validation |
| `get_trait_checked` | `GraphAccess` | Get trait by ID with validation |
| `get_impl_checked` | `GraphAccess` | Get impl by ID with validation |
| `get_struct_checked` | `GraphAccess` | Get struct by ID with validation |
| `get_enum_checked` | `GraphAccess` | Get enum by ID with validation |
| `get_type_alias_checked` | `GraphAccess` | Get type alias by ID with validation |
| `get_union_checked` | `GraphAccess` | Get union by ID with validation |
| `get_const_checked` | `GraphAccess` | Get const by ID with validation |
| `get_static_checked` | `GraphAccess` | Get static by ID with validation |
| `get_macro_checked` | `GraphAccess` | Get macro by ID with validation |
| `get_import_checked` | `GraphAccess` | Get import by ID with validation |

### 8.2 Unchecked Getters (return `Option`)

| Function | Location | Description |
|----------|----------|-------------|
| `get_function` | `GraphAccess` | Get function by ID |
| `get_module` | `GraphAccess` | Get module by ID |
| `get_trait` | `GraphAccess` | Get trait by ID |
| `get_impl` | `GraphAccess` | Get impl by ID |
| `get_struct_unchecked` | `GraphAccess` | Get struct by ID |
| `get_enum_unchecked` | `GraphAccess` | Get enum by ID |
| `get_type_alias_unchecked` | `GraphAccess` | Get type alias by ID |
| `get_union` | `GraphAccess` | Get union by ID |
| `get_const` | `GraphAccess` | Get const by ID |
| `get_static` | `GraphAccess` | Get static by ID |
| `get_macro` | `GraphAccess` | Get macro by ID |
| `get_import` | `GraphAccess` | Get import by ID |

---

## 9. Suggested New Xtask Commands Summary

### Diagnostic Commands

| Command | Function(s) Used | Description |
|---------|------------------|-------------|
| `parse validate-relations <path>` | `validate_unique_rels` | Check relation uniqueness |
| `parse debug-relations <path>` | `debug_relationships` | Debug print all relations |
| `parse list-items <path>` | `debug_print_all_visible` | List all parsed items |
| `parse find-node <path> <id>` | `find_node_unique` | Find node by ID |
| `parse find-module <path> <path>` | `find_module_by_path_checked` | Find module by path |

### Metadata Commands

| Command | Function(s) Used | Description |
|---------|------------------|-------------|
| `parse crate-info <path>` | `CrateContext` fields | Show crate metadata |
| `parse dependencies <path>` | `dependency_names` | List crate dependencies |
| `parse features <path>` | `Features` methods | List Cargo features |
| `parse list-files <path>` | `CrateContext::files` | List source files |
| `parse stats <path>` | All getter methods | Show parsing statistics |

### Discovery Commands

| Command | Function(s) Used | Description |
|---------|------------------|-------------|
| `parse discovery-list <path>` | `iter_crate_contexts` | List discovered crates |
| `parse warnings <path>` | `DiscoveryOutput::warnings` | Show discovery warnings |
| `parse check-warnings <path>` | `has_warnings` | Check for warnings |
| `parse locate-workspace <path>` | `locate_workspace_manifest` | Find workspace root |
| `parse manifest-info <path>` | `try_parse_manifest` | Show workspace manifest info |

### Module Tree Commands

| Command | Function(s) Used | Description |
|---------|------------------|-------------|
| `parse tree-root <path>` | `ModuleTree::root` | Show tree root |
| `parse tree-modules <path>` | `ModuleTree::modules` | List all modules |
| `parse tree-relations <path>` | `ModuleTree::tree_relations` | Show tree relations |
| `parse path-index <path>` | `ModuleTree::path_index` | Show path index |
| `parse pending-imports <path>` | `ModuleTree::pending_imports` | List pending imports |
| `parse pending-exports <path>` | `ModuleTree::pending_exports` | List pending exports |

---

## 10. Integration Notes

### For Agent Diagnostics

These functions provide agents with the ability to:

1. **Validate parsing results** - Check for duplicate relations, orphaned nodes
2. **Inspect code structure** - List modules, functions, types, dependencies
3. **Debug issues** - Find specific nodes, trace relations, examine imports
4. **Get metadata** - Crate info, features, workspace structure
5. **Analyze coverage** - See what's been parsed vs what's in source files

### Dependencies

Most of these functions require:
- A parsed `ParsedCodeGraph` or `DiscoveryOutput`
- Some require `ModuleTree` (built after merging)
- `CrateContext` needed for dependency/feature info

### Error Handling

Functions return:
- `Result<T, SynParserError>` for checked operations
- `Option<T>` for unchecked lookups
- `bool` for validation functions
- Empty collections when no data available

---

## 11. Priority Recommendations

### High Priority (implement first)

1. `parse stats <path>` - Quick overview of parsed content
2. `parse crate-info <path>` - Essential metadata
3. `parse dependencies <path>` - Dependency analysis
4. `parse list-modules <path>` - Module structure
5. `parse validate-relations <path>` - Data integrity check

### Medium Priority

6. `parse find-node <path> <id>` - Node lookup
7. `parse find-module <path> <path>` - Module lookup
8. `parse warnings <path>` - Discovery issues
9. `parse tree-relations <path>` - Tree structure

### Lower Priority (specialized use)

10. `parse debug-relations <path>` - Verbose debugging
11. `parse pending-imports <path>` - Import resolution debugging
12. `parse path-index <path>` - Advanced tree inspection

---

*Document generated as part of M.1.2 - Additional syn_parser Functions Survey*
