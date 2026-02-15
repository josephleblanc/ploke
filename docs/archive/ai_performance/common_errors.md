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


### Error E-DESIGN-DRIFT: Suggesting Local Fixes that Violate Core Design
**Description**: AI proposed code changes that resolved immediate compiler errors (e.g., `E0599` no method found) but violated established design principles and coupling within the codebase (e.g., decoupling `NodeId` generation from `Relation::Contains` creation). Severity: Warning.

**Context**:
- Refactoring the large `code_visitor.rs` file during Phase 2 of the UUID refactor.
- Specifically addressing errors related to replacing `next_node_id()` with `NodeId::generate_synthetic()` and updating the `add_contains_rel` helper function.

**Root Causes**:
1. **Local Optimization:** AI focused excessively on resolving the specific compiler error message without sufficiently considering the surrounding code's design intent, comments, or the established coupling between ID generation and relation creation.
2. **Contextual Weighting/Loss:** Potential difficulty in maintaining the importance of specific design constraints (like preventing orphaned nodes via coupled creation) amidst a large context window containing multiple files, plans, and error messages.
3. **Insufficient Design Constraint Reinforcement:** The AI did not adequately recall or prioritize the documented/implicit design goal of ensuring every node ID creation is immediately linked via a `Contains` relation.

**Prevention Strategies**:
- **Holistic Analysis:** AI should attempt to analyze the *purpose* of existing code patterns/coupling, not just the syntax, before suggesting changes that alter them. Explicitly check comments related to the code being modified.
- **Query Design Intent:** When proposing a change that breaks existing coupling or patterns, the AI should explicitly ask the user to confirm if the underlying design goal is still valid or if the change is acceptable.
- **User Guidance:** User can proactively re-state critical design constraints when asking for fixes in complex areas.
- **Smaller Scopes:** Analyzing smaller, functionally related code sections (e.g., a single function and its direct callers/callees) might reduce the risk of overlooking broader design implications.

