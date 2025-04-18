# ADR-001: CFG Attribute Processing Strategy

## Status
PROPOSED

## Context
The `syn_parser` needs to correctly interpret `#[cfg(...)]` attributes found in Rust source code. This is necessary for two primary reasons:
1.  **Node ID Disambiguation:** Items defined under different `cfg` flags (e.g., `#[cfg(unix)] struct Foo` vs `#[cfg(windows)] struct Foo`) should have distinct `NodeId`s, both synthetic (Phase 2) and resolved (Phase 3), to accurately represent the code structure.
2.  **RAG Filtering:** The Retrieval-Augmented Generation (RAG) system needs to filter code elements based on a target configuration (e.g., show only code active for `target_os = "linux"`), ensuring retrieved context is relevant to the user's query environment.

Several approaches were considered for storing and evaluating CFG information, involving different trade-offs between parsing complexity, database schema design, and query-time evaluation logic.

## Decision
We will adopt the "Alternative A" strategy outlined in the implementation plan: Evaluate CFG conditions in Rust post-database query.

1.  **Phase 2 (Parsing):**
    *   Calculate a *provisional* effective CFG (`Option<cfg_expr::Expression>`) for each code item by combining its own `#[cfg]` attributes with the CFG inherited from its immediate scope (file, module, struct, etc.).
    *   Store this `provisional_effective_cfg` on the corresponding `*Node` struct (e.g., `FunctionNode`, `StructNode`).
    *   Hash the `provisional_effective_cfg` (using its derived `Hash` or a stable serialization) and include these bytes in the input for generating the `NodeId::Synthetic`.
2.  **Phase 3 (Resolution/DB Prep):**
    *   Calculate the *final* effective CFG for each node by recursively combining its `provisional_effective_cfg` with any `declaration_cfg` attributes found on `mod module;` declarations in its module hierarchy.
    *   Serialize the final `Expression` (e.g., to JSON).
    *   Store the serialized expression and a hash-based ID (`cond_id`) in a new `CfgCondition` Cozo relation.
    *   Link the code node to its condition using a `HasCondition { node_id, cond_id }` relation.
    *   Include the `cond_id` in the input hash for generating the `NodeId::Resolved`.
3.  **RAG Querying:**
    *   Determine the `TargetContext` (target triple, features, etc.).
    *   Fetch candidate `node_id`s from Cozo using primary retrieval methods (e.g., HNSW).
    *   Fetch the corresponding `cond_id`s via the `HasCondition` relation.
    *   Fetch the `serialized_expr` from `CfgCondition` using the `cond_id`s.
    *   In Rust: Deserialize the `serialized_expr` back into `cfg_expr::Expression`.
    *   Evaluate each `Expression` against the `TargetContext` using `cfg_expr::eval()`.
    *   Filter the retrieved nodes, keeping only those whose CFG condition evaluates to true.

## Consequences
- Positive:
    - Ensures `NodeId::Synthetic` and `NodeId::Resolved` correctly distinguish between CFG-gated items.
    - Enables accurate filtering of code context in the RAG based on target configuration.
    - Leverages the robust `cfg-expr` crate for parsing and evaluation, handling complex CFG logic correctly.
    - Avoids implementing complex CFG evaluation logic within Cozo's Datalog.
    - Keeps Phase 2 parsing relatively focused on local context.
- Negative:
    - Introduces a distinct Phase 3 calculation step for the final effective CFG.
    - Adds complexity to the RAG query workflow (fetch nodes -> fetch conditions -> fetch expressions -> evaluate in Rust).
    - Potential performance overhead during RAG filtering due to deserialization and evaluation (mitigated by only evaluating candidates).
    - Relies on the stability of `cfg_expr::Expression`'s `Hash` implementation (if used directly) or requires implementing stable serialization for hashing.
- Neutral:
    - Introduces two new relations (`CfgCondition`, `HasCondition`) to the Cozo schema.
    - Requires logic to determine the `TargetContext` for RAG queries.
    - Defers handling of `#[cfg_attr(...)]`.

## Compliance
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-GOOD-ERR: Error handling during `cfg_expr::Expression::parse` should be robust.
- C-DEBUG: `cfg_expr::Expression` should derive `Debug` for logging/diagnostics.
- C-SERDE: Relies on `cfg-expr` deriving `Serialize`, `Deserialize` (via feature flag).
- C-HASH: Relies on `cfg-expr` deriving `Hash` (via feature flag) or requires stable serialization for hashing.

[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: N/A
