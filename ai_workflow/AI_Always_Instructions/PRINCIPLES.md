Key Collaboration Principles:
  - Prioritize design integrity and test robustness over quick fixes.
  - Do NOT weaken test assertions; explain failures instead.
  - Maintain existing documented coupling unless explicitly told otherwise.
  - Work in small, incremental steps.
  - All changes must be consistent with conditional compilation flags.

--- 

**Core Principle:** Maintain code quality, design integrity, and test robustness above all else. AI suggestions are proposals requiring critical evaluation.

**Interaction Rules:**

1.  **Granularity:** Tasks will be broken down into small, specific, verifiable steps. Focus on one logical change per request.
2.  **Explicit Constraints:** Assume NO prior context between prompts. Critical design principles, testing philosophies (e.g., "paranoid checks are strict"), coupling requirements, and feature flag constraints MUST be restated with relevant requests.
3.  **Verification is Key:** All suggested code changes MUST be verified by the user (via `cargo check`/`test`) immediately. AI output will be evaluated based on this feedback.
4.  **Testing Integrity:**
    *   Do NOT suggest weakening test assertions (`assert!`, `assert_eq!`, etc.) to make a test pass.
    *   Explain *why* a test is failing and propose fixes to the underlying code or test *setup*, not the assertion logic itself, unless explicitly instructed.
    *   Respect the intent behind different test types (unit, integration, "paranoid").
5.  **Design Adherence:**
    *   Prioritize maintaining existing, documented design patterns and code coupling.
    *   Analyze comments indicating design intent near the code being modified.
    *   If a suggestion significantly alters existing structure or breaks coupling, explicitly state the rationale and potential risks. Ask for user confirmation.
6.  **Error Handling:** Focus on robust error handling; avoid shortcuts like `.unwrap()` unless clearly justified.
7.  **Conditional Compilation:** Ensure absolute consistency when working under feature flags (`#[cfg(...)]`). Changes must apply correctly to the intended code path.

**Common Pitfalls to Avoid (Derived from Past Errors):**

*   **Local Fixation:** Don't just fix the immediate compiler error; consider the broader impact on design and other code sections.
*   **Ignoring Intent:** Don't discard existing patterns or test logic without understanding *why* they exist.
*   **Overly Broad Changes:** Avoid attempting large refactors across many files or concepts in a single step.
*   **Inconsistency:** Ensure changes are applied uniformly, especially regarding types, function signatures, and feature flags.

**Goal:** Collaborative, safe, and maintainable code generation. Speed is secondary to correctness and robustness.
