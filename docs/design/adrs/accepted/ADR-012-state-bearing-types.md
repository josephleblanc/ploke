# ADR-012: Using State-Bearing Types for Graph Processing Stages

## Status
ACCEPTED 2025-04-28

## Context
The code graph undergoes several processing stages (parsing, merging, validation, resolution). Functions operating at later stages often rely on invariants established by earlier stages (e.g., name resolution requires a validated graph with unique node IDs). Passing the same graph type (e.g., `CodeGraph`) through all stages makes it possible to accidentally call a function with a graph that hasn't completed the necessary prerequisite steps, leading to runtime errors or incorrect results. We need a mechanism to enforce the correct sequence of operations and ensure functions receive graph data that meets their preconditions.

## Decision
We will define distinct Rust types (structs) to represent the graph data at different significant stages of processing. For example:
- `UnvalidatedSyntacticGraph`: Represents the raw graph merged from parsing, potentially containing inconsistencies like duplicate NodeIds.
- `ValidatedSyntacticGraph`: Represents a graph that has passed basic validation checks (e.g., NodeId uniqueness). Indices for relations might be built at this stage.
- `LogicalGraph`: Represents the graph after name/type resolution, containing resolved entities and semantic relationships (potentially built by `ploke-graph`).

Functions performing state transitions (e.g., `validate`) will take one state type as input and return a `Result` containing the next state type upon success (e.g., `fn validate(UnvalidatedSyntacticGraph) -> Result<ValidatedSyntacticGraph, ValidationError>`). Functions operating at a specific stage will require the corresponding state type in their signature (e.g., `fn resolve(graph: &ValidatedSyntacticGraph, ...)`).

This leverages Rust's type system to enforce the processing order at compile time.

## Consequences
- **Positive:**
    - **Compile-Time Safety:** Prevents misuse of graph data by ensuring functions receive graphs in the required state. Invalid state transitions become type errors.
    *   **Improved Clarity:** Function signatures clearly document the expected state of the graph data they operate on.
    *   **Explicit State Transitions:** The use of `Result` for transition functions forces explicit handling of potential failures during stage changes.
    *   **Encapsulation:** Each state type can encapsulate the specific data and invariants relevant to that stage.
- **Negative:**
    - **Boilerplate:** May require defining multiple similar struct definitions if the underlying data fields are largely the same across states.
    - **Data Conversion:** Explicit conversion logic is needed in the state transition functions (e.g., moving data from `UnvalidatedSyntacticGraph` to `ValidatedSyntacticGraph`).
- **Neutral:**
    - Requires careful design of which states warrant distinct types.

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items:
- Section 5: Component Details (implies different data representations between components/stages)
- Section 7: Concurrency Model (distinct types help manage state across boundaries)
- Section 8: Potential Risks and Mitigations (addresses risks related to inconsistent state)
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-CUSTOM-TYPE: Arguments convey meaning through types.
- Type safety: Leverages the type system effectively.
- Dependability: Makes it less likely to do the wrong thing.
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
