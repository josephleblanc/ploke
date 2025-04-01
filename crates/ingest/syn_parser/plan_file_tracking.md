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

## Key Functions
- `resolve_visibility()` - Main visibility resolution logic
- `get_item_module_path()` - Finds module path for an item
- `is_path_visible()` - Checks if path is in current scope
- `check_use_statements()` - Handles use statement impacts

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
