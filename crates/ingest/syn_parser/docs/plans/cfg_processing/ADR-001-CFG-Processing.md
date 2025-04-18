# ADR-001: CFG Attribute Processing Strategy

## Status
PROPOSED (Partially addressed by ADR-002)

**Note:** The immediate goal of ensuring unique *synthetic* IDs is addressed by [ADR-002](./ADR-002-Minimal-CFG-Hashing.md), which implements the Phase 2 hashing logic described below as a first step. This ADR outlines the full plan, including Phase 3 and RAG integration.

## Context
The `syn_parser` needs to correctly interpret `#[cfg(...)]` attributes found in Rust source code. This is necessary for two primary reasons:
1.  **Node ID Disambiguation:** Items defined under different `cfg` flags (e.g., `#[cfg(unix)] struct Foo` vs `#[cfg(windows)] struct Foo`) should have distinct `NodeId`s, both synthetic (Phase 2) and resolved (Phase 3), to accurately represent the code structure.
2.  **RAG Filtering:** The Retrieval-Augmented Generation (RAG) system needs to filter code elements based on a target configuration (e.g., show only code active for `target_os = "linux"`), ensuring retrieved context is relevant to the user's query environment.

Several approaches were considered for storing and evaluating CFG information, involving different trade-offs between parsing complexity, database schema design, and query-time evaluation logic.

## Decision
We will adopt the "Alternative A" strategy: Evaluate CFG conditions in Rust post-database query. The implementation is phased:

1.  **Phase 2 (Parsing - Minimal CFG Hashing per ADR-002):**
    *   Extract the raw string content from `#[cfg(...)]` attributes attached directly to each code item. Store this in a `cfgs: Vec<String>` field on the corresponding `*Node` struct.
    *   Track the combined list of inherited raw CFG strings (`current_scope_cfgs`) during visitation.
    *   For `NodeId::Synthetic` generation: Combine the item's own raw `cfgs` with the inherited `scope_cfgs`, sort the combined list alphabetically, join into a single delimited string, hash the bytes of this string, and include these bytes in the `NodeId` hash input.
    *   **No `cfg-expr` parsing occurs in Phase 2.**
2.  **Phase 3 (Resolution/DB Prep):**
    *   For each node, collect its own `cfgs` and recursively collect the `cfgs` from its module hierarchy (including `mod module;` declarations).
    *   Parse all collected raw strings for a node using `cfg_expr::Expression::parse`.
    *   Combine the parsed `Expression`s logically (likely using an `all(...)` structure, ensuring deterministic ordering) to calculate the *final* effective `cfg_expr::Expression` for the node.
    *   Serialize this final `Expression` (e.g., to JSON or potentially its `.original()` string if stable enough after combination).
    *   Store the serialized final expression string and a hash-based ID (`cond_id`) in a new `CfgCondition` Cozo relation.
    *   Link the code node to its condition using a `HasCondition { node_id, cond_id }` relation.
    *   Include the `cond_id` (or a hash derived from the final expression string) in the input hash for generating the `NodeId::Resolved`.
3.  **RAG Querying:**
    *   Determine the `TargetContext` (target triple, features, etc.).
    *   Fetch candidate `node_id`s from Cozo.
    *   Fetch the corresponding `cond_id`s via `HasCondition`.
    *   Fetch the `serialized_final_expr_string` from `CfgCondition`.
    *   In Rust: Parse the `serialized_final_expr_string` into a `cfg_expr::Expression`.
    *   Evaluate the `Expression` against the `TargetContext` using `cfg_expr::eval()`.
    *   Filter the retrieved nodes based on the evaluation result.

## Consequences
- Positive:
    - Ensures `NodeId::Synthetic` and `NodeId::Resolved` correctly distinguish between CFG-gated items.
    - Enables accurate filtering of code context in the RAG based on target configuration.
    - Leverages the robust `cfg-expr` crate for parsing and evaluation *at the appropriate time* (Phase 3 / RAG).
    - Avoids implementing complex CFG evaluation logic within Cozo's Datalog.
    - Keeps Phase 2 parsing simple by only handling raw strings.
- Negative:
    - Introduces a distinct Phase 3 calculation step for parsing and combining raw CFGs into the final effective `Expression`.
    - Adds complexity to the RAG query workflow (fetch nodes -> fetch conditions -> fetch final expression string -> parse -> evaluate).
    - Potential performance overhead during RAG filtering due to parsing and evaluation (mitigated by only processing candidates).
    - Phase 2 `NodeId::Synthetic` generation has limitations regarding semantically equivalent but syntactically different CFGs (see ADR-002).
- Neutral:
    - Introduces two new relations (`CfgCondition`, `HasCondition`) to the Cozo schema.
    - Requires logic to determine the `TargetContext` for RAG queries.
    - Defers handling of `#[cfg_attr(...)]`.

## Compliance
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-GOOD-ERR: Error handling during `cfg_expr::Expression::parse` (in Phase 3/RAG) should be robust.
- C-DEBUG: `cfg_expr::Expression` should derive `Debug` for logging/diagnostics when used.
- C-SERDE: Required for serializing the *final* `Expression` in Phase 3. Assumes `cfg-expr` provides this or we use a stable string format.

[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: N/A
