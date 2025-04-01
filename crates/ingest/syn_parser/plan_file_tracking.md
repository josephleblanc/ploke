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

## Recommended Next Steps
1. Audit current test coverage for scope tracking features
2. Document expected behavior for complex cases:
   - Nested modules with restricted visibility
   - Use statement renaming and glob imports
   - Cross-crate visibility rules
3. Benchmark performance on large codebases
4. Consider adding more detailed path tracking for better error messages
