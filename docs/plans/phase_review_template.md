# Phase [Phase Number]: [Phase Name] Implementation Review

*Related Plan: [Link to specific phase implementation plan .md file]*
*Related Overview: [Link to overall plan overview .md file]*
*Related ADRs: [Link to relevant ADR .md files, if any]*

## Summary

This document reviews the implementation of Phase [Phase Number] ([Phase Name]) of the [Overall Feature Name, e.g., UUID Refactor] for `ploke`, focusing on changes within the `[crate_name]` crate(s).

**Key Outcomes & Decisions:**

*   **Functionality:** Briefly describe the core functionality implemented in this phase. What does it achieve?
*   **Key Design Choices:** Summarize the most important design decisions made during implementation (e.g., error handling strategy, data structures used, concurrency model choices).
*   **Feature Gating:** Confirm if the implementation is correctly gated behind the intended feature flag(s).
*   **Testing:** Briefly mention the types of tests added (unit, integration) and what they cover at a high level.

## Departures from Plans

*   **[Specific Phase Plan Link]:** Note any significant deviations from the detailed plan for this phase. Explain the reason for the departure. If none, state "No significant departures."
*   **[Overall Plan Overview Link]:** Note any deviations from the high-level overview plan relevant to this phase. If none, state "No departures."

## Input/Output Expectations for Integration

*   **`function_name_1(...) -> ReturnType`**
    *   **Input:** Describe the key inputs and any important assumptions about them (e.g., absolute paths, expected state).
    *   **Output (on Success):** Describe the structure and content of the successful output.
    *   **Output (on Failure):** Describe the error type returned and the conditions under which failure occurs.
*   **`function_name_2(...) -> ReturnType`**
    *   *(Repeat for other key functions that integrate with subsequent phases)*

## Known Limitations & Rationale

*   **Limitation 1:** Describe a known limitation of the current implementation for this phase.
    *   **Rationale:** Explain *why* this limitation exists (e.g., simplification for current scope, complexity deferred, trade-off made).
*   **Limitation 2:** (Add more as needed)
*   **Design Rationale:** Explain the reasoning behind any non-obvious design choices or trade-offs made during implementation. Why was approach X chosen over approach Y?

## Testing Overview

*   **Location:** List the primary directory/files containing the tests for this phase (e.g., `crates/[crate_name]/tests/[feature_module]/`).
*   **Coverage:**
    *   **Unit Tests:** Describe what aspects are covered by unit tests.
    *   **Integration Tests:** Describe what aspects are covered by integration tests, including any specific fixtures used.
*   **Gating:** Confirm that tests are correctly gated by the relevant feature flag(s).

## Conclusion & Next Steps

Briefly summarize the status of this phase (e.g., "Phase [Number] implementation is complete and tested.") and state the next logical phase or task according to the overall plan.
