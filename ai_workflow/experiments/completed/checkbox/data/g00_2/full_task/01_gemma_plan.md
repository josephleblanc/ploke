Okay, I understand the task. The goal is to adapt `syn_parser` to directly output data compatible with CozoDB, focusing on type alignment and `Send + Sync` compliance, while minimizing changes to the core data flow and reducing the scope of `CodeGraph`. Here's a breakdown of subtasks:

| Subtask | Rationale | Effort (hours) | Breaking Change? | Feature Flag (if applicable) | Commit Message |
|---|---|---|---|---|---|
| **1. Analyze CozoDB Types & `syn_parser` Mismatch** | Identify all type mismatches between `syn_parser`'s current types (e.g., `String`, `usize`) and the corresponding CozoDB types (e.g., `Bytes`, `Int`, `Float`). This is the foundational step for all subsequent changes. | 4 | No | N/A | `feat: Analyze CozoDB type compatibility` |
| **2. Implement `Send + Sync` for Core Data Structures** | Add `Send + Sync` bounds to all publicly exposed structs within `parser/nodes.rs` and `parser/graph.rs`. This ensures thread safety and prepares for potential future concurrency.  This might involve adding `Arc` or `Mutex` where necessary, but aim to minimize this initially. | 8 | Potentially (if public API changes) | `feature_send_sync_fixes` | `feat: Implement Send + Sync for core data structures` |
| **3. Refactor `Attribute` struct to use `String` for arguments and values** | The `Attribute` struct currently uses `Vec<String>` for arguments and an `Option<String>` for the value. This is fine for now, but it's good to standardize on `String` for consistency with CozoDB's string type. | 2 | No | N/A | `refactor: Standardize Attribute struct` |
| **4. Initial `CodeGraph` Reduction - Remove `defined_types` and `type_graph`** |  Remove the `defined_types` and `type_graph` fields from the `CodeGraph` struct. These seem redundant given the goal of direct streaming to CozoDB.  This simplifies the `CodeGraph` and reduces memory usage. | 4 | No | N/A | `refactor: Remove redundant type information from CodeGraph` |
| **5.  Begin Direct Streaming - Function Node to CozoDB Representation** |  Modify the `VisitorState` in `parser/visitor.rs` to, instead of building a `FunctionNode`, directly construct a representation suitable for insertion into CozoDB.  Start with the `FunctionNode` and its associated data. This is the first step towards eliminating the `CodeGraph` as an intermediary. | 8 | Potentially (if the structure of `FunctionNode` is exposed publicly) | `feature_direct_streaming` | `feat: Begin direct streaming - FunctionNode to CozoDB` |
| **6. Adapt `String` to `Bytes` where appropriate** | Identify fields that represent content (e.g., docstrings, function bodies) and convert them from `String` to `Bytes` before storing them in the CozoDB representation. | 4 | No | N/A | `feat: Convert String to Bytes for content fields` |
| **7.  Continue Direct Streaming - Struct/Enum/Trait Nodes** | Extend the direct streaming approach to `StructNode`, `EnumNode`, and `TraitNode`.  This will involve adapting the data structures and the visitor pattern to generate CozoDB-compatible representations directly. | 12 | Potentially (if the structure of these nodes is exposed publicly) | `feature_direct_streaming` | `feat: Continue direct streaming - Struct/Enum/Trait Nodes` |
| **8.  Final `CodeGraph` Reduction - Remove Remaining Fields** |  After streaming all relevant node types directly to CozoDB, remove the remaining fields from the `CodeGraph` struct (e.g., `functions`, `impls`, `traits`).  This completes the elimination of the `CodeGraph` as an intermediary. | 2 | No | N/A | `refactor: Final CodeGraph reduction` |

**Notes:**

*   Effort estimates are rough and assume a reasonable level of familiarity with the codebase.
*   Breaking changes are assessed based on potential API changes.  The feature flags are proposed as a precaution.
*   The `feature_direct_streaming` flag allows for a gradual rollout of the direct streaming approach, enabling testing and validation before fully committing to the change.



Please let me know if you'd like me to elaborate on any of these subtasks or adjust the plan.  I'm ready to proceed when you give the go-ahead.
