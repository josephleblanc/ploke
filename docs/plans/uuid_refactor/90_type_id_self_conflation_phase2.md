# Known Issue: TypeId Conflation for `Self` Keyword in Phase 2 Parsing

**Date:** 2025-04-10
**Status:** Documented Limitation

## 1. Summary

During the parallel parsing phase (Phase 2) of the UUID refactor, the mechanism for generating `TypeId::Synthetic` identifiers currently leads to the **conflation** of the `Self` keyword across different `impl` blocks within the same file. This means that the parser generates the *same* `TypeId` for `Self` when it appears in `impl TypeA { ... }` as when it appears in `impl TypeB { ... }`, even though `Self` refers to different concrete types (`TypeA` vs. `TypeB`) in those contexts.

This behavior is a known limitation of the Phase 2 design, which prioritizes parallel, context-free file processing over immediate semantic resolution. The resolution of `Self` to its correct concrete type is planned for Phase 3.

## 2. Background: TypeId Generation in Phase 2

Phase 2 parsing aims to quickly generate a provisional `CodeGraph` for each file. `TypeId::Synthetic` identifiers are created by the `get_or_create_type` function, primarily based on:

1.  The `crate_namespace` (UUID).
2.  The `file_path` (PathBuf) of the file being parsed.
3.  A string representation of the `syn::Type`, obtained via `type.to_token_stream().to_string()`.

This string representation is used as a key component in the UUIDv5 hash generation for the `TypeId`.

## 3. Discovery of the Issue

The issue was confirmed during the testing of `ImplNode` generation (see [tests in `nodes/impls.rs`](../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/impls.rs)).

*   **Initial Test:** A test (`test_impl_node_self_type_conflation_phase2`) was designed to *fail* by asserting that the `TypeId` generated for `Self` used as a return type in `impl SimpleStruct { fn new() -> Self; }` would be *different* from the `TypeId` generated for the underlying `Self` type in the `&self` parameter of a method in `impl GenericStruct<T> { fn print_value(&self); }`.
*   **Expectation:** It was assumed that because the `TypeId` generation relied heavily on the literal string `"Self"` (which is how `syn` represents the `Self` type path), both usages would hash to the same `TypeId::Synthetic` within the same file context, causing the `assert_ne!` in the test to fail (panic). The test was marked with `#[should_panic]`.
*   **Observation:** The test initially passed unexpectedly. However, after removing the `#[should_panic]` attribute, the test failed exactly as predicted, confirming that the `assert_ne!` failed because the two `TypeId`s *were identical*.
*   **Test Link:** The current test verifying this behavior (which passes because the expected panic occurs) is: [`test_impl_node_self_type_conflation_phase2`](../../crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/nodes/impls.rs).

## 4. Root Cause Analysis

The root cause is the reliance on the context-free string representation `"Self"` during `TypeId` generation in Phase 2.

1.  `syn` represents both usages of the `Self` keyword (e.g., `-> Self`, `&self`) using a `syn::Type::Path` whose identifier is `"Self"`.
2.  The `type_to_string()` utility converts this `syn::Type` into the literal string `"Self"`.
3.  `TypeId::generate_synthetic` uses this string `"Self"`, along with the *same* `crate_namespace` and `file_path`, to generate the UUID.
4.  Because the inputs to the UUID generation are identical for both usages of `Self` within the same file, the resulting `TypeId::Synthetic` is also identical.
5.  The parser in Phase 2 lacks the necessary contextual information (i.e., which `impl` block it is currently inside) to differentiate the meaning of `Self`.

This correctly implements the Phase 2 strategy but results in a semantically inaccurate `TypeId` at this stage.

## 5. Scope of Impact (Speculation)

*   **Confirmed:** Affects `Self` keyword usage within different `impl` blocks in the same file (parameters, return types, potentially field types if `Self` were used there).
*   **Speculated:** It is highly likely that this same conflation occurs for `Self` usage within `trait` definitions (e.g., `trait Example { fn method(&self) -> Vec<Self>; }`). If multiple traits using `Self` are defined in the same file, the `TypeId` for `Self` would likely be the same for all of them during Phase 2. This has not been explicitly tested yet.
*   **Unaffected:** Does not affect the `TypeId` generated for the concrete type being implemented (e.g., the `TypeId` for `SimpleStruct` in `impl SimpleStruct` is distinct from the `TypeId` for `GenericStruct<T>` in `impl GenericStruct<T>`).

## 6. Potential Solutions & Chosen Path

1.  **Phase 3 Resolution (Current Plan):** This remains the intended solution.
    *   Phase 3 will resolve the concrete `self_type` for each `ImplNode`.
    *   A subsequent step in Phase 3 will iterate through the items associated with each `impl` (methods, etc.) and replace occurrences of the generic "Self" `TypeId` with the specific `TypeId` of the concrete `self_type` for that `impl` block.
    *   *Pros:* Keeps Phase 2 simple and parallelizable. Performs resolution when full context is available.
    *   *Cons:* Defers semantic accuracy. Requires careful implementation in Phase 3.

2.  **Span-Based `TypeId` Generation:** Incorporate the `span` of the `Self` keyword usage into the `TypeId` hash.
    *   *Pros:* Creates unique IDs for each *syntactic occurrence* of `Self`.
    *   *Cons:*
        *   Does not solve the semantic goal (all `Self` within *one* impl should resolve to the *same* ID).
        *   Makes Phase 3 resolution much harder (no single "Self" ID to find/replace).
        *   Requires reliable access to the specific `Self` keyword span during `get_or_create_type`, which may be difficult.

3.  **Contextual `TypeId` Generation (Phase 2 Refactor):** Pass the current `impl` context (e.g., the `TypeId` of the concrete type being implemented) down through the visitor state. Generate `TypeId` for `Self` based on this context.
    *   *Pros:* Generates semantically correct `TypeId`s in Phase 2.
    *   *Cons:* Significantly increases visitor complexity, potentially breaks parallelism assumptions, makes visitor state management harder.

**Chosen Path:** We will adhere to **Solution 1 (Phase 3 Resolution)**. The Phase 2 conflation is accepted as a necessary intermediate state. Tests like `test_impl_node_self_type_conflation_phase2` serve to document and verify this expected intermediate behavior.
