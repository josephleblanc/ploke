| Subtask | Rationale | Effort (hours) | Breaking Change? | Feature Flag (if applicable) | Commit Message |
|---|---|---|---|---|---|
| Audit and align type representations with CozoDB schema | CozoDB requires strict type adherence (e.g. Bytes for hashes). Direct type alignment prevents runtime conversions and ensures schema compatibility | 4 | Yes - type changes alter serialization format | `feature_cozo_type_migration` | "refactor: align type system with CozoDB requirements" |
| Implement Send + Sync for all public types | Required for safe cross-thread usage and future concurrency needs. Ensures compliance with Rust's ownership model in distributed systems | 2 | Low risk - additive change | None needed | "feat: make all public types Send + Sync" |
| Create direct AST→Cozo streaming pipeline | Eliminate CodeGraph middle layer to reduce memory overhead and potential data staleness. Direct streaming improves performance for large codebases | 6 | No - additive API surface | `feature_direct_ast_streaming` | "perf: implement direct AST to CozoDB streaming" |
| Deprecate CodeGraph with feature flag toggle | Gradual migration path for existing users. Maintains backward compatibility during transition period | 2 | No - old path remains available | `legacy_codegraph` | "deprecate: mark CodeGraph as legacy feature" |
| Implement new relation-based serialization | Match CozoDB's relational model for efficient querying. Enables native graph operations without translation layer | 5 | Yes - changes output format | `feature_relational_export` | "feat: implement native Cozo relation serializer" |
| Remove CodeGraph core structures | Final cleanup after migration period. Reduces maintenance surface and potential confusion | 1 | Yes - removes public API surface | N/A (requires previous flags) | "chore: remove deprecated CodeGraph implementation" |

**Key Considerations:**
1. Type changes use explicit feature flag due to potential serialization breakage
2. Direct streaming implemented as additive functionality to prevent service disruption
3. Legacy path remains available until final removal phase
4. Final CodeGraph removal delayed until v2.0 semver boundary

Approve any subtask for detailed implementation guidance. Would you like me to elaborate on any specific item?