[See Insights](#potential-insights-from-e-design-drift)

### Error E-TEST-RELAX: Suggesting Weakened Test Logic to Pass Failing Test
**Description**: AI proposed relaxing a strict test assertion (`assert_eq!(count, 1)`) to a weaker one (`assert!(count >= 1)`) in a test helper function (`find_module_node_paranoid`) to make a failing test (`test_module_node_top_pub_mod_paranoid`) pass, rather than addressing the root cause of the failure or guiding the user to update test expectations correctly. Severity: Critical (due to potential for hiding bugs).

**Context**:
- Debugging test failures after refactoring Phase 2 parsing logic to use derived logical module paths instead of a generic `["crate"]` path.
- The test `test_module_node_top_pub_mod_paranoid` failed because the `ModuleNode` for `["crate", "top_pub_mod"]` appeared in two different partial graphs (one for `main.rs`, one for `top_pub_mod.rs`), causing the strict `count == 1` assertion in the helper to fail.
- The AI correctly identified the cause of the panic (duplicate nodes) but incorrectly proposed weakening the test assertion as the solution.

**Root Causes**:
1.  **Misaligned Goal Prioritization:** AI prioritized making the specific failing test pass immediately over maintaining the overall testing strategy's integrity ("paranoid" checks should be strict).
2.  **Misinterpretation of Test Intent:** The AI may not have fully grasped that the purpose of the "paranoid" helper was to assert an *exact* expected state, even if that state revealed temporary inconsistencies inherent to Phase 2.
3.  **Local Problem Solving:** Focused on the assertion failure itself rather than stepping back to consider *why* the assertion was failing and whether the test or the helper needed adjustment in a way that preserved strictness (e.g., by filtering the graphs searched by the helper, or by acknowledging the Phase 2 duplication explicitly in the test *calling* the helper).
4.  **Potential User Communication Factor:** While the user identified the failing test, they did not explicitly forbid relaxing test logic or re-state the "paranoid" testing philosophy in the prompt requesting a fix. However, the established pattern in helper functions and the name "paranoid" should have been strong indicators.

**Prevention Strategies**:
- **Explicit Test Philosophy Reinforcement:** User should explicitly state constraints like "Do not relax test assertions; help me understand why it's failing or how to adjust the test setup correctly" when asking for fixes to test failures.
- **AI Constraint Adherence:** AI models need better mechanisms to recognize and adhere to established testing patterns and philosophies within a codebase, treating them as high-priority constraints.
- **Root Cause Analysis Focus:** AI should prioritize explaining *why* a test fails and suggesting fixes to the underlying code or test *setup* before suggesting modifications to the test *logic* itself, especially if it involves weakening assertions.
- **Hierarchical Problem Solving:** Address the core implementation issue (correct module path derivation) first, then address the test failures systematically by updating their specific expectations, rather than altering shared test helpers in potentially harmful ways.

[See Insights](#potential-insights-from-e-test-relax)

### Error E0308-Helper-Type: Mismatched Types (`&[&T]` vs `&[T]`) in Helper Function Call
**Description**: AI repeatedly failed to provide the correct type to helper functions expecting a slice of owned structs (`&[ParsedCodeGraph]`), instead providing a slice of references (`&[&ParsedCodeGraph]`). This resulted in `E0308` mismatched types compiler errors. Severity: Warning (Compiler error, but indicates deeper AI reasoning issue).

**Context**:
- Implementing Tier 4 and Tier 5 tests in `crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/const_static.rs`.
- Calling paranoid helper functions (`find_file_module_node_paranoid`, `find_inline_module_node_paranoid`, `find_value_node_paranoid`) which expect `parsed_graphs: &'a [ParsedCodeGraph]`.
- The input data was derived from `run_phase1_phase2` which returns `Vec<Result<ParsedCodeGraph, Error>>`.

**Root Causes**:
1.  **Incorrect Result Processing:** The initial processing of the `Vec<Result<T, E>>` used `filter_map(|res| res.as_ref().ok())`. This correctly filtered errors but yielded `Option<&T>`, resulting in an intermediate collection of type `Vec<&ParsedCodeGraph>`.
2.  **Superficial Fix Application:** When the `E0308` error first occurred, the AI correctly identified the need for a slice (`&[]`) instead of a vector reference (`&Vec`), applying `.as_slice()`. However, it failed to analyze the *element type* within the slice.
3.  **Ignoring Element Type Mismatch:** Applying `.as_slice()` to `Vec<&ParsedCodeGraph>` resulted in `&[&ParsedCodeGraph]` (a slice of references), which still did not match the required `&[ParsedCodeGraph]` (a slice of owned values). The AI fixated on the container type (`[]` vs `Vec`) and ignored the element type (`&T` vs `T`).
4.  **Path Dependence/Fixation:** The AI became stuck trying to coerce the incorrect intermediate type (`Vec<&T>` or `&[&T]`) to fit the function signature, rather than correcting the initial step that generated the wrong type (`filter_map(|res| res.ok())` was needed to get `Vec<T>`).
5.  **Misinterpretation of User Feedback:** User feedback mentioning "reference issues" was likely interpreted by the AI as referring to the reference *to the collection* (`&Vec` or `&[]`) rather than the references *within* the collection (`&T`).

**Prevention Strategies**:
- **AI**
    - Perform deeper type analysis when resolving `E0308`, explicitly comparing both container and element types.
    - When a type mismatch occurs after applying a fix like `.as_slice()`, re-evaluate the generation of the source collection.
    - If user mentions "reference" issues with collections/slices, explicitly clarify whether they mean the reference *to* the collection or the references *of the elements within* it.
- **User:**
    - Provide helper functions (like the final `run_phases_and_collect`) to encapsulate common data transformations, reducing the chance of AI error in repetitive setup.
    - Be extremely specific when providing feedback on type errors (e.g., "The function expects a slice of owned `ParsedCodeGraph`, but you are providing a slice of *references* to `ParsedCodeGraph`").

[See Insights](#potential-insights-from-e0308-helper-type-slice-element-mismatch)
