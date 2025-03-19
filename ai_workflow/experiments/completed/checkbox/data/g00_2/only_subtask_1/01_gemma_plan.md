Okay, I understand the task. We need to align the types used within the `syn_parser` crate to match the CozoDB schema. This is a potentially significant change, so a phased approach with feature flags is a good idea. Here's a breakdown of subtasks, with risk assessment and feature flags where appropriate:

| Subtask | Rationale | Effort (hours) | Breaking Change? | Feature Flag (if applicable) | Commit Message |
|---|---|---|---|---|---|
| **1. Analyze CozoDB Type Mapping** |  Establish a clear mapping between `syn_parser`'s current types and the corresponding CozoDB types. This is the foundation for all subsequent steps. | 4 | No |  |  `feat: Analyze CozoDB type mapping` |
| **2. Introduce `Bytes` for Content Hashes** | Replace any string-based representations of content hashes with a `Bytes` type. This is a common change and a good starting point. | 6 | No |  | `feat: Use Bytes for content hashes` |
| **3. Refactor `FunctionNode` - Replace String with `Bytes` for function body** |  The `FunctionNode` currently uses `Option<String>` for the function body.  Change this to `Option<Bytes>`. | 8 | No |  | `feat: Use Bytes for function bodies` |
| **4. Numeric Type Review & Initial Conversion (Int -> Number)** | Review all integer types (e.g., in `FieldNode`, `VariantNode`) and begin converting them to the CozoDB `Number` type.  Start with a focused scope (e.g., only fields representing IDs). | 8 | Yes | `feature_cozo_number_ids` | `feat: Convert ID fields to Cozo Number type (feature-gated)` |
| **5. Numeric Type Conversion (Float -> Number)** | Extend the numeric type conversion to include floating-point numbers. | 6 | Yes | `feature_cozo_number_ids` | `feat: Convert float fields to Cozo Number type (feature-gated)` |
| **6.  Type Alias Conversion** |  Examine `TypeAliasNode` and ensure the aliased types are also aligned with CozoDB types. | , 4 | Yes | `feature_cozo_type_alignment` | `feat: Align TypeAliasNode types with CozoDB (feature-gated)` |
| **7.  Vector Type Handling** |  Implement support for `Vector` types in `FieldNode` and `VariantNode` where appropriate. This will likely involve creating a new type or adapting existing ones. | 12 | Yes | `feature_cozo_vector_support` | `feat: Add support for CozoDB Vector types (feature-gated)` |
| **8.  Json Support** |  If `Json` is used in any part of the `syn_parser` data structures, ensure it's handled correctly and consistently with CozoDB's `Json` type. | 6 | Yes | `feature_cozo_json_support` | `feat: Align Json types with CozoDB (feature-gated)` |
| **9.  Comprehensive Testing** |  Add comprehensive tests to verify the correctness of the type conversions and ensure no data loss or unexpected behavior. | 16 | No |  | `test: Comprehensive tests for CozoDB type alignment` |
| **10. Feature Flag Removal & Cleanup** | Once confident in the changes, remove the feature flags and clean up any related code. | 4 | Yes |  | `chore: Remove CozoDB type alignment feature flags` |

**Notes:**

* **Effort:** These are estimates and could vary significantly based on the complexity of the codebase and the extent of type usage.
* **Breaking Changes:**  The changes to numeric types and vector types are most likely to introduce breaking changes because they change the underlying representation of data.  The feature flags allow us to test these changes in a controlled environment.
* **Feature Flags:**  The feature flags allow us to enable the new types incrementally and test them without impacting existing functionality.
* **Commit Messages:** These are suggestions and can be refined.

Please let me know if you'd like me to elaborate on any of these subtasks or adjust the plan. I'm ready to proceed when you give the go-ahead.
