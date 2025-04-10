# Ploke Project: syn_parser Testing Philosophy

**Core Principle:** Tests for `syn_parser`, especially under `uuid_ids`, must be rigorous and paranoid to ensure the foundational correctness of the parsing pipeline. Speed of test writing is secondary to robustness and the prevention of regressions or subtle errors.

**Key Directives for AI:**

1.  **Strict Assertions:**
    *   **NO Weakening:** Never relax assertions (`assert_eq!`, `assert!`) to make a test pass. Explain failures instead.
    *   **Exactness:** Prefer exact checks (e.g., specific counts, specific enum variants, exact relation kinds) over existence checks (`>= 1`, `is_some()`) unless the test explicitly targets variability.
2.  **Node Identification:**
    *   **Use Paranoid Helpers:** Always use the `find_*_node_paranoid` helpers from `uuid_ids_utils.rs` to locate nodes in graphs for assertions.
    *   **Respect ID Regeneration:** These helpers rely on regenerating synthetic IDs based on expected context (namespace, path, name, span). Ensure test setup provides the *correct* context for this regeneration. Do *not* bypass this check.
    *   **NO Find-by-Name:** Do *not* use simple iteration and string comparison on `node.name` (e.g., `graph.functions.iter().find(|f| f.name == "...")`) for primary node identification in assertions. Use the paranoid helpers.
3.  **Invariant Validation:** Focus tests on verifying key structural invariants and relationships within the `CodeGraph` (e.g., node properties, `Contains` relations, type links) rather than superficial checks.
4.  **Context Accuracy:** Ensure test setup correctly reflects the expected state derived from fixture files and Phase 2 logic (e.g., derived module paths).
5.  **Phase Awareness:** Understand the limitations of Phase 2 (e.g., duplicate module nodes across partial graphs are expected, final IDs are not resolved). Test logic must account for this expected intermediate state *without relaxing core assertions*. Handle expected duplication via test setup or careful helper usage (e.g., filtering graphs searched), not by weakening checks.

**Goal:** Create tests that provide high confidence in the parser's output and act as reliable guards against regressions, even when assisted by AI. Assume any deviation from these principles in AI suggestions is an error requiring correction.
