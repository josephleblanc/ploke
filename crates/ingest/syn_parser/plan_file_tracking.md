# Scope Tracking System Analysis

## Key Data Structures

### Core Types
1. **ModuleNode** - Tracks module hierarchy and contained items
   - `path`: Module path segments (when feature enabled)
   - `submodules`: Child module IDs
   - `items`: Contained item IDs

2. **ImportNode** - Represents use statements and extern crates
   - `path`: Import path segments
   - `kind`: Distinguishes between use statements and extern crates

3. **UseStatement** - Detailed use statement info
   - `path`: Original path segments
   - `visible_name`: Final name in scope (handles renames)

4. **VisibilityKind** - Enum of visibility levels
   - `Public`, `Crate`, `Restricted(path)`, `Inherited`

5. **VisibilityResult** - Result of visibility checks
   - `Direct`, `NeedsUse(path)`, `OutOfScope{reason, allowed_scopes}`

### Supporting Types
- **CodeGraph** - Contains all modules, items and relations
- **Relation** (Contains kind) - Tracks parent-child relationships
- **TypeKind::Named** - Contains path info for type references

## Key Functions and Their Interactions

### Visibility Resolution Flow
1. `get_item_module_path()` - Called first to determine:
   - The module hierarchy path where an item is defined
   - Uses `Contains` relations to walk up the module tree
   - Returns path segments like `["crate", "module", "submodule"]`

2. `is_path_visible()` - Checks if:
   - The item's module path is within current context
   - Handles visibility restrictions (pub(crate), pub(super) etc.)
   - Returns boolean for basic path visibility

3. `check_use_statements()` - Examines:
   - Relevant use statements in current scope
   - Renames and glob imports
   - Returns `VisibilityResult` (Direct/NeedsUse/OutOfScope)

4. `resolve_visibility()` - Orchestrates the above:
   - Calls functions in sequence
   - Combines results into final visibility determination
   - Handles edge cases and fallbacks

### Module Building Functions
- `visit_item_mod()` - Called during AST traversal to:
  - Create new `ModuleNode` for each module
  - Track current module path in `VisitorState`
  - Establish parent-child relationships via `add_contains_rel()`

- `add_contains_rel()` - Creates containment edges:
  - Links modules to their contained items
  - Used by `get_item_module_path()` to walk hierarchy
  - Maintained for all item types (functions, structs, etc.)

### Interaction Diagram
```
AST Traversal → visit_item_mod()
    ↓
add_contains_rel() → CodeGraph Relations
    ↓
Later Visibility Check:
get_item_module_path() → is_path_visible() → check_use_statements()
    ↓
resolve_visibility() returns final result
```

### Key Invariants
1. Module hierarchy must be built before visibility checks
2. `Contains` relations must accurately reflect nesting
3. Current module path stack must match traversal position

## Terminology Proposal
"Scope Tracking" seems appropriate as it encompasses:
- Module hierarchy tracking
- Visibility resolution rules 
- Use statement impacts on scope

## Questions for Further Analysis
1. Are there any edge cases in visibility resolution that need special handling?
2. Should we track workspace/dependency boundaries more explicitly?
3. How should we handle macro-expanded items in scope tracking?
4. Are there performance considerations for large module hierarchies?

## Project-Wide Scope Tracking Requirements

### Current Limitations
1. Single-file focus in parser
2. No distinction between user code vs dependencies
3. Missing file path context in visibility checks
4. No workspace/module boundary tracking

### Key Changes Needed

#### Data Structure Changes
1. **CodeGraph**:
   - Add `source: CodeSource` enum (UserCode, Dependency)
   - Add `file_path: PathBuf` to all nodes
   - Add `workspace_root: Option<PathBuf>`

2. **ModuleNode**:
   - Add `file_path: PathBuf` for mod.rs/lib.rs files
   - Add `is_workspace_boundary: bool` flag

3. **ImportNode**:
   - Add `resolved_path: Option<PathBuf>` for dependency paths
   - Add `is_external: bool` flag

4. **VisitorState**:
   - Add `current_file: PathBuf`
   - Add `workspace_roots: Vec<PathBuf>`
   - Add `dependency_mode: bool` flag

#### Function Changes
1. **analyze_code()**:
   - Accept `source: CodeSource` parameter
   - Store file path in visitor state
   - Handle module file discovery (mod.rs/lib.rs)

2. **resolve_visibility()**:
   - Check `CodeSource` when evaluating cross-crate visibility
   - Consider workspace boundaries
   - Handle dependency-specific visibility rules

3. **get_item_module_path()**:
   - Return absolute paths for dependencies
   - Handle workspace-relative paths for user code

4. New Functions Needed:
   - `discover_module_files()` - Find all module files in project
   - `resolve_dependency_path()` - Map use paths to dependency files
   - `is_in_workspace()` - Check if path is within workspace

#### File Changes Required
1. **graph.rs**:
   - Modify `CodeGraph` and node structs
   - Add new visibility resolution logic

2. **nodes.rs**:
   - Add new fields to node structs
   - Update `Visible` trait implementations

3. **visitor/mod.rs**:
   - Add file discovery logic
   - Modify analysis entry points

4. **visitor/state.rs**:
   - Extend `VisitorState` with new fields
   - Add workspace tracking

### Dependency Handling Considerations
1. Two-tier visibility system:
   - User code: Full scope tracking
   - Dependencies: Public items only by default

2. Dependency analysis modes:
   - Lightweight: Only public API surface
   - Full: All items with visibility markers

3. Crate boundary rules:
   - `pub` means public to dependents
   - `pub(crate)` is crate-internal
   - Private items never visible

### Validation Requirements
1. Unit tests for:
   - Cross-crate visibility resolution
   - Workspace boundary detection
   - Dependency path resolution

2. Integration tests with:
   - Multi-file projects
   - Workspace setups
   - External dependencies

3. Benchmarking:
   - Project size scaling
   - Dependency analysis overhead

## Recommended Implementation Path
1. First add file path tracking to nodes
2. Implement workspace boundary detection
3. Add dependency analysis mode
4. Extend visibility resolution rules
5. Add validation tooling

## Database Integration Notes
The enhanced scope tracking will enable:
- Querying by precise file locations
- Filtering by user code vs dependencies
- Scope-aware snippet retrieval
- Workspace boundary awareness in RAG

This aligns with CozoDB's graph capabilities by:
- Making scope a queryable property
- Enabling path-based filtering
- Supporting multi-crate analysis
