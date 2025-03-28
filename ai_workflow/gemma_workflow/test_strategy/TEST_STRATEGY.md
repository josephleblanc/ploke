# Refactor Testing Strategy

**Goal:** Ensure the type alignment and related changes are correct and don't introduce regressions, while minimizing effort on the soon-to-be-removed `CodeGraph`.

**Test Types & Prioritization:**

1.  **Unit Tests (High Priority):**
    *   Focus: Verify the correctness of individual functions and modules involved in type conversion and data structure updates (e.g., `get_or_create_type`, `process_type`, the `CozoDbType` enum).  Adhere to [IDIOMATIC_RUST](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) guidelines for naming and structure.
    *   Coverage: Aim for high code coverage within the modified code.
    *   Location: `src/parser/tests/refactor` (separate module).
    *   Enabled via: `cozo_type_refactor` feature flag.  See [Type Alignment Tasks](gemma_workflow/tasks/type_alignment_tasks.md) for related subtasks.
2.  **Regression Tests (Medium Priority, Managed with `deprecated`):**
    *   Focus: Ensure existing functionality (as much as possible) continues to work.
    *   Approach:
        *   Mark deprecated code with `#[deprecated]` attributes, providing clear migration guidance.
        *   Update existing tests to use the deprecated code where necessary, but add comments indicating that these tests will eventually need to be updated.
        *   As code is replaced, update the tests accordingly.
    *   Location: Existing `tests/mod.rs` and other test files.
3.  **Minimal Integration Tests (Low Priority):**
    *   Focus: Verify that the parser produces *some* valid output, even if the `CodeGraph` is simplified or removed.
    *   Scope: Parse a small, representative Rust file and verify that the basic data structures are populated correctly. Avoid extensive testing of the `CodeGraph` itself.
    *   Location: `src/parser/tests/refactor` or a dedicated integration test directory.
    *   Enabled via: `cozo_type_refactor` feature flag.  See [Type Alignment Tasks](gemma_workflow/tasks/type_alignment_tasks.md) for related subtasks.
4.  **Property-Based Tests (Deferred):**
    *   Focus: Complex type conversions and data validation.
    *   Priority: Defer this until the core type alignment is complete and stable.

**Workflow:**

1.  **Implement a Subtask:** Make the necessary code changes.
2.  **Identify Relevant Test Category:** Determine which existing test category (e.g., `parser`, `serialization`) is most appropriate for the changes.
3.  **Add Tests to Existing Category:** Add new tests to the relevant test file, using fixtures from the `fixtures` directory as input.
4.  **Run Tests with Feature Flag:** `cargo test --features cozo_type_refactor`.
5.  **Update Regression Tests:** If the changes break existing tests, mark the affected code as `#[deprecated]` and update the tests to use the deprecated code.
6.  **Run Regression Tests:** `cargo test`.
7.  **Commit Changes:** Commit the code and tests.

**Key Principles:**

*   **Prioritize Unit Tests:** They provide the fastest and most focused feedback.
*   **Manage Regression Tests with `deprecated`:** This allows us to continue testing while gradually migrating to the new code.
*   **Minimize Effort on `CodeGraph`:** Focus on ensuring the parser produces valid output, not on refining the `CodeGraph`.

## Test Directory Structure

The tests for the `syn_parser` crate are organized into the following directories:

*   `common`: Contains common test utilities and helper functions.
*   `integration`: Contains integration tests that verify the interaction between different parts of the system.
*   `parser`: Contains unit tests for the parser, with separate files for different language features (e.g., enums, functions, structs).
*   `serialization`: Contains tests for the serialization logic (JSON and RON).
*   `refactor`: Contains tests that are specifically related to the current refactor (e.g., tests for the `CozoDbType` enum).
*   `fixtures`: Contains example Rust code that can be used as input for our tests.
*   `data`: Contains example RON files for testing the serialization logic.

Our refactor tests will primarily be integrated into the existing test categories, with the `refactor` directory reserved for tests that are specific to the refactor itself. We will leverage the fixtures in the `fixtures` directory to provide realistic test input.
