# Common Error Patterns in AI-Assisted Development

## Error E0774: Misplaced Derive Attribute
**Description**: Attempting to apply `#[derive]` to trait implementations rather than type definitions.

**Context**: 
- Occurred while implementing the `Visible` trait across multiple node types
- Was trying to keep all related code together but misplaced attributes

**Root Causes**:
1. Pattern matching without context awareness
2. Not distinguishing between type definitions and implementations
3. Working across large files with many similar-looking blocks

**Prevention Strategies**:
- Clear separation of type definitions and trait implementations in code
- Linting rule: "Derives only on type definitions"
- Context-aware editing that understands Rust syntax boundaries

[See Insights](#potential-insights-from-e0774)

## Error E0405: Missing Trait in Scope  
**Description**: Referencing a trait that hasn't been imported or defined.

**Context**:
- Implementing visibility resolution system
- Trait was defined but not properly imported across modules

**Root Causes**:
1. Feature gating complexity
2. Cross-module dependencies not tracked
3. Partial context in AI working memory

**Prevention Strategies**:
- Better visibility tracking for traits
- Automated import suggestions
- Feature flag awareness in tooling

[See Insights](#potential-insights-from-e0405)

### Error E0499: Multiple Mutable Borrows in Visitor Pattern
**Description**: Attempting to mutate visitor state while already holding a mutable reference during AST traversal.

**Context**:
- Occurred while processing module items in code visitor
- Nested structure traversal requires multiple state mutations
- Specifically during module path tracking implementation

**Root Causes**:
1. Complex nested ownership patterns in visitor implementation
2. Not accounting for recursive borrows in AST traversal
3. State management design that doesn't separate lookup from mutation

**Prevention Strategies**:
- Separate mutable operations into distinct phases
- Use interior mutability patterns for shared state
- Better static analysis of borrow patterns in visitor code

[See Insights](#potential-insights-from-e0499)

### Error Cascade E-UUID-Refactor: Multi-File Refactor Failure
**Description**: A large number (54) of compilation errors occurred after attempting to refactor core data types (`NodeId`, `TypeId`) and related structures (`*Node`, `VisitorState`) across multiple files (`ploke-core`, `syn_parser/src/parser/*`, `syn_parser/src/parser/visitor/*`) simultaneously under a feature flag (`uuid_ids`).

**Context**:
- Implementing Phase 2 of the UUID refactoring plan.
- Modifying structs to use new ID types from `ploke-core`.
- Adding `tracking_hash` field.
- Changing `VisitorState` fields and methods conditionally.

**Root Causes**:
1. **Incomplete Application:** Structural changes (new fields, type changes) were made, but the code *using* these structures (initializers, method calls) was not updated consistently under the feature flag.
2. **Missing Trait Implementations:** The new `NodeId` enum was missing required trait implementations (`Ord`, `PartialOrd`) needed by deriving structs (`Relation`).
3. **Import/Visibility Issues:** Problems with how the new types were imported or re-exported under conditional compilation.
4. **Overly Broad Change Set:** Attempting to modify too many interdependent files and concepts in a single step, making it hard to track all necessary downstream changes.

**Prevention Strategies**:
- **Incremental Refactoring:** Apply changes step-by-step, focusing on one aspect (e.g., fixing trait bounds, updating initializers, replacing method calls) at a time.
- **Compile After Each Step:** Run `cargo check` (with and without the feature flag) frequently after small changes.
- **Stubbing:** Implement new methods/functions as stubs first to satisfy the compiler before adding complex logic.
- **Focused File Access:** Request changes for a smaller, related set of files per interaction.

[See Insights](#potential-insights-from-e-uuid-refactor)
