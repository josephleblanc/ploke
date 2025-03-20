# Type Mapping - Rust to CozoDB

This document details the mapping of Rust types (as used in the `syn_parser` crate) to corresponding CozoDB types.  This mapping is crucial for ensuring data integrity and compatibility when storing parsed code information in CozoDB.

## Mapping Table

| Rust Type (`TypeKind`) | CozoDB Type | Notes |
|---|---|---|
| `Named` (identifiers - function names, struct names, etc.) | `Bytes` |  Using `Bytes` for identifiers allows for efficient storage and comparison.  Consider potential performance implications of `Bytes` comparisons. |
| `Reference` | N/A | References are handled through relationships, not direct type mapping. |
| `Slice` | N/A | Slices are handled through relationships, not direct type mapping. |
| `Array` | N/A | Arrays are handled through relationships, not direct type mapping. |
| `Tuple` | N/A | Tuples are handled through relationships, not direct type mapping. |
| `Function` | N/A | Functions are represented as nodes with relationships to their parameters and return types. |
| `Never` | `Null` | Represents a type that never evaluates to a value. |
| `Inferred` | `String` |  Inferred types are often represented as strings for debugging and analysis. |
| `RawPointer` | `Null` | Raw pointers don't have a direct CozoDB equivalent.  Representing them as `Null` might be appropriate, or they could be omitted. |
| `TraitObject` | `String` | Trait objects are represented as strings (trait name). |
| `ImplTrait` | `String` | Impl traits are represented as strings (trait name). |
| `Paren` | N/A | Parenthesized types are handled recursively. |
| `Macro` | `String` | Macros are represented as strings (macro name). |
| `Unknown` | `String` | Unknown types are represented as strings for debugging. |

## Considerations

*   **Data Loss:**  Be mindful of potential data loss during type conversion. For example, converting floating-point numbers to integers may result in truncation.
*   **Performance:**  The choice of CozoDB types can impact performance.  `Bytes` comparisons can be slower than integer comparisons.
*   **Generics:** Handling generic types requires careful consideration.  The type parameters should be stored as separate nodes and linked to the generic type.
*   **Error Handling:**  Implement robust error handling to gracefully handle type conversion failures.

## Future Work

*   Investigate the use of custom CozoDB types to represent complex Rust types more accurately.
*   Develop a comprehensive set of unit tests to verify the type mapping.
