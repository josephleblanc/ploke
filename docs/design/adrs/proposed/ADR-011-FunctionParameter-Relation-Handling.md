# ADR-001: Handling Function Parameter Type Relations

## Status
PROPOSED

## Context
During Phase 2 parsing (`CodeVisitor`), relations of kind `RelationKind::FunctionParameter` were being created immediately within the `visit_item_fn` method for each function parameter encountered. This relation linked the function's `NodeId` to the parameter's `TypeId`.

The problem arises when a function has multiple parameters of the *same type*. For example, in `fn example(a: i32, b: i32)`, both parameters `a` and `b` resolve to the same `TypeId` for `i32`. The visitor loop would then attempt to add the exact same relation `Node(example_id) -> Type(i32_type_id)` with kind `FunctionParameter` twice, leading to duplicate relations in the `ParsedCodeGraph`.

This duplication violates graph integrity assumptions and causes errors in validation checks (`validate_unique_rels`). It stems from conflating the specific parameter *declaration* (which should ideally have its own `NodeId`) with the *type* used by the parameter (`TypeId`). The relation being created only captured the function-to-type link, losing the distinction between individual parameters sharing that type.

## Decision
We will adopt the "Create Relations Later (Phase 3 / Post-Processing)" approach:

1.  **Remove Immediate Relation Creation:** The `relations.push(...)` call for `RelationKind::FunctionParameter` within the parameter processing loop in `CodeVisitor::visit_item_fn` will be removed (or remain commented out).
2.  **Store Information:** The necessary information to create these relations later (specifically, the function's `NodeId` and the `TypeId` of each of its parameters) will be stored during Phase 2. This information is already captured within the `FunctionNode` struct (in the `parameters: Vec<ParamData>` field, where `ParamData` contains the `TypeId`).
3.  **Implement Post-Processing Step:** A new step will be added after the initial graph parsing and merging (likely at the beginning of Phase 3 resolution or as a dedicated step between Phase 2 and Phase 3).
4.  **Deduplicated Relation Creation:** This post-processing step will:
    *   Iterate through all `FunctionNode`s in the merged graph.
    *   For each function, iterate through its `parameters: Vec<ParamData>`.
    *   Maintain a temporary collection (e.g., `HashSet<(NodeId, TypeId)>`) for the *current function* being processed to track which `FunctionParameter` relations have already been added *for that specific function*.
    *   For each parameter's `TypeId`, check the temporary collection. If the pair `(fn_id, param_type_id)` is not present, add it to the set and create the `Relation { source: GraphId::Node(fn_id), target: GraphId::Type(param_type_id), kind: RelationKind::FunctionParameter }`, adding it to the main graph's relations collection.

This ensures that for any given function, the relation linking it to a specific parameter type is added exactly once, regardless of how many parameters share that type.

## Consequences
- **Positive:**
    - Fixes the duplicate `FunctionParameter` relation bug.
    - Simplifies the logic within `CodeVisitor::visit_item_fn` by removing immediate relation handling.
    - Correctly models the intended relationship ("function uses type X as a parameter") without redundancy in the final graph used for Phase 3.
- **Negative:**
    - Defers the creation of these specific relations; the graph generated directly from Phase 2 parsing will be missing them.
    - Introduces a new post-processing step, adding minor complexity to the overall pipeline between Phase 2 and Phase 3.
    - Requires careful implementation of the deduplication logic in the post-processing step.
- **Neutral:**
    - The fundamental relationship between functions and their parameter types is still captured accurately, just at a later stage in the processing pipeline.

## Compliance
- [PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md): Aligns with the phased architecture by deferring this specific relation resolution until after initial parsing (Phase 2).
- [IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md): The post-processing step will leverage iterators (`C-ITER`) and potentially `HashSet` for efficient deduplication. Error handling (`C-GOOD-ERR`) should be considered if the post-processing step can fail.
- [PRINCIPLES.md](ai_workflow/AI_Always_Instructions/PRINCIPLES.md): Addresses a design integrity issue (duplicate relations) and aims for robustness.
