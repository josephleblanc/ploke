//! # Node Test Coverage Gaps and Future Work
//!
//! This document summarizes areas where the node-specific tests within this module
//! (using the `paranoid_test_fields_and_values!` macro) have known coverage gaps
//! or areas for future improvement, based on the analysis in individual test files
//! (`consts.rs`, `statics.rs`, `functions.rs`, `imports.rs`, `modules.rs`).
//!
//! ## Common Gaps Across Node Types:
//!
//! *   **`#[cfg]` Attributes:** Consistently poor coverage. Fixtures and tests are needed for
//!     items declared with various `#[cfg(...)]` attributes to ensure correct parsing
//!     and ID generation based on CFG context.
//! *   **Attributes:** Coverage is often limited to empty attribute lists or simple examples.
//!     Tests for items with more complex attributes (multiple attributes, varied arguments)
//!     are needed.
//! *   **Docstrings:** Coverage often limited to `None`. Tests for items with actual docstrings
//!     are needed for `FunctionNode`, etc.
//! *   **Span Checks:** Explicit checks for span values (beyond non-zero) are not performed
//!     by the current macro framework.
//! *   **Relation Checks:** The macro currently only checks for `SyntacticRelation::Contains`.
//!     Checks for other potentially relevant relations (like `ModuleImports` for `ImportNode`,
//!     or relations specific to associated items) are not included.
//!
//! ## Specific Node Type Gaps:
//!
//! *   **`FunctionNode`:**
//!     *   Detailed type checking: Verification of specific `TypeId`s or `TypeKind`s for
//!         parameters and return types is missing.
//!     *   Parameter/Generic details: Checks for specific parameter names or generic parameter
//!         details (names, bounds) are missing.
//!     *   Function kinds: Tests for functions without bodies (trait/extern), `async`, `const`,
//!         and `unsafe` functions are needed.
//! *   **`ImportNode`:**
//!     *   Visibility: Tests for `use` statements with explicit visibility (`pub`, `pub(crate)`,
//!         `pub(in path)`) are missing.
//! *   **`ConstNode`:**
//!     *   Associated consts: Detailed tests for `const` items within `impl` blocks (inherent
//!         or trait) are missing.
//! *   **`StaticNode`:**
//!     *   Missing detailed tests: Several `static` items from the fixture lack detailed field checks.
//!     *   Visibility: `VisibilityKind::Crate` is not covered by detailed tests.
//!     *   Associated statics: Not currently tested (and may not be applicable).
//! *   **`ModuleNode`:**
//!     *   Visibility: `VisibilityKind::Restricted` (e.g., `pub(in path)`) is not explicitly tested.
//!     *   `exports` field: Population of re-exported items is a Phase 3 concern and not tested in Phase 2.
//!     *   `ModuleKind::Declaration.resolved_definition`: This link to a definition is established in Phase 3 and not tested in Phase 2.
//!     *   `RelationKind::ModuleDeclarationResolvesToDefinition`: This relation is established in Phase 3.
//!     *   `extern crate` items: Ensuring `extern crate` statements are correctly reflected in `items_count` and lead to `ImportNode` creation.
//!     *   `#[cfg_attr(..., path = "...")]`: Modules declared with conditional path attributes.
//!
//! ## General Notes:
//!
//! This list focuses on gaps identified in the refactored tests using the macro framework.
//! Other node types not yet refactored (e.g., `StructNode`, `EnumNode`, `TraitNode`) will
//! require similar analysis once updated.

// -- Files that have been updated for the new typed id system are here:
mod consts;
mod functions;
mod imports;
mod modules;
mod statics;

// -- Files that have yet to be updated are gated behind the cfgs below:
#[cfg(not(feature = "type_bearing_ids"))]
mod enums;
#[cfg(not(feature = "type_bearing_ids"))]
mod impls;
#[cfg(not(feature = "type_bearing_ids"))]
mod macros;
#[cfg(not(feature = "type_bearing_ids"))]
mod structs;
#[cfg(not(feature = "type_bearing_ids"))]
mod traits;
#[cfg(not(feature = "type_bearing_ids"))]
mod type_alias;
#[cfg(not(feature = "type_bearing_ids"))]
mod unions;
// Add other node types here later:
//   const_alias
